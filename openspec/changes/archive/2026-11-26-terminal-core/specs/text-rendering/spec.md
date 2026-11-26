## ADDED Requirements

### Requirement: GPU grid rendering
The system SHALL render the terminal grid on the GPU (wgpu, Metal backend), redrawing only when the terminal content, cursor blink, selection or window changes, and SHALL NOT redraw continuously while idle.

#### Scenario: Idle CPU
- **WHEN** the terminal is idle with no output
- **THEN** CPU usage stays near 0%, apart from cursor blink frames

### Requirement: Nerd Font support
The system SHALL render text with the configured monospace font (default `FiraCode Nerd Font Mono`). When a glyph is missing, it SHALL fall back to installed system fonts. Nerd Font icon glyphs SHALL render, and double-width characters (CJK, emoji) SHALL occupy two cells.

#### Scenario: Prompt icons
- **WHEN** the prompt contains Nerd Font icons (for example U+E0B0 or U+F121)
- **THEN** the icons render as glyphs, not as replacement boxes

#### Scenario: Font not installed
- **WHEN** the configured font is not installed
- **THEN** the system falls back to `Menlo` and logs a warning to stderr

### Requirement: Colours and attributes
The system SHALL render the 16 ANSI colours from the theme, the 256-colour palette, truecolor, bold, italic, underline, strikethrough, inverse and dim.

#### Scenario: Truecolor
- **WHEN** the program prints `ESC[38;2;53;214;192m`
- **THEN** the text renders in `#35D6C0`

### Requirement: Arrecife theme
The system SHALL use the "arrecife" theme by default: background `#091319`, foreground `#EDF4F3`, cursor and accent `#35D6C0`.

#### Scenario: Default look
- **WHEN** no config file exists
- **THEN** the window background is `#091319`

### Requirement: Cursor
The default cursor SHALL be a blinking beam in the accent colour (`#35D6C0`), blinking every 530 ms and staying visible while the user types. The system SHALL draw the shape requested by the application (block, beam, underline), blink it when requested, and draw it hollow when the window is unfocused.

#### Scenario: Default cursor
- **WHEN** the shell has not requested a cursor style
- **THEN** the cursor is a blinking accent-coloured beam

#### Scenario: Unfocused
- **WHEN** the window loses focus
- **THEN** the block cursor is drawn as an outline

### Requirement: HiDPI
The system SHALL rasterise glyphs at the display scale factor, so text is crisp on Retina screens.

#### Scenario: Move between displays
- **WHEN** the window moves from a 2x display to a 1x display
- **THEN** glyphs are re-rasterised and the grid is recomputed
