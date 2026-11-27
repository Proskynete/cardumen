//! Font lookup, cell metrics and glyph rasterisation (cosmic-text + swash).
use std::collections::HashMap;

use cosmic_text::{Attrs, Buffer, CacheKey, Family, FontSystem, Metrics, Shaping, Style, SwashCache, SwashContent, Weight};

pub const FALLBACK_FAMILY: &str = "Menlo";

pub struct Glyph {
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
    pub color: bool,
    /// RGBA8; masks store coverage in every channel.
    pub rgba: Vec<u8>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Face {
    /// The system UI font (SF Pro).
    Sans,
    /// The configured terminal font.
    Mono,
}

/// Longest interface text shaped: wider than any window at any font size.
/// Bounds the run cache (4096 entries) by size too: unclipped, one minified
/// diff line or a huge title made multi-MB entries and grew it to 20 GB.
pub const MAX_RUN_CHARS: usize = 512;

/// `text` cut to its first `MAX_RUN_CHARS` chars.
pub fn clip(text: &str) -> &str {
    text.char_indices().nth(MAX_RUN_CHARS).map_or(text, |(i, _)| &text[..i])
}

/// A shaped single-line text run: glyph cache keys with positions relative to the
/// run's top-left (y already includes the baseline), and its width in pixels.
#[derive(Clone)]
pub struct Run {
    pub glyphs: Vec<(CacheKey, i32, i32)>,
    pub width: f32,
}

pub struct Fonts {
    system: FontSystem,
    cache: SwashCache,
    buffer: Buffer,
    ui_buffer: Buffer,
    runs: HashMap<(String, Face, u32, bool), Run>,
    sans: String,
    pub family: String,
    /// Font size in physical pixels.
    pub px: f32,
    pub cell_w: f32,
    pub cell_h: f32,
    /// Distance from the cell top to the baseline, in physical pixels.
    pub baseline: f32,
}

impl Fonts {
    pub fn new(family: &str, points: f32, scale: f32) -> Self {
        let mut system = FontSystem::new();
        let installed = system.db().faces().any(|f| f.families.iter().any(|(name, _)| name == family));
        let family = if installed {
            family.to_string()
        } else {
            eprintln!("cardumen: font \"{family}\" not installed, using {FALLBACK_FAMILY}");
            FALLBACK_FAMILY.to_string()
        };
        let buffer = Buffer::new(&mut system, Metrics::new(14.0, 18.0));
        let ui_buffer = Buffer::new(&mut system, Metrics::new(13.0, 17.0));
        let sans = ["System Font", ".SF NS", "Helvetica Neue"]
            .iter()
            .find(|name| system.db().faces().any(|f| f.families.iter().any(|(n, _)| n == *name)))
            .unwrap_or(&"Helvetica")
            .to_string();
        let mut fonts = Self {
            system,
            cache: SwashCache::new(),
            buffer,
            ui_buffer,
            runs: HashMap::new(),
            sans,
            family,
            px: 0.0,
            cell_w: 0.0,
            cell_h: 0.0,
            baseline: 0.0,
        };
        fonts.set_size(points, scale);
        fonts
    }

    /// macOS points are logical pixels, so physical px = points × scale.
    pub fn set_size(&mut self, points: f32, scale: f32) {
        self.px = (points * scale).round();
        let query = cosmic_text::fontdb::Query {
            families: &[cosmic_text::fontdb::Family::Name(&self.family)],
            ..Default::default()
        };
        let id = self.system.db().query(&query).expect("font family resolved in new()");
        let font = self.system.get_font(id, Weight::NORMAL).expect("font loads");
        let swash = font.as_swash();
        let m = swash.metrics(&[]);
        let k = self.px / m.units_per_em as f32;
        let (ascent, descent, leading) = (m.ascent * k, m.descent * k, m.leading * k);
        let advance = swash.glyph_metrics(&[]).advance_width(swash.charmap().map('M')) * k;
        self.cell_w = advance.round().max(1.0);
        self.cell_h = (ascent + descent + leading).ceil().max(1.0);
        self.baseline = (leading / 2.0 + ascent).round();
        self.buffer.set_metrics(Metrics::new(self.px, self.cell_h));
    }

