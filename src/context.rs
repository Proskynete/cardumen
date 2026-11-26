//! Session context for returning to a session: step, PR/MR, changes, Claude's last words.
//! Pure readers/parsers first; the worker thread at the bottom runs them off the UI thread.
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::layout::PaneId;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Provider {
    GitHub,
    GitLab,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PrState {
    Draft,
    Open,
    Approved,
    ChangesRequested,
    Merged,
    Closed,
}

impl PrState {
    pub fn label(self) -> &'static str {
        match self {
            PrState::Draft => "draft",
            PrState::Open => "abierto",
            PrState::Approved => "aprobado",
            PrState::ChangesRequested => "cambios pedidos",
            PrState::Merged => "mergeado",
            PrState::Closed => "cerrado",
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Pr {
    /// `#42` on GitHub, `!17` on GitLab.
    pub number: String,
    pub title: String,
    pub state: PrState,
}

#[derive(Clone, PartialEq, Debug)]
pub enum PrInfo {
    Unknown,
    None,
    Found(Pr),
    Error(String),
}

#[derive(Clone, PartialEq, Debug)]
pub struct FileStat {
    pub path: String,
    pub added: u32,
    pub removed: u32,
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct Changes {
    pub files: Vec<FileStat>,
    pub total_files: usize,
    pub added: u32,
    pub removed: u32,
    pub unpushed: Option<u32>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Spec {
    pub change: String,
    pub done: u32,
    pub total: u32,
    pub has_tasks: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Step {
    Spec,
    Tasks(u32, u32),
    Pr,
    Review,
    Merge,
}

impl Step {
    pub fn label(self) -> String {
        match self {
            Step::Spec => "Spec".into(),
            Step::Tasks(d, t) => format!("Tareas {d}/{t}"),
            Step::Pr => "PR".into(),
            Step::Review => "Review".into(),
            Step::Merge => "Merge".into(),
        }
    }

    pub fn index(self) -> usize {
        match self {
            Step::Spec => 0,
            Step::Tasks(..) => 1,
            Step::Pr => 2,
            Step::Review => 3,
            Step::Merge => 4,
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Context {
    pub repo: PathBuf,
    pub branch: Option<String>,
    pub spec: Option<Spec>,
    pub changes: Option<Changes>,
    pub pr: PrInfo,
    pub provider: Option<Provider>,
}

impl Context {
    pub fn step(&self) -> Step {
        step(self.spec.as_ref(), &self.pr)
    }
}

pub fn step(spec: Option<&Spec>, pr: &PrInfo) -> Step {
    match pr {
        PrInfo::Found(p) if p.state == PrState::Merged => Step::Merge,
        PrInfo::Found(p) if p.state != PrState::Closed => Step::Review,
        _ => match spec {
            Some(s) if !s.has_tasks => Step::Spec,
            Some(s) if s.done < s.total => Step::Tasks(s.done, s.total),
            _ => Step::Pr,
        },
    }
}

pub fn repo_root(dir: &Path) -> Option<PathBuf> {
    dir.ancestors().find(|d| d.join(".git").exists()).map(Path::to_path_buf)
}

/// Branch name from `.git/HEAD`; follows the `gitdir:` file of worktrees.
pub fn branch(repo: &Path) -> Option<String> {
    let dot_git = repo.join(".git");
    let git_dir = if dot_git.is_file() {
        let text = std::fs::read_to_string(&dot_git).ok()?;
        let dir = PathBuf::from(text.trim().strip_prefix("gitdir:")?.trim());
        if dir.is_absolute() { dir } else { repo.join(dir) }
    } else {
        dot_git
    };
    parse_head(&std::fs::read_to_string(git_dir.join("HEAD")).ok()?)
}

pub fn parse_head(head: &str) -> Option<String> {
    let head = head.trim();
    match head.strip_prefix("ref: refs/heads/") {
        Some(branch) => Some(branch.to_string()),
        None if head.len() >= 7 => Some(head[..7].to_string()),
        None => None,
    }
}

/// Most recently modified change under `openspec/changes/` (not `archive`).
pub fn openspec(repo: &Path) -> Option<Spec> {
    let changes = repo.join("openspec/changes");
    let newest = std::fs::read_dir(changes)
        .ok()?
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()) && e.file_name() != "archive")
        .max_by_key(|e| newest_mtime(&e.path()))?;
    let tasks = std::fs::read_to_string(newest.path().join("tasks.md")).ok();
    let (done, total) = tasks.as_deref().map(count_tasks).unwrap_or((0, 0));
    Some(Spec { change: newest.file_name().to_string_lossy().into_owned(), done, total, has_tasks: tasks.is_some() })
}

fn newest_mtime(dir: &Path) -> std::time::SystemTime {
    let own = std::fs::metadata(dir).and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH);
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.metadata().and_then(|m| m.modified()).ok())
        .fold(own, std::cmp::max)
}

pub fn count_tasks(md: &str) -> (u32, u32) {
    md.lines().map(str::trim_start).fold((0, 0), |(d, t), l| {
        if l.starts_with("- [x]") || l.starts_with("- [X]") {
            (d + 1, t + 1)
        } else if l.starts_with("- [ ]") {
            (d, t + 1)
        } else {
            (d, t)
        }
    })
}

pub fn provider(remote: &str) -> Option<Provider> {
    let r = remote.to_lowercase();
    if r.contains("github") {
        Some(Provider::GitHub)
    } else if r.contains("gitlab") {
        Some(Provider::GitLab)
    } else {
        None
    }
}

/// `git diff --numstat` output; binary files (`-`) count as 0/0.
pub fn parse_numstat(out: &str) -> Changes {
    let mut c = Changes::default();
    for line in out.lines() {
        let mut parts = line.splitn(3, '\t');
        let (Some(a), Some(r), Some(path)) = (parts.next(), parts.next(), parts.next()) else { continue };
        let (added, removed) = (a.parse().unwrap_or(0), r.parse().unwrap_or(0));
        c.added += added;
        c.removed += removed;
        c.total_files += 1;
        c.files.push(FileStat { path: path.to_string(), added, removed });
    }
    c.files.sort_by_key(|f| std::cmp::Reverse(f.added + f.removed));
    c.files.truncate(30);
    c
}

pub fn parse_gh(json: &str) -> Option<Pr> {
    let v: Value = serde_json::from_str(json).ok()?;
    let state = match (v["state"].as_str()?, v["isDraft"].as_bool().unwrap_or(false), v["reviewDecision"].as_str()) {
        ("MERGED", ..) => PrState::Merged,
        ("CLOSED", ..) => PrState::Closed,
        (_, true, _) => PrState::Draft,
        (_, _, Some("APPROVED")) => PrState::Approved,
        (_, _, Some("CHANGES_REQUESTED")) => PrState::ChangesRequested,
        _ => PrState::Open,
    };
    Some(Pr { number: format!("#{}", v["number"].as_u64()?), title: v["title"].as_str()?.to_string(), state })
}

pub fn parse_glab(json: &str) -> Option<Pr> {
    let v: Value = serde_json::from_str(json).ok()?;
    let draft = v["draft"].as_bool().or(v["work_in_progress"].as_bool()).unwrap_or(false);
    let state = match v["state"].as_str()? {
        "merged" => PrState::Merged,
        "closed" | "locked" => PrState::Closed,
        _ if draft => PrState::Draft,
        _ => PrState::Open,
    };
    Some(Pr { number: format!("!{}", v["iid"].as_u64()?), title: v["title"].as_str()?.to_string(), state })
}

/// One line for a permission request: tool plus its main argument.
pub fn permission_summary(hook: &Value) -> Option<String> {
    let tool = hook["tool_name"].as_str()?;
    let input = &hook["tool_input"];
    let arg = ["command", "file_path", "url", "pattern", "path", "description", "prompt"]
        .iter()
        .find_map(|k| input[*k].as_str())
        .unwrap_or("");
    let arg: String = arg.lines().next().unwrap_or("").chars().take(160).collect();
    Some(if arg.is_empty() { tool.to_string() } else { format!("{tool} · {arg}") })
}

/// Last assistant text in a Claude Code transcript (reads only the tail).
pub fn last_assistant_text(path: &Path) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(len.saturating_sub(256 * 1024))).ok()?;
    let mut tail = String::new();
    let mut bytes = vec![];
    file.read_to_end(&mut bytes).ok()?;
    tail.push_str(&String::from_utf8_lossy(&bytes));
    last_text_in(&tail)
}

pub fn last_text_in(jsonl: &str) -> Option<String> {
    jsonl.lines().rev().find_map(|line| {
        let v: Value = serde_json::from_str(line).ok()?;
        if v["type"] != "assistant" {
            return None;
        }
        let text: Vec<&str> = v["message"]["content"]
            .as_array()?
            .iter()
            .filter(|c| c["type"] == "text")
            .filter_map(|c| c["text"].as_str())
            .collect();
        let text = text.join("\n").trim().to_string();
        (!text.is_empty()).then_some(text)
    })
}

/// Run a command with stdin closed and a timeout; `Err` carries a short reason.
fn run(program: &str, args: &[&str], dir: &Path, timeout: Duration) -> Result<String, String> {
    let mut child = Command::new(program)
        .args(args)
        .current_dir(dir)
        .env("GH_PROMPT_DISABLED", "1")
        .env("NO_PROMPT", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound { format!("{program} no está instalado") } else { e.to_string() }
        })?;
    let end = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < end => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                return Err(format!("{program}: tiempo agotado"));
            }
        }
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).lines().next().unwrap_or("error").to_string())
    }
}

