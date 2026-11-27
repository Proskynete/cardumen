//! Claude Code session state, reported by hooks over a Unix socket.
use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::time::Instant;

use winit::event_loop::EventLoopProxy;

use crate::layout::PaneId;
use crate::session::UserEvent;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Claude {
    Working,
    Waiting,
    Done,
}

/// `Some(None)` ends the session; `None` means an unknown event (ignored).
pub fn transition(event: &str) -> Option<Option<Claude>> {
    Some(match event {
        "prompt" | "tool" => Some(Claude::Working),
        "waiting" => Some(Claude::Waiting),
        "stop" => Some(Claude::Done),
        "end" => None,
        _ => return None,
    })
}

/// What a hook's JSON payload tells us beyond the state.
#[derive(Clone, Default, PartialEq, Debug)]
pub struct Hook {
    /// Permission request summary, e.g. "Bash · git push".
    pub ask: Option<String>,
    pub transcript: Option<PathBuf>,
}

/// Parse `<pane id> <event> [<hook json>]`.
pub fn parse(line: &str) -> Option<(PaneId, Option<Claude>, Hook)> {
    let mut parts = line.trim().splitn(3, ' ');
    let pane = parts.next()?.parse().ok()?;
    let state = transition(parts.next()?.trim())?;
    let json: Option<serde_json::Value> = parts.next().and_then(|j| serde_json::from_str(j).ok());
    let hook = json.map_or_else(Hook::default, |v| Hook {
        ask: (state == Some(Claude::Waiting)).then(|| crate::context::permission_summary(&v)).flatten(),
        transcript: v["transcript_path"].as_str().map(PathBuf::from),
    });
    Some((pane, state, hook))
}

#[derive(Clone, Copy)]
pub struct State {
    pub claude: Claude,
    pub since: Instant,
}

pub fn elapsed(since: Instant) -> String {
    let s = since.elapsed().as_secs();
    match s {
        0..60 => "ahora".into(),
        60..3600 => format!("{}m", s / 60),
        _ => format!("{}h {}m", s / 3600, s / 60 % 60),
    }
}

pub fn socket_path() -> PathBuf {
    std::env::temp_dir().join(format!("cardumen-{}.sock", std::process::id()))
}

/// Listen for hook lines and forward them to the event loop.
pub fn listen(path: PathBuf, proxy: EventLoopProxy<UserEvent>) -> std::io::Result<()> {
    serve(path, move |pane, state, hook| proxy.send_event(UserEvent::Status(pane, state, hook)).is_ok())
}

/// Accept connections and call `on_event` per valid line; stops when it returns false.
fn serve(path: PathBuf, on_event: impl Fn(PaneId, Option<Claude>, Hook) -> bool + Send + 'static) -> std::io::Result<()> {
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path)?;
    // Owner only; $TMPDIR is already per-user, this is defence in depth.
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    std::thread::Builder::new().name("status socket".into()).spawn(move || {
        for stream in listener.incoming().flatten() {
            for line in BufReader::new(stream).lines().map_while(Result::ok) {
                if let Some((pane, state, hook)) = parse(&line) {
                    if !on_event(pane, state, hook) {
                        return;
                    }
                }
            }
        }
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lines() {
        let none = Hook::default();
        assert_eq!(parse("3 waiting\n"), Some((3, Some(Claude::Waiting), none.clone())));
        assert_eq!(parse("12 prompt"), Some((12, Some(Claude::Working), none.clone())));
        assert_eq!(parse("12 tool"), Some((12, Some(Claude::Working), none.clone())));
        assert_eq!(parse("1 stop"), Some((1, Some(Claude::Done), none.clone())));
        assert_eq!(parse("1 end"), Some((1, None, none)));
        let (_, _, hook) = parse(r#"4 waiting {"transcript_path":"/t.jsonl","tool_name":"Bash","tool_input":{"command":"ls"}}"#).unwrap();
        assert_eq!(hook, Hook { ask: Some("Bash · ls".into()), transcript: Some("/t.jsonl".into()) });
        let (_, _, hook) = parse(r#"4 stop {"transcript_path":"/t.jsonl"}"#).unwrap();
        assert_eq!(hook.ask, None);
        assert_eq!(parse("1 dance"), None);
        assert_eq!(parse("x waiting"), None);
        assert_eq!(parse(""), None);
    }

    /// The exact command `scripts/install-hooks.py` installs, sent through a real socket.
    #[test]
    fn hook_command_reaches_socket() {
        let path = std::env::temp_dir().join(format!("cardumen-test-{}.sock", std::process::id()));
        let (tx, rx) = std::sync::mpsc::channel();
        serve(path.clone(), move |pane, state, hook| tx.send((pane, state, hook)).is_ok()).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        let hook = r#"[ -n "$CARDUMEN_SOCKET" ] && { printf '%s waiting ' "$CARDUMEN_PANE"; tr -d '\n'; echo; } | nc -U -w 1 "$CARDUMEN_SOCKET" >/dev/null 2>&1; exit 0"#;
        let mut child = std::process::Command::new("sh")
            .args(["-c", hook])
            .env("CARDUMEN_SOCKET", &path)
            .env("CARDUMEN_PANE", "7")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        use std::io::Write;
        // Claude Code sends pretty or compact JSON on stdin; newlines must not split the line.
        child.stdin.take().unwrap().write_all(b"{\"tool_name\": \"Bash\",\n \"tool_input\": {\"command\": \"git push\"}}\n").unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success() && out.stdout.is_empty());
        let got = rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        assert_eq!(got.0, 7);
        assert_eq!(got.1, Some(Claude::Waiting));
        assert_eq!(got.2.ask.as_deref(), Some("Bash · git push"));
        let _ = std::fs::remove_file(path);
    }
}
