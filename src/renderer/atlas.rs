//! Shelf-packed glyph atlas. When full it is cleared and refilled on demand.
use std::collections::HashMap;

pub const SIZE: u32 = 1024;

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub struct GlyphKey {
    pub c: char,
    pub bold: bool,
    pub italic: bool,
}

#[derive(Clone, Copy)]
pub struct Entry {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub left: i32,
    pub top: i32,
    pub color: bool,
}

#[derive(Default)]
pub struct Atlas {
    x: u32,
    y: u32,
    row_h: u32,
    /// `None` caches "this char has no visible glyph" (space, etc.).
    pub map: HashMap<GlyphKey, Option<Entry>>,
}

impl Atlas {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Reserve a `w × h` slot, with 1px padding. `None` when the atlas is full.
    pub fn alloc(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        if self.x + w > SIZE {
            self.x = 0;
            self.y += self.row_h + 1;
            self.row_h = 0;
        }
        if self.y + h > SIZE || w > SIZE {
            return None;
        }
        let pos = (self.x, self.y);
        self.x += w + 1;
        self.row_h = self.row_h.max(h);
        Some(pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packs_rows_and_reports_full() {
        let mut a = Atlas::default();
        assert_eq!(a.alloc(600, 10), Some((0, 0)));
        assert_eq!(a.alloc(600, 20), Some((0, 11)));
        assert_eq!(a.alloc(10, 10), Some((601, 11)));
        assert_eq!(a.alloc(10, SIZE), None);
        a.clear();
        assert_eq!(a.alloc(5, 5), Some((0, 0)));
    }
}
