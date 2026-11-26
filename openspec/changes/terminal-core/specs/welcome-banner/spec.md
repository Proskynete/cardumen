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
The banner SHALL draw Tiburoncín from the 22×22 pixel grid (`k` outline `#1A3A57`, `b` body `#3E7CB1`, `w` foam `#EDF4F3`, `r` tongue `#F4736B`, `.` transparent) using the upper half-block `▀`, with truecolor foreground for the top pixel and background for the bottom pixel. The result SHALL occupy 22 columns and 11 rows.

#### Scenario: Pixel mapping
- **WHEN** a cell covers a `b` pixel on top and a `w` pixel below
- **THEN** it is `▀` with foreground `#3E7CB1` and background `#EDF4F3`

#### Scenario: Transparent pixel
- **WHEN** both pixels of a cell are `.`
- **THEN** the cell is a space with the default background

### Requirement: Hardware info block
To the right of the drawing, the banner SHALL show the user's name and these fields, each as a muted label with a primary-colour value: `chip`, `ram`, `macos`, `uptime`, `shell`, `font`. Values SHALL be read locally (sysctl, `sw_vers`-equivalent data, `$SHELL`, config), without network access or child processes that slow startup by more than 20 ms.

#### Scenario: M1 machine
- **WHEN** the banner renders on an Apple M1 with 8 GB
- **THEN** it shows `chip  Apple M1` and `ram  8 GB`

#### Scenario: Narrow window
- **WHEN** the window has fewer than 60 columns
- **THEN** only the info block is printed, without the drawing
