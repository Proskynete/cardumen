## Context

3.6 GB of transcripts in 971 files (~53k assistant messages) on the owner's Mac. A cold full scan takes about 4.5 s, mostly I/O.

## Decisions

- **Fast line filter**: only lines containing `"type":"assistant"` together with `"usage"`, or `"type":"user"`, are parsed with serde_json. Everything else is skipped by a substring check.
- **Compact records** (~32 bytes): unix seconds, session index, project index, model index, four token counts. User turns are records too, with zero tokens and a flag. Strings (session ids, projects, models) are interned per cache.
- **Cache file**: a small hand-written little-endian format (magic + version, string tables, per-file `{path, size, offset}`, records), with no serde dependency. Any read error means rebuilding from scratch.
- **Short-lived worker**: opening the view or changing the range spawns a thread that loads the cache, reads new bytes, writes the cache back, computes a `Summary` for the range and the previous period, sends it, and exits. The records never live in the UI process between uses, so the resident memory stays at the terminal's baseline.
- **Aggregation is pure** (`summarize(records, range, now, tz offset)`) and unit-tested. Agent time and concurrency come from per-session sorted timestamps.
- **Drawing** reuses the chrome primitives: cards and pills are rounded rects, bars and heatmap cells are rects, all text is SF Pro.

## Risks / Trade-offs

- [First open takes a few seconds] → Show progress text; later opens read only appended bytes.
- [Transcript format changes] → Unknown lines are ignored; nothing crashes; the numbers might undercount.
- [Time zones] → Days and hours use the local offset at summary time.
