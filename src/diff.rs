//! Full diff of a repository against the Cambios base: run git, parse the unified diff.
//! The parser and the scroll math are pure; `load` runs on a short-lived thread.
use std::io::Read;
use std::ops::Range;
use std::path::Path;
use std::process::{Command, Stdio};

pub const MAX_FILE_LINES: usize = 5000;
pub const MAX_BYTES: usize = 3 * 1024 * 1024;
const MAX_UNTRACKED: usize = 200;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// `@@ -a,b +c,d @@ …`, text is the whole header.
    Hunk,
    Add,
    Del,
    Ctx,
    /// `\ No newline at end of file`.
    NoNewline,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Line {
    pub kind: Kind,
    pub old: Option<u32>,
    pub new: Option<u32>,
    /// Without the leading `+`/`-`/space.
    pub text: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Status {
    #[default]
    Modified,
    Added,
    Deleted,
    Renamed,
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct File {
    pub path: String,
    /// Old path of a rename.
    pub from: Option<String>,
    pub status: Status,
    pub binary: bool,
    pub added: u32,
    pub removed: u32,
    pub lines: Vec<Line>,
    pub truncated: bool,
}

impl File {
    fn push(&mut self, line: Line) {
        if self.lines.len() < MAX_FILE_LINES {
            self.lines.push(line);
        } else {
            self.truncated = true;
        }
    }
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct Diff {
    pub files: Vec<File>,
    /// The total byte cap was hit.
    pub truncated: bool,
    pub error: Option<String>,
}

/// Parse `git diff` output (with `a/` and `b/` prefixes).
pub fn parse(text: &str) -> Vec<File> {
    let mut files: Vec<File> = vec![];
    // Lines still expected in the current hunk, and the next line numbers.
    let (mut old_left, mut new_left, mut old_no, mut new_no) = (0u32, 0u32, 0u32, 0u32);
    for raw in text.lines() {
        if let Some(f) = files.last_mut().filter(|_| old_left > 0 || new_left > 0) {
            let rest = raw.get(1..).unwrap_or("").to_string();
            let line = match raw.as_bytes().first() {
                Some(b'+') => {
                    new_left = new_left.saturating_sub(1);
                    new_no += 1;
                    f.added += 1;
                    Line { kind: Kind::Add, old: None, new: Some(new_no - 1), text: rest }
                }
                Some(b'-') => {
                    old_left = old_left.saturating_sub(1);
                    old_no += 1;
                    f.removed += 1;
                    Line { kind: Kind::Del, old: Some(old_no - 1), new: None, text: rest }
                }
                Some(b'\\') => Line { kind: Kind::NoNewline, old: None, new: None, text: rest.trim().to_string() },
                _ => {
                    old_left = old_left.saturating_sub(1);
                    new_left = new_left.saturating_sub(1);
                    old_no += 1;
                    new_no += 1;
                    Line { kind: Kind::Ctx, old: Some(old_no - 1), new: Some(new_no - 1), text: rest }
                }
            };
            f.push(line);
            continue;
        }
        if let Some(rest) = raw.strip_prefix("diff --git ") {
            // `a/x b/x`; `+++` or `rename to` correct it when present.
            let path = rest.rsplit_once(" b/").map_or(rest, |(_, b)| b);
            files.push(File { path: path.to_string(), ..File::default() });
            continue;
        }
        let Some(f) = files.last_mut() else { continue };
        if let Some(h) = raw.strip_prefix("@@ ") {
            let Some(((o, ol), (n, nl))) = hunk_ranges(h) else { continue };
            (old_no, old_left, new_no, new_left) = (o, ol, n, nl);
            f.push(Line { kind: Kind::Hunk, old: None, new: None, text: raw.to_string() });
        } else if raw.starts_with("new file mode") {
            f.status = Status::Added;
        } else if raw.starts_with("deleted file mode") {
            f.status = Status::Deleted;
        } else if let Some(p) = raw.strip_prefix("rename from ") {
            f.status = Status::Renamed;
            f.from = Some(p.to_string());
        } else if let Some(p) = raw.strip_prefix("rename to ") {
            f.path = p.to_string();
        } else if let Some(p) = raw.strip_prefix("+++ b/") {
            f.path = p.trim_end_matches('\t').to_string();
        } else if raw.starts_with("Binary files ") || raw == "GIT binary patch" {
            f.binary = true;
        } else if let Some(rest) = raw.strip_prefix('\\') {
            f.push(Line { kind: Kind::NoNewline, old: None, new: None, text: rest.trim().to_string() });
        }
    }
    files
}

/// `-a,b +c,d @@ …` → ((a, b), (c, d)); a missing count means 1.
fn hunk_ranges(h: &str) -> Option<((u32, u32), (u32, u32))> {
    let mut it = h.split(' ');
    let range = |s: &str| -> Option<(u32, u32)> {
        match s.split_once(',') {
            Some((a, b)) => Some((a.parse().ok()?, b.parse().ok()?)),
            None => Some((s.parse().ok()?, 1)),
        }
    };
    let old = range(it.next()?.strip_prefix('-')?)?;
    let new = range(it.next()?.strip_prefix('+')?)?;
    Some((old, new))
}

/// An untracked file as an all-added diff; `cut` means the byte budget ended inside it.
pub fn untracked(path: &str, bytes: &[u8], cut: bool) -> File {
    let mut f = File { path: path.to_string(), status: Status::Added, truncated: cut, ..File::default() };
    if bytes[..bytes.len().min(8000)].contains(&0) {
        f.binary = true;
        return f;
    }
    let text = String::from_utf8_lossy(bytes);
    let n = text.lines().count() as u32;
    if n == 0 {
        return f;
    }
    f.push(Line { kind: Kind::Hunk, old: None, new: None, text: format!("@@ -0,0 +1,{n} @@") });
    for (i, l) in text.lines().enumerate() {
        f.added += 1;
        f.push(Line { kind: Kind::Add, old: None, new: Some(i as u32 + 1), text: l.to_string() });
    }
    if !cut && !text.ends_with('\n') {
        f.push(Line { kind: Kind::NoNewline, old: None, new: None, text: "No newline at end of file".into() });
    }
    f
}

/// Run git reading at most `cap` bytes of stdout; the bool says the output was cut.
/// shortcut: no timeout (git diff waits on neither input nor locks); add one if it ever hangs.
fn git_capped(repo: &Path, args: &[&str], cap: usize) -> Result<(Vec<u8>, bool), String> {
    let mut child = Command::new("git")
        .args(args)
        .current_dir(repo)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let mut buf = vec![];
    if let Some(out) = child.stdout.take() {
        out.take(cap as u64 + 1).read_to_end(&mut buf).map_err(|e| e.to_string())?;
    }
    let cut = buf.len() > cap;
    if cut {
        buf.truncate(cap);
        let _ = child.kill();
    }
    let ok = child.wait().is_ok_and(|s| s.success());
    if !ok && !cut {
        return Err("git diff falló".into());
    }
    Ok((buf, cut))
}

/// Working tree against the Cambios base, plus untracked files, within the caps.
pub fn load(repo: &Path) -> Diff {
    let base = crate::context::diff_base(repo);
    let args = ["-c", "core.quotepath=false", "diff", "--no-color", "--no-ext-diff", "-M", "--src-prefix=a/", "--dst-prefix=b/", &base];
    let (out, cut) = match git_capped(repo, &args, MAX_BYTES) {
        Ok(r) => r,
        Err(e) => return Diff { error: Some(e), ..Diff::default() },
    };
    let mut files = parse(&String::from_utf8_lossy(&out));
    if cut {
        if let Some(f) = files.last_mut() {
            f.truncated = true;
        }
    }
    let mut budget = MAX_BYTES - out.len();
    let mut truncated = cut;
    let list = git_capped(repo, &["ls-files", "--others", "--exclude-standard", "-z"], 1 << 20).map(|r| r.0).unwrap_or_default();
    for path in String::from_utf8_lossy(&list).split('\0').filter(|p| !p.is_empty()).take(MAX_UNTRACKED) {
        if budget == 0 {
            truncated = true;
            break;
        }
        // Nested repositories show up as `dir/` and fail to open: skipped.
        let Ok(file) = std::fs::File::open(repo.join(path)) else { continue };
        let mut bytes = vec![];
        if file.take(budget as u64 + 1).read_to_end(&mut bytes).is_err() {
            continue;
        }
        let over = bytes.len() > budget;
        bytes.truncate(budget);
        budget -= bytes.len();
        truncated |= over;
        files.push(untracked(path, &bytes, over));
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Diff { files, truncated, error: None }
}

/// Rows a file takes in the viewer: its lines, or one message row, plus the truncation row.
pub fn rows(f: &File) -> usize {
    f.lines.len().max(1) + usize::from(f.truncated)
}

/// Whole rows that fit in `view_h`.
fn per_page(row_h: f32, view_h: f32) -> usize {
    ((view_h / row_h).floor() as usize).max(1)
}

/// Rows to draw for a scroll offset in pixels; rows snap to whole lines (there is no clipping).
pub fn visible(scroll: f32, rows: usize, row_h: f32, view_h: f32) -> Range<usize> {
    let per = per_page(row_h, view_h);
    let first = ((scroll.max(0.0) / row_h).round() as usize).min(rows.saturating_sub(per));
    first..(first + per).min(rows)
}

/// Keep the offset between the top and the point where the last row is at the bottom.
pub fn clamp(scroll: f32, rows: usize, row_h: f32, view_h: f32) -> f32 {
    scroll.clamp(0.0, rows.saturating_sub(per_page(row_h, view_h)) as f32 * row_h)
}

/// Offset that makes row `i` fully visible, moving as little as possible.
pub fn reveal(scroll: f32, i: usize, row_h: f32, view_h: f32) -> f32 {
    let per = per_page(row_h, view_h);
    let first = (scroll / row_h).round() as usize;
    if i < first {
        i as f32 * row_h
    } else if i >= first + per {
        (i + 1 - per) as f32 * row_h
    } else {
        scroll
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
diff --git a/src/a.rs b/src/a.rs
index 1111111..2222222 100644
--- a/src/a.rs
+++ b/src/a.rs
@@ -10,4 +10,4 @@ fn main() {
 keep
--- removed line that looks like a header
+++ added line that looks like a header
 keep2
-old last
\\ No newline at end of file
+new last
\\ No newline at end of file
diff --git a/old name.rs b/new name.rs
similarity index 90%
rename from old name.rs
rename to new name.rs
index 3333333..4444444 100644
--- a/old name.rs
+++ b/new name.rs
@@ -1 +1 @@
-x
+y
diff --git a/pure.rs b/moved/pure.rs
similarity index 100%
rename from pure.rs
rename to moved/pure.rs
diff --git a/logo.png b/logo.png
index 5555555..6666666 100644
Binary files a/logo.png and b/logo.png differ
diff --git a/new.txt b/new.txt
new file mode 100644
index 0000000..7777777
--- /dev/null
+++ b/new.txt
@@ -0,0 +1,2 @@
+one
+two
diff --git a/gone.txt b/gone.txt
deleted file mode 100644
index 8888888..0000000
--- a/gone.txt
+++ /dev/null
@@ -1 +0,0 @@
-bye
";

    #[test]
    fn parse_kinds() {
        let files = parse(SAMPLE);
        let names: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(names, ["src/a.rs", "new name.rs", "moved/pure.rs", "logo.png", "new.txt", "gone.txt"]);

        let a = &files[0];
        assert_eq!((a.status, a.added, a.removed), (Status::Modified, 2, 2));
        let k: Vec<(Kind, Option<u32>, Option<u32>)> = a.lines.iter().map(|l| (l.kind, l.old, l.new)).collect();
        assert_eq!(
            k,
            [
                (Kind::Hunk, None, None),
                (Kind::Ctx, Some(10), Some(10)),
                (Kind::Del, Some(11), None),
                (Kind::Add, None, Some(11)),
                (Kind::Ctx, Some(12), Some(12)),
                (Kind::Del, Some(13), None),
                (Kind::NoNewline, None, None),
                (Kind::Add, None, Some(13)),
                (Kind::NoNewline, None, None),
            ]
        );
        assert_eq!(a.lines[2].text, "-- removed line that looks like a header");
        assert_eq!(a.lines[6].text, "No newline at end of file");

        let r = &files[1];
        assert_eq!((r.status, r.from.as_deref(), r.added, r.removed), (Status::Renamed, Some("old name.rs"), 1, 1));
        let p = &files[2];
        assert_eq!((p.status, p.from.as_deref(), p.lines.len()), (Status::Renamed, Some("pure.rs"), 0));
        assert!(files[3].binary && files[3].lines.is_empty());
        assert_eq!((files[4].status, files[4].added, files[4].lines[2].new), (Status::Added, 2, Some(2)));
        assert_eq!((files[5].status, files[5].removed, files[5].lines[1].old), (Status::Deleted, 1, Some(1)));
    }

    #[test]
    fn per_file_cap() {
        let n = MAX_FILE_LINES + 10;
        let mut text = format!("diff --git a/big b/big\n--- a/big\n+++ b/big\n@@ -0,0 +1,{n} @@\n");
        for i in 0..n {
            text.push_str(&format!("+{i}\n"));
        }
        let f = &parse(&text)[0];
        assert!(f.truncated);
        assert_eq!((f.lines.len(), f.added), (MAX_FILE_LINES, n as u32));
        assert_eq!(rows(f), MAX_FILE_LINES + 1);
    }

    #[test]
    fn untracked_files() {
        let f = untracked("n.txt", b"a\nb", false);
        assert_eq!((f.status, f.added, f.lines.len()), (Status::Added, 2, 4));
        assert_eq!(f.lines[0].text, "@@ -0,0 +1,2 @@");
        assert_eq!(f.lines[3].kind, Kind::NoNewline);
        assert!(untracked("x.bin", b"PNG\0\x01", false).binary);
        assert!(untracked("e", b"", false).lines.is_empty());
    }

    #[test]
    fn this_repo_loads() {
        let repo = crate::context::repo_root(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        assert!(load(&repo).error.is_none());
    }

    #[test]
    fn scroll_math() {
        // 100 rows of 10 px in a 95 px view: 9 whole rows per page.
        assert_eq!(visible(0.0, 100, 10.0, 95.0), 0..9);
        assert_eq!(visible(24.0, 100, 10.0, 95.0), 2..11);
        assert_eq!(visible(5000.0, 100, 10.0, 95.0), 91..100);
        assert_eq!(visible(0.0, 3, 10.0, 95.0), 0..3);
        assert_eq!(clamp(-5.0, 100, 10.0, 95.0), 0.0);
        assert_eq!(clamp(5000.0, 100, 10.0, 95.0), 910.0);
        assert_eq!(clamp(50.0, 3, 10.0, 95.0), 0.0);
        assert_eq!(reveal(0.0, 4, 10.0, 95.0), 0.0);
        assert_eq!(reveal(0.0, 12, 10.0, 95.0), 40.0);
        assert_eq!(reveal(200.0, 3, 10.0, 95.0), 30.0);
    }
}
