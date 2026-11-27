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
    /// Web page of the PR/MR; empty when the CLI did not report one.
    pub url: String,
}

/// Open PRs/MRs of the repository: the newest few, plus how many there are.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct OpenPrs {
    pub list: Vec<Pr>,
    /// Capped at `PR_LIST_LIMIT`.
    pub total: usize,
}

/// Open PRs fetched to count them; at this many the panel shows "100+".
pub const PR_LIST_LIMIT: usize = 100;
/// Open PRs shown in the panel.
pub const PR_LIST_SHOWN: usize = 3;

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
    pub open: OpenPrs,
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
    gh_pr(&serde_json::from_str(json).ok()?)
}

pub fn parse_glab(json: &str) -> Option<Pr> {
    glab_mr(&serde_json::from_str(json).ok()?)
}

/// A `gh pr list` / `glab mr list` JSON array: the newest `PR_LIST_SHOWN` and the count.
pub fn parse_list(json: &str, item: fn(&Value) -> Option<Pr>) -> Option<OpenPrs> {
    let v: Value = serde_json::from_str(json).ok()?;
    let all = v.as_array()?;
    let list = all.iter().filter_map(item).take(PR_LIST_SHOWN).collect();
    Some(OpenPrs { list, total: all.len() })
}

fn gh_pr(v: &Value) -> Option<Pr> {
    let state = match (v["state"].as_str()?, v["isDraft"].as_bool().unwrap_or(false), v["reviewDecision"].as_str()) {
        ("MERGED", ..) => PrState::Merged,
        ("CLOSED", ..) => PrState::Closed,
        (_, true, _) => PrState::Draft,
        (_, _, Some("APPROVED")) => PrState::Approved,
        (_, _, Some("CHANGES_REQUESTED")) => PrState::ChangesRequested,
        _ => PrState::Open,
    };
    Some(Pr { number: format!("#{}", v["number"].as_u64()?), title: v["title"].as_str()?.to_string(), state, url: v["url"].as_str().unwrap_or_default().into() })
}

