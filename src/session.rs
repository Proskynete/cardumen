//! One shell: PTY + VT state + the I/O thread. Independent of any window,
//! so tabs and splits can hold several later.
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::event_loop::{EventLoop, EventLoopSender, Msg};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::{self, Term};
use alacritty_terminal::tty;
use alacritty_terminal::vte::ansi;
use winit::event_loop::EventLoopProxy;

pub enum UserEvent {
    Term(Event),
}

#[derive(Clone)]
pub struct Listener(pub EventLoopProxy<UserEvent>);

impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        let _ = self.0.send_event(UserEvent::Term(event));
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

pub struct Session {
    pub term: Arc<FairMutex<Term<Listener>>>,
    sender: EventLoopSender,
}

pub fn shell_program() -> String {
    std::env::var("SHELL").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "/bin/zsh".into())
}

impl Session {
    pub fn spawn(size: WindowSize, scrollback: usize, listener: Listener, banner: Option<String>) -> std::io::Result<Self> {
        let env = HashMap::from([
            ("TERM".to_string(), "xterm-256color".to_string()),
            ("COLORTERM".to_string(), "truecolor".to_string()),
            ("TERM_PROGRAM".to_string(), "cardumen".to_string()),
        ]);
        let options = tty::Options {
            shell: Some(tty::Shell::new(shell_program(), vec!["-l".into()])),
            working_directory: std::env::var_os("HOME").map(Into::into),
            drain_on_exit: false,
            env,
        };
        let grid = GridSize { cols: size.num_cols as usize, lines: size.num_lines as usize };
        let config = term::Config { scrolling_history: scrollback, ..Default::default() };
        let mut term = Term::new(config, &grid, listener.clone());
        if let Some(banner) = banner {
            ansi::Processor::<ansi::StdSyncHandler>::new().advance(&mut term, banner.as_bytes());
        }
        let term = Arc::new(FairMutex::new(term));
        let pty = tty::new(&options, size, 0)?;
        let event_loop = EventLoop::new(term.clone(), listener, pty, false, false)?;
        let sender = event_loop.channel();
        event_loop.spawn();
        Ok(Self { term, sender })
    }

    pub fn write(&self, bytes: impl Into<Cow<'static, [u8]>>) {
        let _ = self.sender.send(Msg::Input(bytes.into()));
    }

    pub fn resize(&self, size: WindowSize) {
        self.term.lock().resize(GridSize { cols: size.num_cols as usize, lines: size.num_lines as usize });
        let _ = self.sender.send(Msg::Resize(size));
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.sender.send(Msg::Shutdown);
    }
}
