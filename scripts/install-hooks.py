#!/usr/bin/env python3
"""Add Cardumen's Claude Code hooks to ~/.claude/settings.json (idempotent, keeps a backup).

Each hook reports `<pane> <event>` to the Cardumen instance the session runs in,
and does nothing outside Cardumen.
"""
import json
import shutil
import time
from pathlib import Path

MARK = "CARDUMEN_SOCKET"
EVENTS = {
    "UserPromptSubmit": "prompt",
    "PostToolUse": "tool",
    "PermissionRequest": "waiting",
    "Stop": "stop",
    "StopFailure": "stop",
    "SessionEnd": "end",
}


def command(event: str) -> str:
    # The hook's JSON payload (stdin) follows the event, with newlines removed.
    return (
        f'[ -n "${MARK}" ] && {{ printf \'%s {event} \' "$CARDUMEN_PANE"; tr -d \'\\n\'; echo; }} '
        f'| nc -U -w 1 "${MARK}" >/dev/null 2>&1; exit 0'
    )


def main() -> None:
    path = Path.home() / ".claude/settings.json"
    settings = json.loads(path.read_text()) if path.exists() else {}
    hooks = settings.setdefault("hooks", {})
    changed = False
    for name, event in EVENTS.items():
        groups = hooks.setdefault(name, [])
        wanted = command(event)
        ours = [g for g in groups if any(MARK in h.get("command", "") for h in g.get("hooks", []))]
        if len(ours) == 1 and ours[0]["hooks"][0].get("command") == wanted:
            continue
        # Replace older Cardumen entries; other tools' hooks stay untouched.
        groups[:] = [g for g in groups if g not in ours]
        groups.append({"hooks": [{"type": "command", "command": wanted}]})
        changed = True
    if not changed:
        print("hooks already installed")
        return
    if path.exists():
        backup = path.with_name(f"settings.json.bak-{time.strftime('%Y%m%d-%H%M%S')}")
        shutil.copy2(path, backup)
        print(f"backup: {backup}")
    path.write_text(json.dumps(settings, indent=2) + "\n")
    print(f"hooks installed in {path}")


if __name__ == "__main__":
    main()
