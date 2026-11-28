## 1. Clipboard

- [x] 1.1 `Event::ClipboardLoad`: ignore the request (no clipboard read, no reply)
- [ ] 1.2 Check by hand: `printf '\e]52;c;?\a'` gets no reply; `printf '\e]52;c;aG9sYQ==\a'` still copies `hola`

## 2. Multi-line paste

- [x] 2.1 Pure helper `needs_confirm(text, bracketed) -> Option<usize>` (line count) with a test: bracketed, single line, trailing newline, `\r\n`
- [x] 2.2 `paste_text`: when it returns `Some(n)`, keep the text pending and open the bar instead of writing
- [x] 2.3 Bar drawn like the find bar ("Pegar N líneas sin protección · Enter pega · Esc cancela"); Enter writes the text, Esc drops it; keys don't reach the pane while it is open
- [ ] 2.4 Check by hand: paste 3 lines into `sh` (bar) and into Claude Code (no bar)

## 3. Socket

- [x] 3.1 `serve`: `set_permissions(0o600)` right after `bind`; extend the socket test to assert the mode

## 4. Verify

- [x] 4.1 `cargo test`, `cargo clippy` with no new warnings
