## ADDED Requirements

### Requirement: Session file override
When `CARDUMEN_SESSION` is set, the system SHALL read and save the session at that path instead of `~/Library/Application Support/cardumen/session.json`, with the same format and atomic write.

#### Scenario: Throwaway session
- **WHEN** Cardumen starts with `CARDUMEN_SESSION=/tmp/x/session.json` holding 5 tabs
- **THEN** it opens those 5 tabs, saves changes to `/tmp/x/session.json`, and the default session file is neither read nor written
