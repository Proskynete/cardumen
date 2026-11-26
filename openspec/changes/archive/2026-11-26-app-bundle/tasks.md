## 1. Environment

- [x] 1.1 `shell_program()`: SHELL → getpwuid pw_shell → /bin/zsh; set `LANG=en_US.UTF-8` when no locale variable is inherited (unit test for the choice)

## 2. Bundle

- [x] 2.1 `scripts/icon.py`: iconset PNGs from the Tiburoncín grid in `src/welcome.rs`
- [x] 2.2 `scripts/bundle.sh`: release build, Info.plist, icns, ad-hoc signature, `--install`
- [x] 2.3 README: how to build and install the app

## 3. Verify

- [x] 3.1 Install, open from /Applications, check icon, `echo $SHELL $LANG`, and the shell prompt
