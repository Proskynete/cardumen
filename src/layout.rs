//! Pane trees and the tab list. Pure data: no sessions, no window.
pub type PaneId = u64;

/// `Right` puts children side by side; `Down` stacks them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dir {
    Right,
    Down,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Arrow {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum Node {
    Leaf(PaneId),
    Split { dir: Dir, children: Vec<Node> },
}

impl Node {
    /// Split `target`, putting `new` after it. Same-direction splits add a sibling,
    /// so repeated Cmd+D gives even columns instead of halving the last one.
    pub fn split(&mut self, target: PaneId, new: PaneId, dir: Dir) -> bool {
        match self {
            Node::Leaf(id) if *id == target => {
                *self = Node::Split { dir, children: vec![Node::Leaf(target), Node::Leaf(new)] };
                true
            }
            Node::Leaf(_) => false,
            Node::Split { dir: d, children } => {
                if *d == dir {
                    if let Some(i) = children.iter().position(|c| *c == Node::Leaf(target)) {
                        children.insert(i + 1, Node::Leaf(new));
                        return true;
                    }
                }
                children.iter_mut().any(|c| c.split(target, new, dir))
            }
        }
    }

    /// Remove a leaf; a split left with one child collapses into it.
    /// Removing the root leaf is the caller's job (the tab closes).
    pub fn remove(&mut self, target: PaneId) -> bool {
        let Node::Split { children, .. } = self else { return false };
        if let Some(i) = children.iter().position(|c| *c == Node::Leaf(target)) {
            children.remove(i);
        } else if !children.iter_mut().any(|c| c.remove(target)) {
            return false;
        }
        if children.len() == 1 {
            *self = children.pop().unwrap();
        }
        true
    }

    pub fn leaves(&self) -> Vec<PaneId> {
        match self {
            Node::Leaf(id) => vec![*id],
            Node::Split { children, .. } => children.iter().flat_map(Node::leaves).collect(),
        }
    }

    pub fn contains(&self, id: PaneId) -> bool {
        self.leaves().contains(&id)
    }

    /// Even split of `r` among children, with `divider` px between them, on whole pixels.
    /// Pushes pane rects into `panes` and divider rects into `dividers`.
    pub fn layout(&self, r: Rect, divider: f32, panes: &mut Vec<(PaneId, Rect)>, dividers: &mut Vec<Rect>) {
        let (dir, children) = match self {
            Node::Leaf(id) => return panes.push((*id, r)),
            Node::Split { dir, children } => (*dir, children),
        };
        let n = children.len() as f32;
        let (start, len) = if dir == Dir::Right { (r.x, r.w) } else { (r.y, r.h) };
        let space = len - divider * (n - 1.0);
        for (i, child) in children.iter().enumerate() {
            let i = i as f32;
            let a = (start + i * (space / n + divider)).round();
            let b = (start + i * divider + (i + 1.0) * space / n).round();
            let cr = if dir == Dir::Right { Rect { x: a, w: b - a, ..r } } else { Rect { y: a, h: b - a, ..r } };
            if i > 0.0 {
                dividers.push(if dir == Dir::Right {
                    Rect { x: a - divider, w: divider, ..r }
                } else {
                    Rect { y: a - divider, h: divider, ..r }
                });
            }
            child.layout(cr, divider, panes, dividers);
        }
    }
}

/// Nearest pane in `arrow` direction that overlaps `from` on the other axis.
pub fn neighbour(rects: &[(PaneId, Rect)], from: PaneId, arrow: Arrow) -> Option<PaneId> {
    let f = rects.iter().find(|(id, _)| *id == from)?.1;
    rects
        .iter()
        .filter(|(id, _)| *id != from)
        .filter_map(|(id, r)| {
            let overlap_y = r.y < f.y + f.h && f.y < r.y + r.h;
            let overlap_x = r.x < f.x + f.w && f.x < r.x + r.w;
            let gap = match arrow {
                Arrow::Left if overlap_y && r.x + r.w <= f.x + 1.0 => f.x - (r.x + r.w),
                Arrow::Right if overlap_y && r.x >= f.x + f.w - 1.0 => r.x - (f.x + f.w),
                Arrow::Up if overlap_x && r.y + r.h <= f.y + 1.0 => f.y - (r.y + r.h),
                Arrow::Down if overlap_x && r.y >= f.y + f.h - 1.0 => r.y - (f.y + f.h),
                _ => return None,
            };
            Some((gap, *id))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id)
}

pub struct Tab {
    /// Stable identity for animations (the first pane's id).
    pub id: u64,
    pub root: Node,
    pub focused: PaneId,
    /// Name set by the user; wins over any automatic title.
    pub name: Option<String>,
}

#[derive(Default)]
pub struct Tabs {
    pub list: Vec<Tab>,
    pub active: usize,
}

impl Tabs {
    pub fn active(&self) -> &Tab {
        &self.list[self.active]
    }

    pub fn active_mut(&mut self) -> &mut Tab {
        &mut self.list[self.active]
    }

    /// Insert a one-pane tab after the active one and select it.
    pub fn open(&mut self, pane: PaneId) {
        let at = if self.list.is_empty() { 0 } else { self.active + 1 };
        self.list.insert(at, Tab { id: pane, root: Node::Leaf(pane), focused: pane, name: None });
        self.active = at;
    }

    pub fn select(&mut self, i: usize) {
        if i < self.list.len() {
            self.active = i;
        }
    }

    /// Move tab `from` to slot `to`; the active tab stays the same tab.
    pub fn move_tab(&mut self, from: usize, to: usize) {
        let n = self.list.len();
        if from >= n || to >= n || from == to {
            return;
        }
        let tab = self.list.remove(from);
        self.list.insert(to, tab);
        self.active = match self.active {
            a if a == from => to,
            a if from < a && to >= a => a - 1,
            a if from > a && to <= a => a + 1,
            a => a,
        };
    }

    pub fn cycle(&mut self, forward: bool) {
        let n = self.list.len();
        self.active = if forward { (self.active + 1) % n } else { (self.active + n - 1) % n };
    }

    /// Remove a pane wherever it is. Focus moves to the previous pane of the tab;
    /// when the tab empties it closes and the tab to its left becomes active.
    pub fn remove_pane(&mut self, id: PaneId) {
        let Some(t) = self.list.iter().position(|t| t.root.contains(id)) else { return };
        let tab = &mut self.list[t];
        if tab.root == Node::Leaf(id) {
            self.list.remove(t);
            if t < self.active || (t == self.active && self.active > 0) {
                self.active -= 1;
            }
            self.active = self.active.min(self.list.len().saturating_sub(1));
            return;
        }
        let before = tab.root.leaves();
        let idx = before.iter().position(|&p| p == id).unwrap_or(0);
        tab.root.remove(id);
        if tab.focused == id {
            let after = tab.root.leaves();
            tab.focused = after[idx.saturating_sub(1).min(after.len() - 1)];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: Rect = Rect { x: 0.0, y: 0.0, w: 101.0, h: 50.0 };

    fn lay(n: &Node) -> (Vec<(PaneId, Rect)>, Vec<Rect>) {
        let (mut p, mut d) = (vec![], vec![]);
        n.layout(R, 1.0, &mut p, &mut d);
        (p, d)
    }

    #[test]
    fn same_direction_splits_are_even() {
        let mut n = Node::Leaf(1);
        n.split(1, 2, Dir::Right);
        n.split(2, 3, Dir::Right);
        assert_eq!(n.leaves(), [1, 2, 3]);
        let (p, d) = lay(&n);
        assert_eq!(d.len(), 2);
        let widths: Vec<f32> = p.iter().map(|(_, r)| r.w).collect();
        assert!(widths.iter().all(|w| (w - 33.0).abs() <= 1.0), "{widths:?}");
        // No gaps or overlaps: each pane starts right after the previous divider.
        assert_eq!(p[1].1.x, p[0].1.x + p[0].1.w + 1.0);
    }

    #[test]
    fn nested_split() {
        let mut n = Node::Leaf(1);
        n.split(1, 2, Dir::Right);
        n.split(2, 3, Dir::Down);
        let (p, _) = lay(&n);
        assert_eq!(p[0].1.h, 50.0);
        assert_eq!(p[1].1.x, p[2].1.x);
        assert!(p[2].1.y > p[1].1.y);
    }

    #[test]
    fn remove_collapses() {
        let mut n = Node::Leaf(1);
        n.split(1, 2, Dir::Right);
        n.split(2, 3, Dir::Down);
        assert!(n.remove(3));
        assert_eq!(n, Node::Split { dir: Dir::Right, children: vec![Node::Leaf(1), Node::Leaf(2)] });
        assert!(n.remove(1));
        assert_eq!(n, Node::Leaf(2));
        assert!(!n.remove(2));
    }

    #[test]
    fn neighbours() {
        let mut n = Node::Leaf(1);
        n.split(1, 2, Dir::Right);
        n.split(2, 3, Dir::Down);
        let (p, _) = lay(&n);
        assert_eq!(neighbour(&p, 2, Arrow::Left), Some(1));
        assert_eq!(neighbour(&p, 2, Arrow::Down), Some(3));
        assert_eq!(neighbour(&p, 3, Arrow::Up), Some(2));
        assert_eq!(neighbour(&p, 1, Arrow::Right).is_some(), true);
        assert_eq!(neighbour(&p, 1, Arrow::Left), None);
    }

    #[test]
    fn tabs_open_cycle_close() {
        let mut t = Tabs::default();
        t.open(1);
        t.open(2);
        t.open(3);
        assert_eq!(t.active, 2);
        t.cycle(true);
        assert_eq!(t.active, 0);
        t.cycle(false);
        assert_eq!(t.active, 2);
        t.select(1);
        t.remove_pane(2); // closing active middle tab selects the one to its left
        assert_eq!(t.list.len(), 2);
        assert_eq!(t.active, 0);
        t.remove_pane(1);
        assert_eq!(t.active, 0);
        assert_eq!(t.active().focused, 3);
        t.remove_pane(3);
        assert!(t.list.is_empty());
    }

    #[test]
    fn move_tab_keeps_active() {
        let mut t = Tabs::default();
        for id in [1, 2, 3] {
            t.open(id);
        }
        let order = |t: &Tabs| t.list.iter().map(|x| x.focused).collect::<Vec<_>>();
        t.select(0);
        t.move_tab(0, 2);
        assert_eq!(order(&t), [2, 3, 1]);
        assert_eq!(t.active, 2);
        t.select(1); // tab 3 active
        t.move_tab(2, 0);
        assert_eq!(order(&t), [1, 2, 3]);
        assert_eq!(t.active().focused, 3);
        t.move_tab(0, 5); // out of range: no-op
        assert_eq!(order(&t), [1, 2, 3]);
    }

    #[test]
    fn closing_focused_pane_moves_focus_to_previous() {
        let mut t = Tabs::default();
        t.open(1);
        t.active_mut().root.split(1, 2, Dir::Right);
        t.active_mut().focused = 2;
        t.remove_pane(2);
        assert_eq!(t.active().focused, 1);
        assert_eq!(t.active().root, Node::Leaf(1));
    }
}
