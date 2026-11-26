# app-bundle Specification

## Purpose
TBD - created by archiving change app-bundle. Update Purpose after archive.
## Requirements
### Requirement: Build the app bundle
`scripts/bundle.sh` SHALL build the release binary and produce `target/Cardumen.app` with bundle id `dev.eduardoalvarez.cardumen`, the version from `Cargo.toml`, high-resolution support, and an icon. The bundle SHALL be signed ad hoc. With `--install`, it SHALL replace `/Applications/Cardumen.app`.

#### Scenario: Install
- **WHEN** the user runs `scripts/bundle.sh --install`
- **THEN** Cardumen appears in /Applications and Spotlight, and opens from the Dock with its icon

### Requirement: App icon
The icon SHALL show Tiburoncín's 22×22 pixels, scaled with nearest-neighbour, centred on a rounded `#091319` square, at every size macOS requests (16 to 1024 px).

#### Scenario: Crisp pixels
- **WHEN** the icon is shown at 512 px
- **THEN** every pixel edge is sharp (no blur)

### Requirement: Environment when launched from Finder
When `SHELL` is empty, the system SHALL use the user's login shell from the user database (`getpwuid`), then `/bin/zsh`. When none of `LANG`, `LC_ALL` or `LC_CTYPE` is set, the shell SHALL get `LANG=en_US.UTF-8`.

#### Scenario: Launched from the Dock
- **WHEN** Cardumen is opened from the Dock without `LANG` in its environment
- **THEN** `echo $LANG` in the shell prints `en_US.UTF-8` and accented characters render correctly

