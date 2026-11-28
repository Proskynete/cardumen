//! Start an OpenSpec change in its own git worktree (`.claude/worktrees/<change>`).
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::context;

/// The main checkout's root, also from inside one of its worktrees (no `git` process).
pub fn main_root(dir: &Path) -> Option<PathBuf> {
    let root = context::repo_root(dir)?;
    let dot_git = root.join(".git");
    if dot_git.is_dir() {
        return Some(root);
    }
    // A worktree's `.git` file points to `<main>/.git/worktrees/<name>`.
    let text = std::fs::read_to_string(&dot_git).ok()?;
    let gitdir = root.join(text.trim().strip_prefix("gitdir:")?.trim());
    gitdir.ancestors().find(|a| a.file_name().is_some_and(|n| n == ".git"))?.parent().map(Path::to_path_buf)
}

/// Active changes (folders in `openspec/changes/` except `archive`), by name.
/// Names git could read as an option or that are not plain words are skipped.
pub fn changes(repo: &Path) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(repo.join("openspec/changes"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n != "archive" && valid_name(n))
        .collect();
    out.sort();
    out
}

fn valid_name(name: &str) -> bool {
    !name.starts_with(['-', '.']) && name.chars().all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

pub fn path(repo: &Path, change: &str) -> PathBuf {
    repo.join(".claude/worktrees").join(change)
}

fn git(repo: &Path, args: &[&str]) -> Result<String, String> {
    context::run("git", args, repo, Duration::from_secs(20))
}

/// Make sure `<repo>/.claude/worktrees/<change>` exists on branch `<change>`: reuse it,
/// else check out the branch, else create the branch from `HEAD`. Keeps the folder
/// out of `git status` and brings over a change folder that has no commit yet.
pub fn ensure(repo: &Path, change: &str) -> Result<PathBuf, String> {
    if !valid_name(change) {
        return Err(format!("nombre de change no válido: {change}"));
    }
    let dir = path(repo, change);
    let rel = format!(".claude/worktrees/{change}");
    if git(repo, &["check-ignore", "-q", &rel]).is_err() {
        let exclude = repo.join(".git/info/exclude");
        let old = std::fs::read_to_string(&exclude).unwrap_or_default();
        let sep = if old.is_empty() || old.ends_with('\n') { "" } else { "\n" };
        std::fs::create_dir_all(repo.join(".git/info")).and_then(|_| std::fs::write(&exclude, format!("{old}{sep}/.claude/worktrees/\n"))).map_err(|e| e.to_string())?;
    }
    if !dir.join(".git").exists() {
        let target = dir.to_string_lossy().into_owned();
        let branch_exists = git(repo, &["show-ref", "--verify", "--quiet", &format!("refs/heads/{change}")]).is_ok();
        if branch_exists {
            git(repo, &["worktree", "add", &target, change])?;
        } else {
            git(repo, &["worktree", "add", "-b", change, &target, "HEAD"])?;
        }
    }
    // A proposal not committed yet is not in `HEAD`: copy it (the original stays).
    let (src, dst) = (repo.join("openspec/changes").join(change), dir.join("openspec/changes").join(change));
    if src.is_dir() && !dst.exists() {
        copy_dir(&src, &dst).map_err(|e| e.to_string())?;
    }
    Ok(dir)
}

fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for e in std::fs::read_dir(src)? {
        let e = e?;
        let to = dst.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_dir(&e.path(), &to)?;
        } else {
            std::fs::copy(e.path(), to)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sh(dir: &Path, cmd: &str) -> String {
        let out = std::process::Command::new("sh").args(["-c", cmd]).current_dir(dir).output().unwrap();
        assert!(out.status.success(), "{cmd}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    #[test]
    fn creates_reuses_and_checks_out() {
        let repo = std::env::temp_dir().join(format!("cardumen-worktree-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&repo);
        std::fs::create_dir_all(repo.join("openspec/changes/archive")).unwrap();
        sh(&repo, "git init -q -b main && git -c user.email=t@t -c user.name=t commit -q --allow-empty -m init");
        // Not committed: only in the main checkout.
        std::fs::create_dir_all(repo.join("openspec/changes/nuevo")).unwrap();
        std::fs::write(repo.join("openspec/changes/nuevo/tasks.md"), "- [ ] 1.1 x\n").unwrap();
        std::fs::create_dir_all(repo.join("openspec/changes/-x")).unwrap();
        assert_eq!(changes(&repo), vec!["nuevo".to_string()]);

        // New branch from HEAD, change folder copied, folder excluded.
        let dir = ensure(&repo, "nuevo").unwrap();
        assert_eq!(sh(&dir, "git branch --show-current").trim(), "nuevo");
        assert!(dir.join("openspec/changes/nuevo/tasks.md").exists());
        assert!(repo.join("openspec/changes/nuevo/tasks.md").exists());
        assert!(!sh(&repo, "git status --porcelain").contains(".claude"));
        assert_eq!(std::fs::read_to_string(repo.join(".git/info/exclude")).unwrap().matches("/.claude/worktrees/").count(), 1);
        // From inside the worktree, the main checkout is found.
        assert_eq!(main_root(&dir.join("openspec")).unwrap().canonicalize().unwrap(), repo.canonicalize().unwrap());

        // Second time: reused, exclude line not repeated.
        assert_eq!(ensure(&repo, "nuevo").unwrap(), dir);
        assert_eq!(std::fs::read_to_string(repo.join(".git/info/exclude")).unwrap().matches("/.claude/worktrees/").count(), 1);

        // Existing branch without a worktree: checked out, not recreated.
        sh(&repo, "git branch otro");
        let other = ensure(&repo, "otro").unwrap();
        assert_eq!(sh(&other, "git branch --show-current").trim(), "otro");

        // Branch busy in another worktree: git's error comes back.
        sh(&repo, "git worktree remove --force .claude/worktrees/otro");
        sh(&dir, "git switch -q otro");
        assert!(ensure(&repo, "otro").is_err());
        assert!(ensure(&repo, "--bad").is_err());
        let _ = std::fs::remove_dir_all(&repo);
    }
}
