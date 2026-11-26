## ADDED Requirements

### Requirement: Banner on session start
When a session starts, the system SHALL write a welcome banner into the terminal before any shell output. The banner SHALL be part of the scrollback like normal output.

#### Scenario: New window
- **WHEN** the user opens Cardumen
- **THEN** the banner appears first, followed by the shell prompt

#### Scenario: Disabled
- **WHEN** `welcome = false` is set in the config
- **THEN** no banner is printed

### Requirement: Tiburoncín drawing
The banner SHALL draw Tiburoncín from the 22×22 pixel grid (`k` outline `#1A3A57`, `b` body `#3E7CB1`, `w` foam `#EDF4F3`, `r` tongue `#F4736B`, `.` transparent). The drawing SHALL use upper half-blocks `▀` (22 columns × 11 rows), the same size as on eduardoalvarez.dev, vertically centred next to the info boxes. Below 74 columns, no drawing is shown. Block elements `▀ ▄ █` SHALL be rendered as exact rectangles, without seams between rows.

#### Scenario: Pixel mapping in half-blocks
- **WHEN** the window has 80 columns and a cell covers a `b` pixel on top and a `k` pixel below
- **THEN** it is `▀` with foreground `#3E7CB1` and background `#1A3A57`

#### Scenario: Narrow window
- **WHEN** the window has fewer than 74 columns
- **THEN** only the info boxes are printed, without the drawing

### Requirement: Info boxes
To the right of the drawing, the banner SHALL show the user's name followed by three 48-column boxes with a centred title, each item prefixed by a Nerd Font icon:
- **Hardware**: CPU with core counts (P + E), display (name, logical resolution, scale, refresh rate), disk, memory and swap as `used / total (pct%)`.
- **Software**: macOS name and version, Darwin kernel, shell, `cardumen <version>`, Homebrew package count, theme name with colour dots, font with size.
- **Tiempo**: time since macOS was installed, uptime, and the date of the last system update, in Spanish.

Percentages SHALL be coloured by fullness: accent below 70%, warm from 70% to 89%, error from 90%. All values SHALL be read locally (sysctl, statfs, file metadata, the window's monitor) without network access or child processes.

#### Scenario: M1 machine
- **WHEN** the banner renders on an Apple M1 with 8 GB
- **THEN** the Hardware box shows `Apple M1 · 8 núcleos (4P + 4E)` and memory as `x / 8.0 GiB (n%)`

#### Scenario: Nearly full disk
- **WHEN** the data volume is 94% used
- **THEN** `94%` is shown in the error colour
