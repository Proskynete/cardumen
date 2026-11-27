## Context

- `Event::ClipboardLoad` (`src/main.rs`) answers every OSC 52 read with the clipboard text. alacritty, iTerm2, Ghostty and kitty block reads by default.
- `paste_text` sends text with `\n` turned into `\r` when bracketed paste is off, so each line runs as a command. zsh enables bracketed paste at its prompt, but `ssh`, `sh`, `python` and some REPLs do not.
- The socket lives in `$TMPDIR`, which macOS makes per-user `0700`; the socket itself gets the default umask mode.

## Goals / Non-Goals

**Goals:** close the clipboard read and the blind multi-line paste with the least code; keep normal paste instant.

**Non-Goals:** a setting to allow OSC 52 reads, per-app allow lists, scanning pasted text for "dangerous" commands, limits on OSC 52 writes.

## Decisions

- **Ignore reads, no reply.** Sending an empty reply still tells the program it asked; ignoring matches alacritty's `OnlyCopy` default. No config option: nobody asked for one, and it can be added if a real tool needs it.
- **Confirm only when it matters.** Ask only when the text contains `\n` or `\r` *and* bracketed paste is off. Single-line pastes and pastes into Claude Code (bracketed) stay instant. A trailing newline counts, since it also runs the line.
- **Reuse the find bar's shape.** Same position, size and key handling (Enter / Esc) as Cmd+F, holding the pending text; while it is open, keys go to it, not the pane. No modal window.
- **`chmod 600` right after `bind`.** The window between `bind` and `chmod` is inside a `0700` directory, so no other user can reach it.

## Risks / Trade-offs

- [A tool needs OSC 52 reads (rare, e.g. some remote editors)] → it gets no answer and falls back to its own paste; add an opt-in setting only if that comes up.
- [The bar slows down a deliberate multi-line paste into `ssh`] → one Enter; it shows the line count so it is quick to judge.
