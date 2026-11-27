//! Find in a pane's scrollback (Cmd+F). The match is shown as the selection.
use alacritty_terminal::event::EventListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Boundary, Column, Direction, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::Term;
use alacritty_terminal::term::search::{Match, RegexSearch};

use crate::layout::PaneId;

pub struct Search {
    pub pane: PaneId,
    pub query: String,
    pub current: Option<Match>,
    /// The query has no match anywhere in the scrollback.
    pub missing: bool,
}

impl Search {
    pub fn new(pane: PaneId) -> Self {
        Self { pane, query: String::new(), current: None, missing: false }
    }
}

/// The query as a literal pattern (alacritty searches with regexes).
fn escape(query: &str) -> String {
    let mut out = String::with_capacity(query.len());
    for c in query.chars() {
        if "\\.+*?()|[]{}^$#&-~".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Find the next match. `again` steps from the current match (Enter: older,
/// Shift+Enter: newer); otherwise the query changed and the search restarts
/// from the current match, or from the bottom going up.
pub fn step<L: EventListener>(term: &mut Term<L>, s: &mut Search, dir: Direction, again: bool) {
    term.selection = None;
    s.missing = false;
    if s.query.is_empty() {
        s.current = None;
        return;
    }
    let Ok(mut regex) = RegexSearch::new(&escape(&s.query)) else {
        s.missing = true;
        return;
    };
    let bottom = Point::new(Line(term.screen_lines() as i32 - 1), Column(term.columns() - 1));
    let origin = match (&s.current, again) {
        (Some(m), true) if dir == Direction::Left => m.start().sub(&*term, Boundary::None, 1),
        (Some(m), true) => m.end().add(&*term, Boundary::None, 1),
        (Some(m), false) => *m.end(),
        (None, _) => bottom,
    };
    let side = if dir == Direction::Left { Side::Right } else { Side::Left };
    s.current = term.search_next(&mut regex, origin, dir, side, None);
    match &s.current {
        Some(m) => {
            let mut sel = Selection::new(SelectionType::Simple, *m.start(), Side::Left);
            sel.update(*m.end(), Side::Right);
            term.selection = Some(sel);
            term.scroll_to_point(*m.start());
        }
        None => s.missing = true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alacritty_terminal::event::VoidListener;
    use alacritty_terminal::term::{Config, test::TermSize};
    use alacritty_terminal::vte::ansi::{Processor, StdSyncHandler};

    fn term(text: &str) -> Term<VoidListener> {
        let mut t = Term::new(Config::default(), &TermSize::new(20, 4), VoidListener);
        let mut p: Processor<StdSyncHandler> = Processor::new();
        p.advance(&mut t, text.as_bytes());
        t
    }

    #[test]
    fn escapes_regex_chars() {
        assert_eq!(escape("a.b(c)"), r"a\.b\(c\)");
    }

    #[test]
    fn finds_newest_then_older() {
        let mut t = term("foo 1\r\nbar\r\nfoo 2\r\n");
        let mut s = Search::new(1);
        s.query = "foo".into();
        step(&mut t, &mut s, Direction::Left, false);
        assert_eq!(s.current.as_ref().map(|m| m.start().line), Some(Line(2)));
        step(&mut t, &mut s, Direction::Left, true);
        assert_eq!(s.current.as_ref().map(|m| m.start().line), Some(Line(0)));
        assert!(t.selection.is_some());
        s.query = "zzz".into();
        step(&mut t, &mut s, Direction::Left, false);
        assert!(s.missing && t.selection.is_none());
    }
}
