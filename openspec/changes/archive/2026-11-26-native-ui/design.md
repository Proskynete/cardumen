## Context

The choice was made visually from three mockups in the browser (`.superpowers/brainstorm/`): option B, native macOS.

## Decisions

- **Rounded shapes in the shader**: a new quad kind evaluates a rounded-box SDF with 1-px anti-aliasing. Circles are rounded boxes with radius = half the size. Tinted fills are precomputed colours mixed over the surface they sit on, so blending stays opaque and simple.
- **Text runs**: `Fonts::run(text, face, px, bold)` shapes with cosmic-text (family `System Font` for SF Pro, the configured mono family for paths). It returns glyph cache keys with positions and the run width, cached by `(text, face, px, bold)`. The atlas is keyed by `Cell(char…)` or `Run(CacheKey)`, so terminal glyphs and UI glyphs share one texture.
- **Layout by measurement**: badges, pills and wrapped text are sized with run widths, so nothing depends on fixed character counts.
- **Gutter**: the window clears to a darker colour, and panes draw their own `#091319` card. Terminal cells with the default background stay transparent, so the card shows through.

## Risks / Trade-offs

- [SF Pro is a variable font; weight selection depends on cosmic-text's variation support] → If bold doesn't render bold, fall back to the default weight. The size hierarchy still reads.
- [More quads per frame] → Still a few hundred, one draw call.
