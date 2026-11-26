//! One shell: PTY + VT state + the I/O thread. Independent of any window,
//! so tabs and splits can hold several later.
use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::event_loop::{EventLoop, EventLoopSender, Msg};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::{self, Term};
use alacritty_terminal::tty;
use alacritty_terminal::vte::ansi;
use winit::event_loop::EventLoopProxy;

use crate::layout::PaneId;

pub enum UserEvent {
    Term(PaneId, Event),
    /// Claude Code state reported by a hook; `None` = session ended.
    Status(PaneId, Option<crate::status::Claude>, crate::status::Hook),
    Context(crate::context::Update),
    Usage(crate::usage::Range, Box<crate::usage::Summary>),
}

#[derive(Clone)]
pub struct Listener(pub EventLoopProxy<UserEvent>, pub PaneId);

impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        let _ = self.0.send_event(UserEvent::Term(self.1, event));
    }
}

#[derive(Clone, Copy)]
pub struct GridSize {
    pub cols: usize,
    pub lines: usize,
}

impl Dimensions for GridSize {
    fn total_lines(&self) -> usize {
        self.lines
    }
    fn screen_lines(&self) -> usize {
        self.lines
    }
    fn columns(&self) -> usize {
        self.cols
    }
}

pub struct Session<L: EventListener = Listener> {
    pub term: Arc<FairMutex<Term<L>>>,
    sender: EventLoopSender,
    pid: u32,
}

/// `$SHELL`, else the login shell from the user database (apps opened from
/// Finder may not inherit `SHELL`), else `/bin/zsh`.
pub fn shell_program() -> String {
    pick_shell(std::env::var("SHELL").ok(), passwd_shell())
}

fn pick_shell(env: Option<String>, passwd: Option<String>) -> String {
    env.filter(|s| !s.is_empty())
        .or(passwd.filter(|s| !s.is_empty()))
        .unwrap_or_else(|| "/bin/zsh".into())
}

fn passwd_shell() -> Option<String> {
    // SAFETY: getpwuid returns static storage or null; we copy the string out immediately.
    unsafe {
        let pw = libc::getpwuid(libc::getuid());
        if pw.is_null() || (*pw).pw_shell.is_null() {
            return None;
        }
        Some(std::ffi::CStr::from_ptr((*pw).pw_shell).to_string_lossy().into_owned())
    }
}

/// Apps launched from the Dock get no locale; without one zsh mangles UTF-8.
fn needs_lang(vars: &[Option<String>]) -> bool {
    vars.iter().all(|v| v.as_deref().is_none_or(str::is_empty))
}

impl<L: EventListener + Clone + Send + 'static> Session<L> {
    pub fn spawn(
        size: WindowSize,
        scrollback: usize,
        listener: L,
        banner: Option<String>,
        cwd: Option<PathBuf>,
        extra_env: Vec<(String, String)>,
    ) -> std::io::Result<Self> {
        let mut env = HashMap::from([
            ("TERM".to_string(), "xterm-256color".to_string()),
            ("COLORTERM".to_string(), "truecolor".to_string()),
            ("TERM_PROGRAM".to_string(), "cardumen".to_string()),
        ]);
        env.extend(extra_env);
        let locale = ["LANG", "LC_ALL", "LC_CTYPE"].map(|k| std::env::var(k).ok());
        if needs_lang(&locale) {
            env.insert("LANG".into(), "en_US.UTF-8".into());
        }
        let options = tty::Options {
            shell: Some(tty::Shell::new(shell_program(), vec!["-l".into()])),
            working_directory: cwd.or_else(|| std::env::var_os("HOME").map(Into::into)),
            drain_on_exit: false,
            env,
        };
        let grid = GridSize { cols: size.num_cols as usize, lines: size.num_lines as usize };
        let config = term::Config {
            scrolling_history: scrollback,
            default_cursor_style: ansi::CursorStyle { shape: ansi::CursorShape::Beam, blinking: true },
            ..Default::default()
        };
        let mut term = Term::new(config, &grid, listener.clone());
        if let Some(banner) = banner {
            ansi::Processor::<ansi::StdSyncHandler>::new().advance(&mut term, banner.as_bytes());
        }
        let term = Arc::new(FairMutex::new(term));
        let pty = tty::new(&options, size, 0)?;
        let pid = pty.child().id();
        let event_loop = EventLoop::new(term.clone(), listener, pty, false, false)?;
        let sender = event_loop.channel();
        event_loop.spawn();
        Ok(Self { term, sender, pid })
    }

    pub fn write(&self, bytes: impl Into<Cow<'static, [u8]>>) {
        let _ = self.sender.send(Msg::Input(bytes.into()));
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// The shell's current directory (what `pwd` prints at the prompt).
    pub fn cwd(&self) -> Option<PathBuf> {
        cwd_of(self.pid)
    }

    pub fn resize(&self, size: WindowSize) {
        self.term.lock().resize(GridSize { cols: size.num_cols as usize, lines: size.num_lines as usize });
        let _ = self.sender.send(Msg::Resize(size));
    }
}

