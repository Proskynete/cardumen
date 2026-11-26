//! Font lookup, cell metrics and glyph rasterisation (cosmic-text + swash).
use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Style, SwashCache, SwashContent, Weight};

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

pub struct Fonts {
    system: FontSystem,
    cache: SwashCache,
    buffer: Buffer,
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
        let mut fonts = Self {
            system,
            cache: SwashCache::new(),
            buffer,
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
