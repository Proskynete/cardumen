//! Claude Code session state, reported by hooks over a Unix socket.
use std::io::{BufRead, BufReader};
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

/// Parse `<pane id> <event>`.
pub fn parse(line: &str) -> Option<(PaneId, Option<Claude>)> {
    let (pane, event) = line.trim().split_once(' ')?;
    Some((pane.parse().ok()?, transition(event.trim())?))
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
    serve(path, move |pane, state| proxy.send_event(UserEvent::Status(pane, state)).is_ok())
}

/// Accept connections and call `on_event` per valid line; stops when it returns false.
fn serve(path: PathBuf, on_event: impl Fn(PaneId, Option<Claude>) -> bool + Send + 'static) -> std::io::Result<()> {
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path)?;
    std::thread::Builder::new().name("status socket".into()).spawn(move || {
        for stream in listener.incoming().flatten() {
            for line in BufReader::new(stream).lines().map_while(Result::ok) {
                if let Some((pane, state)) = parse(&line) {
                    if !on_event(pane, state) {
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
        assert_eq!(parse("3 waiting\n"), Some((3, Some(Claude::Waiting))));
        assert_eq!(parse("12 prompt"), Some((12, Some(Claude::Working))));
        assert_eq!(parse("12 tool"), Some((12, Some(Claude::Working))));
        assert_eq!(parse("1 stop"), Some((1, Some(Claude::Done))));
        assert_eq!(parse("1 end"), Some((1, None)));
        assert_eq!(parse("1 dance"), None);
        assert_eq!(parse("x waiting"), None);
        assert_eq!(parse(""), None);
    }

    /// The exact command `scripts/install-hooks.py` installs, sent through a real socket.
    #[test]
    fn hook_command_reaches_socket() {
        let path = std::env::temp_dir().join(format!("cardumen-test-{}.sock", std::process::id()));
        let (tx, rx) = std::sync::mpsc::channel();
        serve(path.clone(), move |pane, state| tx.send((pane, state)).is_ok()).unwrap();
        let hook = r#"[ -n "$CARDUMEN_SOCKET" ] && printf '%s waiting\n' "$CARDUMEN_PANE" | nc -U -w 1 "$CARDUMEN_SOCKET" >/dev/null 2>&1; exit 0"#;
        let out = std::process::Command::new("sh")
            .args(["-c", hook])
            .env("CARDUMEN_SOCKET", &path)
            .env("CARDUMEN_PANE", "7")
            .output()
            .unwrap();
        assert!(out.status.success() && out.stdout.is_empty());
        let got = rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        assert_eq!(got, (7, Some(Claude::Waiting)));
        let _ = std::fs::remove_file(path);
    }
}