/// Current directory of a process.
pub fn cwd_of(pid: u32) -> Option<PathBuf> {
    let size = std::mem::size_of::<libc::proc_vnodepathinfo>() as i32;
    // SAFETY: proc_pidinfo fills at most `size` bytes of the zeroed struct we own;
    // vip_path is a NUL-terminated C string inside it.
    unsafe {
        let mut info: libc::proc_vnodepathinfo = std::mem::zeroed();
        let n = libc::proc_pidinfo(pid as i32, libc::PROC_PIDVNODEPATHINFO, 0, (&raw mut info).cast(), size);
        if n != size {
            return None;
        }
        let path = std::ffi::CStr::from_ptr(info.pvi_cdir.vip_path.as_ptr().cast());
        Some(PathBuf::from(path.to_string_lossy().into_owned()))
    }
}

impl<L: EventListener> Drop for Session<L> {
    fn drop(&mut self) {
        let _ = self.sender.send(Msg::Shutdown);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alacritty_terminal::term::TermMode;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    #[derive(Clone)]
    struct Probe(mpsc::Sender<String>);
    impl EventListener for Probe {
        fn send_event(&self, e: Event) {
            let name = match e {
                Event::Exit => "exit",
                Event::ChildExit(_) => "child-exit",
                _ => return,
            };
            let _ = self.0.send(name.into());
        }
    }

    fn size(cols: u16, lines: u16) -> WindowSize {
        WindowSize { num_cols: cols, num_lines: lines, cell_width: 8, cell_height: 16 }
    }

    fn start(cols: u16) -> (Session<Probe>, mpsc::Receiver<String>) {
        let (tx, rx) = mpsc::channel();
        let s = Session::spawn(size(cols, 24), 1000, Probe(tx), Some("BANNER\r\n".into()), None, vec![]).unwrap();
        (s, rx)
    }

    fn screen(s: &Session<Probe>) -> String {
        let term = s.term.lock();
        let mut out = String::new();
        let mut last = None;
        for cell in term.grid().display_iter() {
            if last.is_some_and(|l| l != cell.point.line) {
                out.push('\n');
            }
            last = Some(cell.point.line);
            out.push(cell.c);
        }
        out
    }

    fn wait(what: &str, timeout: u64, mut ok: impl FnMut() -> bool) {
        let end = Instant::now() + Duration::from_secs(timeout);
        while !ok() {
            assert!(Instant::now() < end, "timed out waiting for {what}");
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Wait until the login shell is interactive (zsh enables bracketed paste at its prompt).
    fn ready(s: &Session<Probe>) {
        wait("prompt", 15, || s.term.lock().mode().contains(TermMode::BRACKETED_PASTE));
    }

    #[test]
    fn shell_and_locale_fallbacks() {
        assert_eq!(pick_shell(Some("/bin/bash".into()), Some("/bin/zsh".into())), "/bin/bash");
        assert_eq!(pick_shell(Some(String::new()), Some("/opt/homebrew/bin/fish".into())), "/opt/homebrew/bin/fish");
        assert_eq!(pick_shell(None, None), "/bin/zsh");
        assert!(needs_lang(&[None, Some(String::new()), None]));
        assert!(!needs_lang(&[None, None, Some("UTF-8".into())]));
    }

    #[test]
    fn banner_then_shell_output() {
        let (s, _rx) = start(80);
        assert!(screen(&s).starts_with("BANNER"));
        ready(&s);
        s.write(b"echo cardumen-$((40+2))\r".to_vec());
        wait("echo output", 10, || screen(&s).contains("cardumen-42"));
    }

    #[test]
    fn vim_uses_alternate_screen_and_restores() {
        let (s, _rx) = start(80);
        ready(&s);
        s.write(b"vim -u NONE -N\r".to_vec());
        wait("vim alt screen", 10, || s.term.lock().mode().contains(TermMode::ALT_SCREEN));
        s.write(b"\x1b:q!\r".to_vec());
        wait("vim exit", 10, || !s.term.lock().mode().contains(TermMode::ALT_SCREEN));
    }

    #[test]
    fn resize_reaches_the_shell() {
        let (s, _rx) = start(80);
        ready(&s);
        s.resize(size(57, 20));
        s.write(b"echo cols=$(tput cols)\r".to_vec());
        wait("tput cols", 10, || screen(&s).contains("cols=57"));
    }

    #[test]
    fn scrollback_keeps_history() {
        let (s, _rx) = start(80);
        ready(&s);
        s.write(b"for i in {1..200}; do echo line$i; done\r".to_vec());
        wait("output", 10, || screen(&s).contains("line200"));
        let term = s.term.lock();
        assert!(term.grid().history_size() > 150, "history {}", term.grid().history_size());
    }

    #[test]
    fn ctrl_c_interrupts() {
        let (s, _rx) = start(80);
        ready(&s);
        s.write(b"sleep 30\r".to_vec());
        std::thread::sleep(Duration::from_millis(300));
        s.write(vec![3u8]);
        s.write(b"echo after-int\r".to_vec());
        wait("prompt after ^C", 5, || screen(&s).contains("after-int"));
    }

    #[test]
    fn starts_in_cwd_and_tracks_cd() {
        let (tx, _rx) = mpsc::channel();
        let s = Session::spawn(size(80, 24), 100, Probe(tx), None, Some("/usr".into()), vec![]).unwrap();
        ready(&s);
        assert_eq!(s.cwd().unwrap(), PathBuf::from("/usr"));
        s.write(b"cd /tmp\r".to_vec());
        wait("cd", 5, || s.cwd().is_some_and(|p| p.ends_with("tmp")));
    }

    #[test]
    fn exit_ends_session() {
        let (s, rx) = start(80);
        ready(&s);
        s.write(b"exit\r".to_vec());
        let ev = rx.recv_timeout(Duration::from_secs(10)).expect("exit event");
        assert!(ev == "exit" || ev == "child-exit", "{ev}");
    }

    #[test]
    fn claude_code_starts() {
        if std::process::Command::new("which").arg("claude").output().map_or(true, |o| !o.status.success()) {
            return;
        }
        let (s, _rx) = start(100);
        ready(&s);
        s.write(b"claude\r".to_vec());
        // Its UI (or the folder-trust prompt) names "Claude"; quit with two ^C.
        wait("claude UI", 30, || screen(&s).contains("Claude"));
        s.write(vec![3u8]);
        std::thread::sleep(Duration::from_millis(200));
        s.write(vec![3u8]);
    }
}
