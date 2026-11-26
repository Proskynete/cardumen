//! What survives a relaunch: tabs, names, split trees, directories, focus, and the
//! sidebar/panel flags. Pure conversion to and from `Tabs`, plus atomic file I/O.
//! Shells are not kept: each restored pane starts a new one.
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::layout::{Dir, Node, PaneId, Tab, Tabs};

const VERSION: u32 = 1;

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Saved {
    pub version: u32,
    pub tabs: Vec<SavedTab>,
    pub active: usize,
    pub sidebar: bool,
    pub panel: bool,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct SavedTab {
    #[serde(default)]
    pub name: Option<String>,
    /// Index of the focused pane among the tab's leaves, left to right.
    #[serde(default)]
    pub focused: usize,
    pub root: SavedNode,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum SavedNode {
    Pane { cwd: Option<String> },
    Split { dir: Dir, children: Vec<SavedNode> },
}

/// `~/Library/Application Support/cardumen/session.json`.
pub fn path() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support/cardumen/session.json"))
}

pub fn capture(tabs: &Tabs, cwd: impl Fn(PaneId) -> Option<PathBuf>, sidebar: bool, panel: bool) -> Saved {
    fn node(n: &Node, cwd: &dyn Fn(PaneId) -> Option<PathBuf>) -> SavedNode {
        match n {
            Node::Leaf(id) => SavedNode::Pane { cwd: cwd(*id).map(|p| p.to_string_lossy().into_owned()) },
            Node::Split { dir, children } => {
                SavedNode::Split { dir: *dir, children: children.iter().map(|c| node(c, cwd)).collect() }
            }
        }
    }
    let tabs_out = tabs
        .list
        .iter()
        .map(|t| SavedTab {
            name: t.name.clone(),
            focused: t.root.leaves().iter().position(|&p| p == t.focused).unwrap_or(0),
            root: node(&t.root, &cwd),
        })
        .collect();
    Saved { version: VERSION, tabs: tabs_out, active: tabs.active, sidebar, panel }
}

/// Build tabs with fresh pane ids taken from `next_id` (pre-incremented, like
/// `State::take_id`). Also returns each pane's start directory, in tab then leaf
/// order; a directory that no longer exists becomes `home`. Out-of-range indices
/// are clamped and empty splits dropped.
pub fn rebuild(saved: &Saved, next_id: &mut PaneId, home: Option<&Path>) -> (Tabs, Vec<(PaneId, Option<PathBuf>)>) {
    fn node(n: &SavedNode, next_id: &mut PaneId, home: Option<&Path>, panes: &mut Vec<(PaneId, Option<PathBuf>)>) -> Option<Node> {
        match n {
            SavedNode::Pane { cwd } => {
                *next_id += 1;
                panes.push((*next_id, resolve_dir(cwd.as_deref(), home)));
                Some(Node::Leaf(*next_id))
            }
            SavedNode::Split { dir, children } => {
                let mut kids: Vec<Node> = children.iter().filter_map(|c| node(c, next_id, home, panes)).collect();
                match kids.len() {
                    0 => None,
                    1 => kids.pop(),
                    _ => Some(Node::Split { dir: *dir, children: kids }),
                }
            }
        }
    }
    let (mut list, mut panes) = (vec![], vec![]);
    for t in &saved.tabs {
        let Some(root) = node(&t.root, next_id, home, &mut panes) else { continue };
        let leaves = root.leaves();
        let focused = leaves[t.focused.min(leaves.len() - 1)];
        let name = t.name.clone().filter(|n| !n.trim().is_empty());
        list.push(Tab { id: leaves[0], root, focused, name });
    }
    let active = saved.active.min(list.len().saturating_sub(1));
    (Tabs { list, active }, panes)
}

pub fn resolve_dir(cwd: Option<&str>, home: Option<&Path>) -> Option<PathBuf> {
    cwd.map(PathBuf::from).filter(|d| d.is_dir()).or_else(|| home.map(Path::to_path_buf))
}

/// `None` for invalid JSON, another version, or no tabs.
pub fn parse(text: &str) -> Option<Saved> {
    serde_json::from_str::<Saved>(text).ok().filter(|s| s.version == VERSION && !s.tabs.is_empty())
}

pub fn load(path: &Path) -> Option<Saved> {
    parse(&std::fs::read_to_string(path).ok()?)
}

/// Write atomically (temp file + rename). With no tabs, delete the file instead.
pub fn store(path: &Path, saved: &Saved) -> std::io::Result<()> {
    if saved.tabs.is_empty() {
        return match std::fs::remove_file(path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        };
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(saved)?)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Tabs {
        let mut t = Tabs::default();
        t.open(1);
        t.active_mut().root.split(1, 2, Dir::Right);
        t.active_mut().root.split(2, 3, Dir::Down);
        t.active_mut().focused = 3;
        t.active_mut().name = Some("api".into());
        t.open(4);
        t.select(0);
        t
    }

    #[test]
    fn round_trip() {
        let tmp = std::env::temp_dir();
        let cwd = |id: PaneId| Some(if id == 4 { tmp.clone() } else { PathBuf::from("/") });
        let saved = capture(&sample(), cwd, false, true);
        let parsed = parse(&serde_json::to_string(&saved).unwrap()).unwrap();
        assert_eq!(parsed, saved);

        let mut next = 10;
        let (tabs, panes) = rebuild(&parsed, &mut next, None);
        assert_eq!(next, 14);
        assert_eq!(tabs.active, 0);
        assert_eq!(tabs.list.len(), 2);
        let a = &tabs.list[0];
        assert_eq!(a.name.as_deref(), Some("api"));
        assert_eq!(a.root.leaves(), [11, 12, 13]);
        assert_eq!(a.focused, 13);
        assert_eq!(a.id, 11);
        assert_eq!(panes, [(11, Some("/".into())), (12, Some("/".into())), (13, Some("/".into())), (14, Some(tmp))]);
        // Same shape, new ids: capturing again gives the same file.
        let again = capture(&tabs, |id| panes.iter().find(|p| p.0 == id).and_then(|p| p.1.clone()), false, true);
        assert_eq!(again, saved);
    }

    #[test]
    fn invalid_files() {
        assert_eq!(parse("not json"), None);
        assert_eq!(parse(""), None);
        assert_eq!(parse(r#"{"version":1,"tabs":[],"active":0,"sidebar":true,"panel":true}"#), None);
        let mut saved = capture(&sample(), |_| None, true, true);
        saved.version = 99;
        assert_eq!(parse(&serde_json::to_string(&saved).unwrap()), None);
    }

    #[test]
    fn missing_dir_falls_back_to_home() {
        let home = Path::new("/home-for-test");
        assert_eq!(resolve_dir(Some("/no/such/dir/cardumen"), Some(home)), Some(home.into()));
        assert_eq!(resolve_dir(None, Some(home)), Some(home.into()));
        assert_eq!(resolve_dir(Some("/"), Some(home)), Some("/".into()));
    }

    #[test]
    fn indices_are_clamped_and_empty_splits_dropped() {
        let pane = || SavedNode::Pane { cwd: None };
        let saved = Saved {
            version: VERSION,
            tabs: vec![
                SavedTab { name: Some("  ".into()), focused: 5, root: SavedNode::Split { dir: Dir::Right, children: vec![pane(), pane()] } },
                SavedTab { name: None, focused: 0, root: SavedNode::Split { dir: Dir::Down, children: vec![] } },
                SavedTab {
                    name: None,
                    focused: 0,
                    root: SavedNode::Split { dir: Dir::Down, children: vec![SavedNode::Split { dir: Dir::Right, children: vec![] }, pane()] },
                },
            ],
            active: 9,
            sidebar: true,
            panel: false,
        };
        let mut next = 0;
        let (tabs, panes) = rebuild(&saved, &mut next, None);
        assert_eq!(tabs.list.len(), 2);
        assert_eq!(tabs.active, 1);
        assert_eq!(tabs.list[0].focused, 2);
        assert_eq!(tabs.list[0].name, None);
        assert_eq!(tabs.list[1].root, Node::Leaf(3));
        assert_eq!(panes.len(), 3);
    }

    #[test]
    fn store_and_load() {
        let dir = std::env::temp_dir().join(format!("cardumen-session-test-{}", std::process::id()));
        let file = dir.join("session.json");
        let saved = capture(&sample(), |_| None, true, false);
        store(&file, &saved).unwrap();
        assert_eq!(load(&file), Some(saved));
        assert!(!file.with_extension("json.tmp").exists());
        std::fs::write(&file, "{garbage").unwrap();
        assert_eq!(load(&file), None);
        store(&file, &capture(&Tabs::default(), |_| None, true, true)).unwrap();
        assert!(!file.exists());
        store(&file, &capture(&Tabs::default(), |_| None, true, true)).unwrap(); // already gone: fine
        let _ = std::fs::remove_dir_all(&dir);
    }
}