fn git(repo: &Path, args: &[&str]) -> Option<String> {
    run("git", args, repo, Duration::from_secs(5)).ok()
}

fn changes(repo: &Path) -> Option<Changes> {
    let base = ["origin/HEAD", "origin/main", "origin/master"]
        .iter()
        .find_map(|b| git(repo, &["merge-base", "HEAD", b]))
        .map(|s| s.trim().to_string());
    // Working tree against the merge base, so uncommitted work counts too.
    let numstat = match &base {
        Some(b) => git(repo, &["diff", "--numstat", b])?,
        None => git(repo, &["diff", "--numstat", "HEAD"])?,
    };
    let mut c = parse_numstat(&numstat);
    c.unpushed = git(repo, &["rev-list", "--count", "@{u}..HEAD"]).and_then(|s| s.trim().parse().ok());
    Some(c)
}

fn pull_request(repo: &Path, provider: Option<Provider>) -> PrInfo {
    let timeout = Duration::from_secs(15);
    match provider {
        Some(Provider::GitHub) => {
            match run("gh", &["pr", "view", "--json", "number,title,state,isDraft,reviewDecision"], repo, timeout) {
                Ok(json) => parse_gh(&json).map_or(PrInfo::Error("respuesta de gh no válida".into()), PrInfo::Found),
                Err(e) if e.contains("no pull requests found") => PrInfo::None,
                Err(e) => PrInfo::Error(e),
            }
        }
        Some(Provider::GitLab) => match run("glab", &["mr", "view", "-F", "json"], repo, timeout) {
            Ok(json) => parse_glab(&json).map_or(PrInfo::Error("respuesta de glab no válida".into()), PrInfo::Found),
            Err(e) if e.to_lowercase().contains("no open merge request") || e.contains("404") => PrInfo::None,
            Err(e) => PrInfo::Error(e),
        },
        None => PrInfo::Unknown,
    }
}