    pub fn rasterize(&mut self, c: char, bold: bool, italic: bool) -> Option<Glyph> {
        let attrs = Attrs::new()
            .family(Family::Name(&self.family))
            .weight(if bold { Weight::BOLD } else { Weight::NORMAL })
            .style(if italic { Style::Italic } else { Style::Normal });
        let mut text = [0u8; 4];
        self.buffer.set_text(c.encode_utf8(&mut text), &attrs, Shaping::Advanced, None);
        self.buffer.shape_until_scroll(&mut self.system, false);
        let key = {
            let run = self.buffer.layout_runs().next()?;
            run.glyphs.first()?.physical((0.0, 0.0), 1.0).cache_key
        };
        self.raster_key(key)
    }

    /// Shape one line of interface text (cached).
    pub fn run(&mut self, text: &str, face: Face, px: f32, bold: bool) -> Run {
        let text = clip(text);
        let key = (text.to_string(), face, px.to_bits(), bold);
        if let Some(run) = self.runs.get(&key) {
            return run.clone();
        }
        if self.runs.len() > 4096 {
            self.runs.clear();
        }
        let family = match face {
            Face::Sans => Family::Name(&self.sans),
            Face::Mono => Family::Name(&self.family),
        };
        let attrs = Attrs::new().family(family).weight(if bold { Weight::SEMIBOLD } else { Weight::NORMAL });
        self.ui_buffer.set_metrics(Metrics::new(px, (px * 1.3).ceil()));
        self.ui_buffer.set_text(text, &attrs, Shaping::Advanced, None);
        self.ui_buffer.shape_until_scroll(&mut self.system, false);
        let mut run = Run { glyphs: vec![], width: 0.0 };
        if let Some(line) = self.ui_buffer.layout_runs().next() {
            run.width = line.line_w;
            for g in line.glyphs {
                let p = g.physical((0.0, line.line_y), 1.0);
                run.glyphs.push((p.cache_key, p.x, p.y));
            }
        }
        self.runs.insert(key, run.clone());
        run
    }

    pub fn raster_key(&mut self, key: CacheKey) -> Option<Glyph> {
        let image = self.cache.get_image_uncached(&mut self.system, key)?;
        if image.placement.width == 0 || image.placement.height == 0 {
            return None;
        }
        let (color, rgba) = match image.content {
            SwashContent::Mask => (false, image.data.iter().flat_map(|&a| [a, a, a, a]).collect()),
            SwashContent::SubpixelMask => (false, image.data.chunks(4).flat_map(|p| [p[1]; 4]).collect()),
            SwashContent::Color => (true, image.data),
        };
        Some(Glyph {
            left: image.placement.left,
            top: image.placement.top,
            width: image.placement.width,
            height: image.placement.height,
            color,
            rgba,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_counts_chars() {
        assert_eq!(clip("añadí"), "añadí");
        let long = "é".repeat(MAX_RUN_CHARS + 10);
        assert_eq!(clip(&long).chars().count(), MAX_RUN_CHARS);
    }

    #[test]
    fn huge_text_stays_small_in_cache() {
        // Regression: a 200k-char label cached a multi-MB run per probe.
        let mut f = Fonts::new("Menlo", 13.0, 2.0);
        let huge = "x".repeat(200_000);
        let started = std::time::Instant::now();
        let run = f.run(&huge, Face::Mono, 13.0, false);
        assert!(run.glyphs.len() <= MAX_RUN_CHARS);
        assert!(f.runs.keys().all(|(t, ..)| t.chars().count() <= MAX_RUN_CHARS));
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
    }
}
