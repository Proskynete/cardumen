## 1. App hooks

- [x] 1.1 `session_file::path()`: use `CARDUMEN_SESSION` when set; test that it wins over `HOME`
- [x] 1.2 `CARDUMEN_STARTUP_LOG`: print `first frame: <ms>` once after the first presented frame

## 2. Script

- [x] 2.1 Fixture session: 5 tabs (one split) in temp directories outside git, written by the script
- [x] 2.2 `scripts/budget.sh`: build release, launch with both env vars, read the first-frame line, wait 10 s, sample `ps -o rss=,%cpu=` 5×1 s, SIGTERM, `trap` cleanup
- [x] 2.3 Print value vs limit per metric; exit non-zero when any is over (limits as variables at the top)

## 3. Verify

- [x] 3.1 Run it on the M1: record the baseline in the PR; confirm the real `session.json` never gets the fixture tabs (a checksum is useless while the real app is running and saving)
- [x] 3.2 Force a failure (lower the RSS limit) and check the exit code
- [x] 3.3 README "Liviana y privada": mention `scripts/budget.sh`