pub enum Request {
    /// Current panes and their shell pids.
    Panes(Vec<(PaneId, u32)>),
    /// Recompute git data for this pane's repo now (a Claude turn ended).
    Refresh(PaneId),
    /// Read Claude's last message from this transcript.
    Transcript(PaneId, PathBuf),
}

pub enum Update {
    Context(PaneId, Box<Context>),
    LastMessage(PaneId, String),
}

struct RepoState {
    git_at: Option<Instant>,
    pr_at: Option<(Instant, Option<String>)>,
    ctx: Option<Context>,
}

const GIT_EVERY: Duration = Duration::from_secs(5);
const PR_EVERY: Duration = Duration::from_secs(120);

/// Start the worker; `send` delivers updates to the UI (returns false to stop).
pub fn spawn(send: impl Fn(Update) -> bool + Send + 'static, cwd_of: fn(u32) -> Option<PathBuf>) -> Sender<Request> {
    let (tx, rx) = channel();
    std::thread::Builder::new()
        .name("context".into())
        .spawn(move || worker(rx, send, cwd_of))
        .expect("spawn context thread");
    tx
}

fn worker(rx: Receiver<Request>, send: impl Fn(Update) -> bool, cwd_of: fn(u32) -> Option<PathBuf>) {
    let mut panes: Vec<(PaneId, u32)> = vec![];
    let mut repos: HashMap<PathBuf, RepoState> = HashMap::new();
    let mut urgent: Vec<PaneId> = vec![];
    loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(Request::Panes(p)) => panes = p,
            Ok(Request::Refresh(id)) => urgent.push(id),
            Ok(Request::Transcript(id, path)) => {
                if let Some(text) = last_assistant_text(&path) {
                    if !send(Update::LastMessage(id, text)) {
                        return;
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => return,
        }
        // Group panes by repository; panes outside git get nothing.
        let mut by_repo: HashMap<PathBuf, Vec<PaneId>> = HashMap::new();
        for &(id, pid) in &panes {
            if let Some(root) = cwd_of(pid).as_deref().and_then(repo_root) {
                by_repo.entry(root).or_default().push(id);
            }
        }
        repos.retain(|r, _| by_repo.contains_key(r));
        for (repo, ids) in by_repo {
            let st = repos.entry(repo.clone()).or_insert(RepoState { git_at: None, pr_at: None, ctx: None });
            let forced = ids.iter().any(|id| urgent.contains(id));
            let git_due = forced || st.git_at.is_none_or(|t| t.elapsed() >= GIT_EVERY);
            let branch_now = branch(&repo);
            let pr_due = forced
                || st.pr_at.as_ref().is_none_or(|(t, b)| t.elapsed() >= PR_EVERY || *b != branch_now);
            if !git_due && !pr_due {
                continue;
            }
            let remote = git(&repo, &["remote", "get-url", "origin"]).unwrap_or_default();
            let prov = provider(&remote);
            let prev_pr = st.ctx.as_ref().map_or(PrInfo::Unknown, |c| c.pr.clone());
            let pr = if pr_due {
                st.pr_at = Some((Instant::now(), branch_now.clone()));
                pull_request(&repo, prov)
            } else {
                prev_pr
            };
            let ctx = Context {
                repo: repo.clone(),
                branch: branch_now,
                spec: openspec(&repo),
                changes: changes(&repo),
                pr,
                provider: prov,
            };
            st.git_at = Some(Instant::now());
            if st.ctx.as_ref() != Some(&ctx) {
                st.ctx = Some(ctx.clone());
                for id in &ids {
                    if !send(Update::Context(*id, Box::new(ctx.clone()))) {
                        return;
                    }
                }
            }
        }
        urgent.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(done: u32, total: u32, has_tasks: bool) -> Spec {
        Spec { change: "x".into(), done, total, has_tasks }
    }

    fn pr(state: PrState) -> PrInfo {
        PrInfo::Found(Pr { number: "#1".into(), title: "t".into(), state })
    }

    #[test]
    fn pipeline() {
        assert_eq!(step(Some(&spec(0, 0, false)), &PrInfo::None), Step::Spec);
        assert_eq!(step(Some(&spec(7, 12, true)), &PrInfo::None), Step::Tasks(7, 12));
        assert_eq!(step(Some(&spec(12, 12, true)), &PrInfo::None), Step::Pr);
        assert_eq!(step(None, &PrInfo::None), Step::Pr);
        assert_eq!(step(Some(&spec(3, 12, true)), &pr(PrState::Open)), Step::Review);
        assert_eq!(step(None, &pr(PrState::Merged)), Step::Merge);
        assert_eq!(step(None, &pr(PrState::Closed)), Step::Pr);
    }

    #[test]
    fn head_and_tasks() {
        assert_eq!(parse_head("ref: refs/heads/feat/x\n"), Some("feat/x".into()));
        assert_eq!(parse_head("0a09939abcdef"), Some("0a09939".into()));
        assert_eq!(count_tasks("## 1\n- [x] 1.1 a\n  - [ ] 1.2 b\n- [X] c\ntext"), (2, 3));
    }

    #[test]
    fn this_repo() {
        let repo = repo_root(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        assert!(branch(&repo).is_some());
        assert!(openspec(&repo).is_some());
    }

    #[test]
    fn providers_and_numstat() {
        assert_eq!(provider("git@github.com:a/b.git"), Some(Provider::GitHub));
        assert_eq!(provider("https://gitlab.toku.cl/team/app.git"), Some(Provider::GitLab));
        assert_eq!(provider("ssh://git@example.com/x"), None);
        let c = parse_numstat("10\t2\tsrc/a.rs\n-\t-\tlogo.png\n100\t0\tsrc/b.rs\n");
        assert_eq!((c.total_files, c.added, c.removed), (3, 110, 2));
        assert_eq!(c.files[0].path, "src/b.rs");
    }

    #[test]
    fn pr_json() {
        let gh = r#"{"isDraft":false,"number":42,"reviewDecision":"CHANGES_REQUESTED","state":"OPEN","title":"Add panel"}"#;
        assert_eq!(parse_gh(gh).unwrap(), Pr { number: "#42".into(), title: "Add panel".into(), state: PrState::ChangesRequested });
        let gh = r#"{"isDraft":true,"number":7,"reviewDecision":"","state":"OPEN","title":"wip"}"#;
        assert_eq!(parse_gh(gh).unwrap().state, PrState::Draft);
        let gl = r#"{"iid":17,"title":"Fix","state":"opened","draft":false}"#;
        assert_eq!(parse_glab(gl).unwrap(), Pr { number: "!17".into(), title: "Fix".into(), state: PrState::Open });
        assert_eq!(parse_glab(r#"{"iid":3,"title":"x","state":"merged"}"#).unwrap().state, PrState::Merged);
    }

    #[test]
    fn hook_and_transcript() {
        let hook: Value = serde_json::from_str(r#"{"tool_name":"Bash","tool_input":{"command":"git push origin main\nmore"}}"#).unwrap();
        assert_eq!(permission_summary(&hook).unwrap(), "Bash · git push origin main");
        let jsonl = [
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"first"}]}}"#,
            r#"{"type":"user","message":{"content":"hi"}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"thinking","thinking":"x"},{"type":"text","text":"Listo. ¿Hago el PR?"}]}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash"}]}}"#,
        ]
        .join("\n");
        assert_eq!(last_text_in(&jsonl).unwrap(), "Listo. ¿Hago el PR?");
    }

}
