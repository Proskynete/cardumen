## ADDED Requirements

### Requirement: Optional TOML config
The system SHALL read `~/.config/cardumen/config.toml` at startup when it exists. Every key SHALL be optional, with these defaults: `font.family = "FiraCode Nerd Font Mono"`, `font.size = 14.0`, `scrollback = 10000`, `welcome = true`, `user_name = <macOS full name>`.

#### Scenario: No config file
- **WHEN** the file does not exist
- **THEN** the defaults are used and no file is created

#### Scenario: Custom font size
- **WHEN** the file contains `[font]` with `size = 16`
- **THEN** text renders at 16 pt

### Requirement: Invalid config
When the file cannot be parsed, the system SHALL start with the defaults and print the parse error, with line and column, to stderr. It SHALL NOT crash.

#### Scenario: Typo in config
- **WHEN** the file contains `size = "big"`
- **THEN** Cardumen starts with the defaults and reports the error