fn glab_mr(v: &Value) -> Option<Pr> {
    let draft = v["draft"].as_bool().or(v["work_in_progress"].as_bool()).unwrap_or(false);
    let state = match v["state"].as_str()? {
        "merged" => PrState::Merged,
        "closed" | "locked" => PrState::Closed,
        _ if draft => PrState::Draft,
        _ => PrState::Open,
    };
    Some(Pr { number: format!("!{}", v["iid"].as_u64()?), title: v["title"].as_str()?.to_string(), state, url: v["web_url"].as_str().unwrap_or_default().into() })
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
/// `PATH` plus Homebrew's bin dirs: launched from Finder, the app gets only
/// `/usr/bin:/bin:/usr/sbin:/sbin`, where `gh` and `glab` are not.
fn tool_path(current: Option<&str>) -> String {
    let mut dirs: Vec<&str> = current.unwrap_or("").split(':').filter(|d| !d.is_empty()).collect();
    for extra in ["/opt/homebrew/bin", "/usr/local/bin"] {
        if !dirs.contains(&extra) {
            dirs.push(extra);
        }
    }
    dirs.join(":")
}

fn run(program: &str, args: &[&str], dir: &Path, timeout: Duration) -> Result<String, String> {
    let mut child = Command::new(program)
        .args(args)
        .env("PATH", tool_path(std::env::var("PATH").ok().as_deref()))
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

/// What changes are measured against: the merge base with the remote default branch, else `HEAD`.
pub fn diff_base(repo: &Path) -> String {
    ["origin/HEAD", "origin/main", "origin/master"]
        .iter()
        .find_map(|b| git(repo, &["merge-base", "HEAD", b]))
        .map_or_else(|| "HEAD".to_string(), |s| s.trim().to_string())
}

fn changes(repo: &Path) -> Option<Changes> {
    // Working tree against the merge base, so uncommitted work counts too.
    let numstat = git(repo, &["diff", "--numstat", &diff_base(repo)])?;
    let mut c = parse_numstat(&numstat);
    c.unpushed = git(repo, &["rev-list", "--count", "@{u}..HEAD"]).and_then(|s| s.trim().parse().ok());
    Some(c)
}

fn pull_request(repo: &Path, provider: Option<Provider>) -> PrInfo {
    let timeout = Duration::from_secs(15);
    match provider {
        Some(Provider::GitHub) => {
            match run("gh", &["pr", "view", "--json", "number,title,state,isDraft,reviewDecision,url"], repo, timeout) {
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

/// Open PRs/MRs of the repo, newest first; empty when the CLI is missing or fails.
fn open_prs(repo: &Path, provider: Option<Provider>) -> OpenPrs {
    let timeout = Duration::from_secs(15);
    let limit = PR_LIST_LIMIT.to_string();
    let (out, item): (_, fn(&Value) -> Option<Pr>) = match provider {
        Some(Provider::GitHub) => (
            run("gh", &["pr", "list", "--state", "open", "--limit", &limit, "--json", "number,title,state,isDraft,reviewDecision,url"], repo, timeout),
            gh_pr,
        ),
        Some(Provider::GitLab) => (run("glab", &["mr", "list", "-F", "json", "--per-page", &limit], repo, timeout), glab_mr),
        None => return OpenPrs::default(),
    };
    out.ok().and_then(|json| parse_list(&json, item)).unwrap_or_default()
}

pub enum Request {
    /// Current panes and their shell pids.
    Panes(Vec<(PaneId, u32)>),
    /// Recompute git data for this pane's repo now (a Claude turn ended).
    Refresh(PaneId),
    /// Panes on screen (the active tab); only these are polled.
    Visible(Vec<PaneId>),
    /// Read Claude's last message from this transcript.
    Transcript(PaneId, PathBuf),
}

pub enum Update {
    Context(PaneId, Box<Context>),
    LastMessage(PaneId, String),
}

struct RepoState {
    /// Panes that already received the current `ctx`.
    sent: Vec<PaneId>,
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
    let mut visible: Vec<PaneId> = vec![];
    loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(Request::Panes(p)) => panes = p,
            Ok(Request::Refresh(id)) => urgent.push(id),
            Ok(Request::Visible(v)) => visible = v,
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
        // Group on-screen (or just-finished) panes by repository; panes outside
        // git get nothing. Hidden panes keep their last context until shown.
        let mut by_repo: HashMap<PathBuf, Vec<PaneId>> = HashMap::new();
        for &(id, pid) in panes.iter().filter(|(id, _)| visible.contains(id) || urgent.contains(id)) {
            if let Some(root) = cwd_of(pid).as_deref().and_then(repo_root) {
                by_repo.entry(root).or_default().push(id);
            }
        }
        // Repos of hidden panes keep their state, so showing them again does not
        // refetch the PR at once. shortcut: one entry per repo seen this session.
        for (repo, ids) in by_repo {
            let st = repos.entry(repo.clone()).or_insert(RepoState { sent: vec![], git_at: None, pr_at: None, ctx: None });
            // A pane shown again gets the cached context right away.
            if let Some(ctx) = &st.ctx {
                let new: Vec<PaneId> = ids.iter().copied().filter(|id| !st.sent.contains(id)).collect();
                for id in new {
                    if !send(Update::Context(id, Box::new(ctx.clone()))) {
                        return;
                    }
                    st.sent.push(id);
                }
            }
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
            let prev = st.ctx.as_ref().map_or((PrInfo::Unknown, OpenPrs::default()), |c| (c.pr.clone(), c.open.clone()));
            let (pr, open) = if pr_due {
                st.pr_at = Some((Instant::now(), branch_now.clone()));
                (pull_request(&repo, prov), open_prs(&repo, prov))
            } else {
                prev
            };
            let ctx = Context {
                repo: repo.clone(),
                branch: branch_now,
                spec: openspec(&repo),
                changes: changes(&repo),
                pr,
                open,
                provider: prov,
            };
            st.git_at = Some(Instant::now());
            if st.ctx.as_ref() != Some(&ctx) {
                st.ctx = Some(ctx.clone());
                st.sent.clear();
                for id in &ids {
                    if !send(Update::Context(*id, Box::new(ctx.clone()))) {
                        return;
                    }
                    st.sent.push(*id);
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
        PrInfo::Found(Pr { number: "#1".into(), title: "t".into(), state, url: String::new() })
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
    fn tool_path_adds_homebrew() {
        assert_eq!(tool_path(Some("/usr/bin:/bin")), "/usr/bin:/bin:/opt/homebrew/bin:/usr/local/bin");
        assert_eq!(tool_path(Some("/opt/homebrew/bin:/usr/bin")), "/opt/homebrew/bin:/usr/bin:/usr/local/bin");
        assert_eq!(tool_path(None), "/opt/homebrew/bin:/usr/local/bin");
    }

    #[test]
    fn polls_only_visible_panes() {
        static ROOT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
        let root = ROOT.get_or_init(|| std::env::temp_dir().join(format!("cardumen-visible-{}", std::process::id())));
        std::fs::create_dir_all(root).unwrap();
        assert!(Command::new("git").args(["init", "-q"]).current_dir(root).status().unwrap().success());
        fn cwd(_: u32) -> Option<PathBuf> {
            ROOT.get().cloned()
        }
        let (out_tx, out) = channel();
        let tx = spawn(move |u| out_tx.send(u).is_ok(), cwd);
        let next = || match out.recv_timeout(Duration::from_secs(5)) {
            Ok(Update::Context(id, _)) => Some(id),
            _ => None,
        };
        tx.send(Request::Panes(vec![(1, 0), (2, 0)])).unwrap();
        tx.send(Request::Visible(vec![1])).unwrap();
        assert_eq!(next(), Some(1));
        // Pane 2 (same repo) is hidden: nothing for it until it is shown.
        assert_eq!(out.recv_timeout(Duration::from_millis(1500)).ok().map(|_| ()), None);
        tx.send(Request::Visible(vec![2])).unwrap();
        assert_eq!(next(), Some(2));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn this_repo() {
        let repo = repo_root(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        assert!(branch(&repo).is_some());
    }

    #[test]
    fn openspec_skips_archive() {
        // Fixture dir, not this repo: its active changes come and go.
        let root = std::env::temp_dir().join(format!("cardumen-openspec-{}", std::process::id()));
        let changes = root.join("openspec/changes");
        std::fs::create_dir_all(changes.join("archive/old")).unwrap();
        std::fs::create_dir_all(changes.join("feat-x")).unwrap();
        std::fs::write(changes.join("feat-x/tasks.md"), "- [x] a\n- [ ] b\n").unwrap();
        let spec = openspec(&root).unwrap();
        assert_eq!((spec.change.as_str(), spec.done, spec.total), ("feat-x", 1, 2));
        std::fs::remove_dir_all(changes.join("feat-x")).unwrap();
        assert!(openspec(&root).is_none());
        std::fs::remove_dir_all(&root).unwrap();
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
        let gh = r#"{"isDraft":false,"number":42,"reviewDecision":"CHANGES_REQUESTED","state":"OPEN","title":"Add panel","url":"https://github.com/a/b/pull/42"}"#;
        let url = "https://github.com/a/b/pull/42".to_string();
        assert_eq!(parse_gh(gh).unwrap(), Pr { number: "#42".into(), title: "Add panel".into(), state: PrState::ChangesRequested, url });
        let gh = r#"{"isDraft":true,"number":7,"reviewDecision":"","state":"OPEN","title":"wip"}"#;
        assert_eq!(parse_gh(gh).unwrap().state, PrState::Draft);
        let gl = r#"{"iid":17,"title":"Fix","state":"opened","draft":false,"web_url":"https://gl.dev/a/-/merge_requests/17"}"#;
        assert_eq!(parse_glab(gl).unwrap().url, "https://gl.dev/a/-/merge_requests/17");
        assert_eq!(parse_glab(gl).unwrap().state, PrState::Open);
        assert_eq!(parse_glab(r#"{"iid":3,"title":"x","state":"merged"}"#).unwrap().state, PrState::Merged);
    }

    #[test]
    fn pr_lists_keep_newest_three_and_count() {
        let item = |n: u32| format!(r#"{{"number":{n},"title":"t{n}","state":"OPEN","isDraft":false,"reviewDecision":""}}"#);
        let json = format!("[{}]", (1..=5).rev().map(item).collect::<Vec<_>>().join(","));
        let open = parse_list(&json, gh_pr).unwrap();
        assert_eq!(open.total, 5);
        assert_eq!(open.list.iter().map(|p| p.number.as_str()).collect::<Vec<_>>(), ["#5", "#4", "#3"]);
        let gl = r#"[{"iid":9,"title":"a","state":"opened"},{"iid":8,"title":"b","state":"opened","draft":true}]"#;
        let open = parse_list(gl, glab_mr).unwrap();
        assert_eq!((open.total, open.list[1].state.clone()), (2, PrState::Draft));
        assert_eq!(parse_list("[]", gh_pr).unwrap(), OpenPrs::default());
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
