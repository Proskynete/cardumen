//! "arrecife" palette, taken from eduardoalvarez.dev's design tokens.
use alacritty_terminal::term::color::Colors;
use alacritty_terminal::vte::ansi::{Color, NamedColor};

pub type Rgb = [u8; 3];

pub const BACKGROUND: Rgb = [0x09, 0x13, 0x19];
pub const FOREGROUND: Rgb = [0xED, 0xF4, 0xF3];
pub const ACCENT: Rgb = [0x35, 0xD6, 0xC0];
pub const SELECTION: Rgb = [0x1A, 0x4A, 0x4C];

pub const ANSI: [Rgb; 16] = [
    [0x0F, 0x1F, 0x27], // black
    [0xE1, 0x57, 0x57], // red
    [0x4F, 0xB4, 0x77], // green
    [0xF2, 0xA6, 0x5A], // yellow
    [0x3E, 0x7C, 0xB1], // blue
    [0xC3, 0x8F, 0xD9], // magenta
    [0x35, 0xD6, 0xC0], // cyan
    [0xC9, 0xD6, 0xD8], // white
    [0x71, 0x91, 0x9C], // bright black
    [0xEF, 0x7B, 0x7B],
    [0x6F, 0xD3, 0x94],
    [0xF7, 0xBB, 0x7D],
    [0x6A, 0xA3, 0xD6],
    [0xD8, 0xAE, 0xEA],
    [0x5F, 0xE3, 0xD1],
    [0xED, 0xF4, 0xF3],
];

pub fn dim(c: Rgb) -> Rgb {
    c.map(|v| (v as f32 * 0.66) as u8)
}

/// xterm 256-colour palette.
pub fn indexed(i: u8) -> Rgb {
    match i {
        0..=15 => ANSI[i as usize],
        16..=231 => {
            let i = i - 16;
            let level = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            [level(i / 36), level((i / 6) % 6), level(i % 6)]
        }
        _ => {
            let v = 8 + (i - 232) * 10;
            [v, v, v]
        }
    }
}

fn named(n: NamedColor) -> Rgb {
    use NamedColor::*;
    match n {
        Foreground | BrightForeground => FOREGROUND,
        Background => BACKGROUND,
        Cursor => ACCENT,
        DimForeground => dim(FOREGROUND),
        DimBlack => dim(ANSI[0]),
        DimRed => dim(ANSI[1]),
        DimGreen => dim(ANSI[2]),
        DimYellow => dim(ANSI[3]),
        DimBlue => dim(ANSI[4]),
        DimMagenta => dim(ANSI[5]),
        DimCyan => dim(ANSI[6]),
        DimWhite => dim(ANSI[7]),
        other => ANSI[(other as usize).min(15)],
    }
}

/// Resolve a cell colour, honouring OSC 4/10/11 overrides set by the app.
pub fn resolve(c: Color, overrides: &Colors) -> Rgb {
    let o = |i: usize| overrides[i].map(|r| [r.r, r.g, r.b]);
    match c {
        Color::Spec(r) => [r.r, r.g, r.b],
        Color::Named(n) => o(n as usize).unwrap_or_else(|| named(n)),
        Color::Indexed(i) => o(i as usize).unwrap_or_else(|| indexed(i)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_cube_and_greys() {
        assert_eq!(indexed(16), [0, 0, 0]);
        assert_eq!(indexed(21), [0, 0, 255]);
        assert_eq!(indexed(196), [255, 0, 0]);
        assert_eq!(indexed(231), [255, 255, 255]);
        assert_eq!(indexed(232), [8, 8, 8]);
        assert_eq!(indexed(255), [238, 238, 238]);
    }

    #[test]
    fn named_defaults() {
        let colors = Colors::default();
        assert_eq!(resolve(Color::Named(NamedColor::Background), &colors), BACKGROUND);
        assert_eq!(resolve(Color::Named(NamedColor::Red), &colors), ANSI[1]);
        assert_eq!(resolve(Color::Named(NamedColor::BrightWhite), &colors), ANSI[15]);
    }
}
