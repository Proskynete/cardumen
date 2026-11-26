mod config;
mod context;
mod font;
mod input;
mod layout;
mod renderer;
mod session;
mod status;
mod theme;
mod usage;
mod welcome;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use alacritty_terminal::event::{Event, WindowSize};
use alacritty_terminal::grid::Scroll;
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::TermMode;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey};
use winit::platform::macos::WindowAttributesExtMacOS;
use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
use winit::window::{Window, WindowId};

use config::Config;
use font::Fonts;
use layout::{Arrow, Dir, PaneId, Rect, Tabs};
use font::Face;
use renderer::{Chrome, Frame, Renderer};
use session::{Listener, Session, UserEvent};
use theme::Rgb;

// Logical sizes (points); multiplied by the scale factor when used.
/// Titlebar band holding the tabs, next to the traffic lights.
const BAR_H: f32 = 34.0;
const TABS_X: f32 = 80.0;
const TAB_MAX_W: f32 = 180.0;
const TAB_PAD: f32 = 12.0;
const PLUS_W: f32 = 28.0;
/// Inner padding of each pane.
const PAD: f32 = 8.0;
const DIVIDER: f32 = 8.0;
const GUTTER_W: f32 = 8.0;
const HEADER_H: f32 = 28.0;

const HAIRLINE: Rgb = [0x1F, 0x33, 0x3D];
const SIDEBAR_W: f32 = 230.0;
const PANEL_W: f32 = 320.0;
const MERGED: Rgb = [0xC3, 0x8F, 0xD9];
const ERROR: Rgb = [0xE1, 0x57, 0x57];
const PANEL: Rgb = [0x0C, 0x17, 0x1D];
const GUTTER: Rgb = [0x06, 0x0D, 0x11];
const SELECTED: Rgb = [0x13, 0x25, 0x2E];
const CARD: Rgb = [0x0F, 0x1E, 0x25];
const BODY: Rgb = [0xC9, 0xD6, 0xD8];
const WARM: Rgb = [0xF2, 0xA6, 0x5A];
const SUCCESS: Rgb = [0x4F, 0xB4, 0x77];

const BLINK: Duration = Duration::from_millis(530);
const MULTI_CLICK: Duration = Duration::from_millis(400);

struct Pane {
    session: Session,
    title: Option<String>,
    rect: Rect,
    claude: Option<status::State>,
    ctx: Option<Box<context::Context>>,
    /// What Claude asked permission for, while waiting.
    ask: Option<String>,
    last_message: Option<String>,
}

#[derive(Clone, Copy)]
enum UiHit {
    PanelTab(bool),
    ClosePane(PaneId),
    Range(usage::Range),
    CloseUsage,
    OpenUsage,
}

enum TabHit {
    Tab(usize),
    Close(usize),
    New,
}

struct State {
    window: Arc<Window>,
    renderer: Renderer,
    fonts: Fonts,
    config: Config,
    proxy: EventLoopProxy<UserEvent>,
    clipboard: Option<arboard::Clipboard>,
    font_points: f32,
    panes: HashMap<PaneId, Pane>,
    tabs: Tabs,
    next_id: PaneId,
    tab_hits: Vec<(Rect, TabHit)>,
    mods: ModifiersState,
    window_focused: bool,
    cursor_on: bool,
    next_blink: Instant,
    mouse: PhysicalPosition<f64>,
    selecting: Option<PaneId>,
    last_click: Option<(Instant, Point, u8)>,
    scroll_px: f64,
    pressed_button: Option<(PaneId, u8)>,
    /// Tab being renamed and its edit buffer.
    renaming: Option<(usize, String)>,
    last_tab_click: Option<(Instant, usize)>,
    dragging_tab: Option<TabDrag>,
    /// Drawn x of each tab (by tab id), eased towards its slot every frame.
    tab_x: HashMap<u64, f32>,
    animating: bool,
    hovered_tab: Option<usize>,
    tab_w: f32,
    socket: PathBuf,
    sidebar: bool,
    sidebar_hits: Vec<(Rect, PaneId)>,
    next_tick: Instant,
    panel: bool,
    panel_changes: bool,
    usage_open: bool,
    usage_range: usage::Range,
    usage: Option<Box<usage::Summary>>,
    usage_loading: bool,
    footer_hover: bool,
    ui_hits: Vec<(Rect, UiHit)>,
    ctx_tx: std::sync::mpsc::Sender<context::Request>,
}

struct TabDrag {
    index: usize,
    press_x: f32,
    /// Pointer x minus the tab's left edge at press time, so the tab doesn't jump.
    grab: f32,
    moving: bool,
}

struct App {
    socket: PathBuf,
    config: Config,
    proxy: EventLoopProxy<UserEvent>,
    state: Option<State>,
}

impl State {
    fn scale(&self) -> f32 {
        self.window.scale_factor() as f32
    }

    fn sidebar_w(&self) -> f32 {
        if self.sidebar { SIDEBAR_W * self.scale() } else { 0.0 }
    }

    fn panel_w(&self) -> f32 {
        if self.panel { PANEL_W * self.scale() } else { 0.0 }
    }

    fn content_rect(&self) -> Rect {
        let size = self.window.inner_size();
        let s = self.scale();
        // The gutter around the pane cards.
        let (top, left) = (BAR_H * s, self.sidebar_w() + GUTTER_W * s);
        let w = size.width as f32 - left - self.panel_w() - GUTTER_W * s;
        Rect { x: left, y: top, w: w.max(1.0), h: (size.height as f32 - top - GUTTER_W * s).max(1.0) }
    }

    /// Tell the context worker which shells exist.
    fn sync_panes(&self) {
        let panes = self.panes.iter().map(|(id, p)| (*id, p.session.pid())).collect();
        let _ = self.ctx_tx.send(context::Request::Panes(panes));
    }

    /// Where the first tab starts: after the traffic lights, or after the sidebar.
    fn tabs_x(&self) -> f32 {
        let s = self.scale();
        if self.sidebar { self.sidebar_w() + 8.0 * s } else { TABS_X * s }
    }

    fn grid_size(&self, r: Rect) -> WindowSize {
        let s = self.scale();
        let (cw, ch) = (self.fonts.cell_w, self.fonts.cell_h);
        WindowSize {
            num_cols: ((r.w - 2.0 * PAD * s) / cw).floor().max(2.0) as u16,
            num_lines: ((r.h - (HEADER_H + PAD) * s) / ch).floor().max(1.0) as u16,
            cell_width: cw as u16,
            cell_height: ch as u16,
        }
    }

    /// Terminal grid starts under the pane header.
    fn grid_origin(&self, r: Rect) -> (f32, f32) {
        let s = self.scale();
        (r.x + PAD * s, r.y + HEADER_H * s)
    }

    fn tab_layout(&self, tab: usize) -> (Vec<(PaneId, Rect)>, Vec<Rect>) {
        let (mut panes, mut dividers) = (vec![], vec![]);
        self.tabs.list[tab].root.layout(self.content_rect(), DIVIDER * self.scale(), &mut panes, &mut dividers);
        (panes, dividers)
    }

    fn focused_id(&self) -> PaneId {
        self.tabs.active().focused
    }

    fn focused(&self) -> &Pane {
        &self.panes[&self.focused_id()]
    }

    /// Give every pane its rect and resize PTYs whose rect changed.
    fn relayout(&mut self) {
        for t in 0..self.tabs.list.len() {
            for (id, rect) in self.tab_layout(t).0 {
                let size = self.grid_size(rect);
                if let Some(p) = self.panes.get_mut(&id) {
                    if p.rect != rect {
                        p.rect = rect;
                        p.session.resize(size);
                    }
                }
            }
        }
        self.update_title();
        self.window.request_redraw();
    }

    /// Start a shell for `id`, which must already be in the active tab's tree.
    fn spawn_pane(&mut self, id: PaneId, banner: bool, cwd: Option<PathBuf>) -> bool {
        let Some(rect) = self.tab_layout(self.tabs.active).0.into_iter().find(|(p, _)| *p == id).map(|(_, r)| r) else {
            return false;
        };
        let size = self.grid_size(rect);
        let banner = (banner && self.config.welcome).then(|| {
            let monitor = self.window.current_monitor();
            let display = monitor.map(|m| {
                let s = m.size().to_logical::<u32>(m.scale_factor());
                let hz = m.refresh_rate_millihertz().map_or(String::new(), |r| format!(", {} Hz", (r + 500) / 1000));
                // winit falls back to "Monitor #<id>" when macOS gives no name; that adds nothing.
                let name = m.name().filter(|n| !n.starts_with("Monitor #")).map_or(String::new(), |n| format!("{n} · "));
                format!("{name}{}×{} @ {}x{hz}", s.width, s.height, m.scale_factor())
            });
            let font = format!("{} ({}pt)", self.fonts.family, self.font_points);
            let info = welcome::SysInfo::read(self.config.user_name.as_deref(), &font, display);
            welcome::banner(&info, size.num_cols as usize)
        });
        let env = vec![
            ("CARDUMEN_SOCKET".to_string(), self.socket.to_string_lossy().into_owned()),
            ("CARDUMEN_PANE".to_string(), id.to_string()),
        ];
        match Session::spawn(size, self.config.scrollback, Listener(self.proxy.clone(), id), banner, cwd, env) {
            Ok(session) => {
                self.panes.insert(id, Pane { session, title: None, rect, claude: None, ctx: None, ask: None, last_message: None });
                self.sync_panes();
                true
            }
            Err(e) => {
                eprintln!("cardumen: cannot start shell: {e}");
                false
            }
        }
    }

    fn take_id(&mut self) -> PaneId {
        self.next_id += 1;
        self.next_id
    }

    fn focused_cwd(&self) -> Option<PathBuf> {
        self.panes.get(&self.tabs.list.get(self.tabs.active)?.focused)?.session.cwd()
    }

    fn new_tab(&mut self) {
        let cwd = if self.tabs.list.is_empty() { None } else { self.focused_cwd() };
        let id = self.take_id();
        self.tabs.open(id);
        if !self.spawn_pane(id, true, cwd) {
            self.tabs.remove_pane(id);
        }
        self.relayout();
    }

    fn split(&mut self, dir: Dir) {
        let cwd = self.focused_cwd();
        let (target, id) = (self.focused_id(), self.take_id());
        self.tabs.active_mut().root.split(target, id, dir);
        self.tabs.active_mut().focused = id;
        if !self.spawn_pane(id, false, cwd) {
            self.tabs.remove_pane(id);
        }
        self.relayout();
    }

    /// Returns true when no tabs are left (the app should quit).
    fn close_pane(&mut self, id: PaneId) -> bool {
        if self.panes.remove(&id).is_none() {
            return self.tabs.list.is_empty();
        }
        self.renaming = None;
        self.tabs.remove_pane(id);
        self.sync_panes();
        if self.tabs.list.is_empty() {
            return true;
        }
        self.reset_blink();
        self.relayout();
        false
    }

    /// Close tab `i` and all its panes. Returns true when no tabs are left.
    fn close_tab(&mut self, i: usize) -> bool {
        let Some(tab) = self.tabs.list.get(i) else { return false };
        let mut quit = false;
        for id in tab.root.leaves() {
            quit = self.close_pane(id);
        }
        quit
    }

    fn tab_title(&self, i: usize) -> String {
        if let Some(name) = &self.tabs.list[i].name {
            return name.clone();
        }
        let Some(pane) = self.panes.get(&self.tabs.list[i].focused) else { return String::new() };
        // Shell prompts set "user@host: path" on every tab; the folder name says more.
        let user_prefix = format!("{}@", std::env::var("USER").unwrap_or_default());
        if let Some(t) = pane.title.as_deref().filter(|t| !t.is_empty() && !t.starts_with(&user_prefix)) {
            return t.to_string();
        }
        let home = std::env::var_os("HOME").map(PathBuf::from);
        match pane.session.cwd() {
            Some(p) if Some(&p) == home.as_ref() => "~".into(),
            Some(p) => p.file_name().map_or("/".into(), |n| n.to_string_lossy().into_owned()),
            None => "zsh".into(),
        }
    }

    /// A pane's own label: its program title (not the shell's user@host), else its folder.
    fn pane_label(&self, id: PaneId) -> String {
        let Some(pane) = self.panes.get(&id) else { return String::new() };
        let user_prefix = format!("{}@", std::env::var("USER").unwrap_or_default());
        if let Some(t) = pane.title.as_deref().filter(|t| !t.is_empty() && !t.starts_with(&user_prefix)) {
            return t.to_string();
        }
        folder_name(pane.session.cwd())
    }

    fn tab_waiting(&self, i: usize) -> bool {
        self.tabs.list[i]
            .root
            .leaves()
            .iter()
            .any(|id| self.panes.get(id).and_then(|p| p.claude).is_some_and(|c| c.claude == status::Claude::Waiting))
    }

    /// Sidebar: summary pills, then panes grouped by project with their Claude state.
    fn sidebar_chrome(&mut self, out: &mut Vec<Chrome>) {
        self.sidebar_hits.clear();
        if !self.sidebar {
            return;
        }
        let s = self.scale();
        let (w, height) = (self.sidebar_w(), self.window.inner_size().height as f32);
        out.push(Chrome::Rect { rect: Rect { x: 0.0, y: 0.0, w, h: height }, color: PANEL });
        out.push(Chrome::Rect { rect: Rect { x: w - s, y: 0.0, w: s, h: height }, color: HAIRLINE });

        // (project, pane id, tab index), grouped by first appearance in tab order.
        let mut groups: Vec<(String, Vec<(PaneId, usize)>)> = vec![];
        for (i, tab) in self.tabs.list.iter().enumerate() {
            for id in tab.root.leaves() {
                let project = project_name(self.panes.get(&id).and_then(|p| p.session.cwd()));
                match groups.iter_mut().find(|(p, _)| *p == project) {
                    Some((_, rows)) => rows.push((id, i)),
                    None => groups.push((project, vec![(id, i)])),
                }
            }
        }
        let count = |c: status::Claude| self.panes.values().filter(|p| p.claude.is_some_and(|x| x.claude == c)).count();
        let (waiting, working) = (count(status::Claude::Waiting), count(status::Claude::Working));

        let mut y = BAR_H * s + 2.0 * s;
        let mut x = 14.0 * s;
        for (n, label, color) in [(waiting, "te espera", WARM), (working, "trabajando", theme::ACCENT)] {
            let color = if n > 0 { color } else { theme::MUTED };
            x += self.pill(out, x, y, &format!("{n} {label}"), color, PANEL) + 6.0 * s;
        }
        y += 34.0 * s;

        // Footer: button to the usage dashboard.
        let footer = Rect { x: 8.0 * s, y: height - 44.0 * s, w: w - 16.0 * s, h: 34.0 * s };
        let hover = footer.contains(self.mouse.x as f32, self.mouse.y as f32);
        out.push(Chrome::Rect { rect: Rect { x: 0.0, y: footer.y - 8.0 * s, w: w - s, h: s }, color: HAIRLINE });
        if hover {
            out.push(Chrome::Round { rect: footer, radius: 8.0 * s, color: CARD });
        }
        let fy = footer.y + (footer.h - 12.5 * s * 1.3) / 2.0;
        out.push(label(footer.x + 10.0 * s, fy, "\u{f080}".into(), Face::Mono, 12.5 * s, false, theme::MUTED, 18.0 * s));
        out.push(label(footer.x + 32.0 * s, fy, "Uso".into(), Face::Sans, 12.5 * s, false, theme::FOREGROUND, 80.0 * s));
        let key_w = self.fonts.run("⌘U", Face::Sans, 11.0 * s, false).width;
        out.push(label(footer.x + footer.w - 10.0 * s - key_w, fy + s, "⌘U".into(), Face::Sans, 11.0 * s, false, theme::MUTED, key_w + s));
        self.ui_hits.push((footer, UiHit::OpenUsage));
        let height = footer.y - 12.0 * s;

        let focused = self.focused_id();
        let row_w = w - 16.0 * s;
        for (project, rows) in groups {
            if y > height {
                break;
            }
            out.push(label(18.0 * s, y, project.to_uppercase(), Face::Sans, 10.5 * s, true, theme::MUTED, row_w));
            y += 20.0 * s;
            for (id, tab) in rows {
                if y + 62.0 * s > height {
                    break;
                }
                let row = Rect { x: 8.0 * s, y, w: row_w, h: 62.0 * s };
                if id == focused {
                    out.push(Chrome::Round { rect: row, radius: 10.0 * s, color: SELECTED });
                }
                let pane = self.panes.get(&id);
                let state = pane.and_then(|p| p.claude);
                let ctx = pane.and_then(|p| p.ctx.as_deref());
                let (dot, detail) = match state {
                    Some(st) => match st.claude {
                        status::Claude::Waiting => (WARM, format!("te espera · {}", status::elapsed(st.since))),
                        status::Claude::Working => (theme::ACCENT, format!("trabajando · {}", status::elapsed(st.since))),
                        status::Claude::Done => (SUCCESS, format!("lista · {}", status::elapsed(st.since))),
                    },
                    None => (theme::MUTED, "sin Claude".into()),
                };
                let badge = match (state.map(|s| s.claude), ctx) {
                    (Some(status::Claude::Waiting), _) => Some(("te toca".to_string(), WARM)),
                    (_, Some(c)) => Some((c.step().label(), theme::MUTED)),
                    _ => None,
                };
                let mut meta = ctx.and_then(|c| c.branch.clone()).unwrap_or_else(|| folder_name(pane.and_then(|p| p.session.cwd())));
                if let Some(context::PrInfo::Found(pr)) = ctx.map(|c| &c.pr) {
                    meta.push_str(&format!(" · {}", pr.number));
                }
                let title = self.tabs.list[tab].name.clone().unwrap_or_else(|| self.pane_label(id));

                let text_x = row.x + 24.0 * s;
                let text_w = row.x + row.w - 10.0 * s - text_x;
                out.push(Chrome::Round { rect: Rect { x: row.x + 10.0 * s, y: y + 9.0 * s, w: 7.0 * s, h: 7.0 * s }, radius: 3.5 * s, color: dot });
                let badge_w = badge.as_ref().map_or(0.0, |(t, _)| self.fonts.run(t, Face::Sans, 10.5 * s, true).width + 18.0 * s);
                let title_w = self.fonts.run(&title, Face::Sans, 13.0 * s, true).width.min(text_w - badge_w);
                out.push(label(text_x, y + 3.0 * s, title, Face::Sans, 13.0 * s, true, theme::FOREGROUND, text_w - badge_w));
                if let Some((t, c)) = badge {
                    self.pill(out, text_x + title_w + 6.0 * s, y + 4.0 * s, &t, c, if id == focused { SELECTED } else { PANEL });
                }
                out.push(label(text_x, y + 22.0 * s, detail, Face::Sans, 12.0 * s, false, if state.is_some() { dot } else { theme::MUTED }, text_w));
                out.push(label(text_x, y + 40.0 * s, "\u{e0a0}".into(), Face::Mono, 11.0 * s, false, theme::MUTED, 14.0 * s));
                out.push(label(text_x + 14.0 * s, y + 40.0 * s, meta, Face::Sans, 11.5 * s, false, theme::MUTED, text_w - 14.0 * s));
                self.sidebar_hits.push((row, id));
                y += 66.0 * s;
            }
            y += 8.0 * s;
        }
    }

    /// A tinted rounded badge with `text`; returns its width.
    fn pill(&mut self, out: &mut Vec<Chrome>, x: f32, y: f32, text: &str, color: Rgb, on: Rgb) -> f32 {
        let s = self.scale();
        let px = 10.5 * s;
        let w = self.fonts.run(text, Face::Sans, px, true).width + 14.0 * s;
        let h = 18.0 * s;
        out.push(Chrome::Round { rect: Rect { x, y, w, h }, radius: 6.0 * s, color: mix(color, on, 0.16) });
        out.push(label(x + 7.0 * s, y + (h - px * 1.3) / 2.0, text.into(), Face::Sans, px, true, color, w));
        w
    }

    /// Greedy word wrap by measured width.
    fn wrap_px(&mut self, text: &str, px: f32, max_w: f32) -> Vec<String> {
        let mut lines = vec![];
        for para in text.lines() {
            let mut line = String::new();
            for word in para.split_whitespace() {
                let candidate = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
                if !line.is_empty() && self.fonts.run(&candidate, Face::Sans, px, false).width > max_w {
                    lines.push(std::mem::replace(&mut line, word.to_string()));
                } else {
                    line = candidate;
                }
            }
            lines.push(line);
        }
        while lines.last().is_some_and(String::is_empty) {
            lines.pop();
        }
        lines
    }

    /// Right "Resumen" panel for the focused pane: step, Claude, PR/MR, changes.
    fn panel_chrome(&mut self, out: &mut Vec<Chrome>) {
        let pw = self.panel_w();
        if pw == 0.0 {
            return;
        }
        let s = self.scale();
        let size = self.window.inner_size();
        let (x0, height) = (size.width as f32 - pw, size.height as f32);
        out.push(Chrome::Rect { rect: Rect { x: x0, y: 0.0, w: pw, h: height }, color: PANEL });
        out.push(Chrome::Rect { rect: Rect { x: x0, y: 0.0, w: s, h: height }, color: HAIRLINE });
        let left = x0 + 18.0 * s;
        let tw = pw - 36.0 * s;
        let id = self.focused_id();
        let Some(pane) = self.panes.get(&id) else { return };
        let title = self.tabs.active().name.clone().unwrap_or_else(|| self.pane_label(id));
        let ctx = pane.ctx.clone();
        let (claude, ask, last) = (pane.claude.map(|c| c.claude), pane.ask.clone(), pane.last_message.clone());

        let mut y = BAR_H * s;
        out.push(label(left, y, title, Face::Sans, 16.0 * s, true, theme::FOREGROUND, tw));
        y += 30.0 * s;
        // Resumen / Cambios switch.
        let mut x = left;
        for (name, changes) in [("Resumen", false), ("Cambios", true)] {
            let on = self.panel_changes == changes;
            let w = self.fonts.run(name, Face::Sans, 12.5 * s, on).width;
            out.push(label(x, y, name.into(), Face::Sans, 12.5 * s, on, if on { theme::FOREGROUND } else { theme::MUTED }, w + s));
            if on {
                out.push(Chrome::Round { rect: Rect { x, y: y + 21.0 * s, w, h: 2.0 * s }, radius: s, color: theme::ACCENT });
            }
            self.ui_hits.push((Rect { x: x - 4.0 * s, y: y - 4.0 * s, w: w + 8.0 * s, h: 28.0 * s }, UiHit::PanelTab(changes)));
            x += w + 18.0 * s;
        }
        y += 23.0 * s;
        out.push(Chrome::Rect { rect: Rect { x: x0, y, w: pw, h: s }, color: HAIRLINE });
        y += 14.0 * s;

        let section = |out: &mut Vec<Chrome>, y: &mut f32, t: &str| {
            out.push(label(left, *y, t.to_uppercase(), Face::Sans, 10.5 * s, true, theme::MUTED, tw));
            *y += 20.0 * s;
        };
        let Some(ctx) = ctx else {
            out.push(label(left, y, "Fuera de un repo git".into(), Face::Sans, 12.5 * s, false, theme::MUTED, tw));
            return;
        };

        if self.panel_changes {
            self.changes_list(out, &ctx, left, tw, y, height, 30);
            return;
        }

        // Paso: rail with circles.
        section(out, &mut y, "Paso");
        let step = ctx.step();
        let stages: Vec<(usize, String)> = [
            (0, "Spec".to_string()),
            (1, match (step, &ctx.spec) {
                (context::Step::Tasks(d, t), _) => format!("Tareas {d}/{t}"),
                (_, Some(sp)) if sp.total > 0 => format!("{}/{}", sp.done, sp.total),
                _ => "Tareas".into(),
            }),
            (2, "PR".into()),
            (3, "Review".into()),
            (4, "Merge".into()),
        ]
        .into_iter()
        .filter(|(k, _)| ctx.spec.is_some() || *k >= 2)
        .collect();
        let n = stages.len();
        let (inset, cy) = (16.0 * s, y + 10.0 * s);
        let gap = (tw - 2.0 * inset) / (n.max(2) - 1) as f32;
        let cur = stages.iter().position(|(k, _)| *k == step.index()).unwrap_or(0);
        out.push(Chrome::Rect { rect: Rect { x: left + inset, y: cy - s, w: tw - 2.0 * inset, h: 2.0 * s }, color: HAIRLINE });
        out.push(Chrome::Rect { rect: Rect { x: left + inset, y: cy - s, w: gap * cur as f32, h: 2.0 * s }, color: SUCCESS });
        for (i, (_, name)) in stages.iter().enumerate() {
            let cx = left + inset + gap * i as f32;
            let circle = |d: f32, color: Rgb| Chrome::Round { rect: Rect { x: cx - d / 2.0, y: cy - d / 2.0, w: d, h: d }, radius: d / 2.0, color };
            match i.cmp(&cur) {
                std::cmp::Ordering::Less => out.push(circle(14.0 * s, SUCCESS)),
                std::cmp::Ordering::Equal => {
                    out.push(circle(24.0 * s, mix(theme::ACCENT, PANEL, 0.18)));
                    out.push(circle(14.0 * s, theme::ACCENT));
                    out.push(circle(9.0 * s, PANEL));
                }
                std::cmp::Ordering::Greater => {
                    out.push(circle(14.0 * s, HAIRLINE));
                    out.push(circle(10.0 * s, PANEL));
                }
            }
            let px = 10.5 * s;
            let lw = self.fonts.run(name, Face::Sans, px, i == cur).width;
            let color = if i == cur { theme::FOREGROUND } else { theme::MUTED };
            out.push(label(cx - lw / 2.0, cy + 14.0 * s, name.clone(), Face::Sans, px, i == cur, color, lw + s));
        }
        y += 50.0 * s;
        if let Some(sp) = &ctx.spec {
            out.push(label(left, y, format!("openspec · {}", sp.change), Face::Sans, 11.5 * s, false, theme::MUTED, tw));
            y += 24.0 * s;
        }

        // Claude.
        section(out, &mut y, "Claude");
        let body_px = 12.5 * s;
        let line_h = 18.0 * s;
        let (pill, text) = match (claude, ask, last) {
            (Some(status::Claude::Waiting), ask, _) => (Some(("te espera", WARM)), ask.map(|a| format!("Pide permiso: {a}"))),
            (_, _, Some(msg)) => (None, Some(msg)),
            (Some(status::Claude::Done), _, None) => (Some(("lista", SUCCESS)), None),
            (Some(_), _, None) => (Some(("trabajando", theme::ACCENT)), None),
            (None, _, None) => (None, Some("Sin sesión de Claude en este panel.".into())),
        };
        let mut lines = text.map(|t| self.wrap_px(&t, body_px, tw - 24.0 * s)).unwrap_or_default();
        let cut = lines.len() > 8;
        lines.truncate(8);
        if cut {
            if let Some(l) = lines.last_mut() {
                l.push('…');
            }
        }
        let card_h = 20.0 * s + lines.len() as f32 * line_h + if pill.is_some() { 24.0 * s } else { 0.0 };
        out.push(Chrome::Round { rect: Rect { x: left, y, w: tw, h: card_h }, radius: 12.0 * s, color: CARD });
        let mut cy = y + 10.0 * s;
        if let Some((t, c)) = pill {
            self.pill(out, left + 12.0 * s, cy, t, c, CARD);
            cy += 24.0 * s;
        }
        for l in lines {
            out.push(label(left + 12.0 * s, cy, l, Face::Sans, body_px, false, BODY, tw - 24.0 * s));
            cy += line_h;
        }
        y += card_h + 16.0 * s;

        // PR / MR.
        section(out, &mut y, if ctx.provider == Some(context::Provider::GitLab) { "Merge request" } else { "Pull request" });
        match &ctx.pr {
            context::PrInfo::Found(pr) => {
                let color = match pr.state {
                    context::PrState::Open => theme::ACCENT,
                    context::PrState::Approved => SUCCESS,
                    context::PrState::ChangesRequested => WARM,
                    context::PrState::Draft => theme::MUTED,
                    context::PrState::Merged => MERGED,
                    context::PrState::Closed => ERROR,
                };
                let mut lines = self.wrap_px(&pr.title, body_px, tw - 24.0 * s);
                lines.truncate(2);
                let card_h = 20.0 * s + 24.0 * s + lines.len() as f32 * line_h;
                out.push(Chrome::Round { rect: Rect { x: left, y, w: tw, h: card_h }, radius: 12.0 * s, color: CARD });
                self.pill(out, left + 12.0 * s, y + 10.0 * s, &format!("{} {}", pr.number, pr.state.label()), color, CARD);
                let mut cy = y + 34.0 * s;
                for l in lines {
                    out.push(label(left + 12.0 * s, cy, l, Face::Sans, body_px, false, theme::FOREGROUND, tw - 24.0 * s));
                    cy += line_h;
                }
                y += card_h + 16.0 * s;
            }
            other => {
                let msg = match other {
                    context::PrInfo::None => "Sin PR/MR para esta rama".to_string(),
                    context::PrInfo::Error(e) => e.clone(),
                    _ => "Remote sin GitHub/GitLab".to_string(),
                };
                out.push(label(left, y, msg, Face::Sans, 12.0 * s, false, theme::MUTED, tw));
                y += 30.0 * s;
            }
        }

        section(out, &mut y, "Cambios");
        self.changes_list(out, &ctx, left, tw, y, height, 3);
    }

    /// Change summary plus up to `limit` files.
    #[allow(clippy::too_many_arguments)]
    fn changes_list(&mut self, out: &mut Vec<Chrome>, ctx: &context::Context, left: f32, tw: f32, mut y: f32, height: f32, limit: usize) {
        let s = self.scale();
        let Some(c) = &ctx.changes else {
            out.push(label(left, y, "—".into(), Face::Sans, 12.0 * s, false, theme::MUTED, tw));
            return;
        };
        if c.total_files == 0 {
            out.push(label(left, y, "Sin cambios".into(), Face::Sans, 12.0 * s, false, theme::MUTED, tw));
            return;
        }
        let px = 12.5 * s;
        let unit = if c.total_files == 1 { "archivo" } else { "archivos" };
        let mut x = left;
        for (t, color, bold) in [
            (format!("{} {unit}", c.total_files), theme::FOREGROUND, true),
            (format!("+{}", c.added), SUCCESS, true),
            (format!("−{}", c.removed), ERROR, true),
        ] {
            let w = self.fonts.run(&t, Face::Sans, px, bold).width;
            out.push(label(x, y, t, Face::Sans, px, bold, color, w + s));
            x += w + 8.0 * s;
        }
        y += 22.0 * s;
        if let Some(n) = c.unpushed.filter(|n| *n > 0) {
            out.push(label(left, y, format!("{n} commits sin push"), Face::Sans, 12.0 * s, false, WARM, tw));
            y += 22.0 * s;
        }
        let mpx = 11.5 * s;
        for f in c.files.iter().take(limit) {
            if y > height {
                break;
            }
            out.push(label(left, y, format!("+{}", f.added), Face::Mono, mpx, false, SUCCESS, 44.0 * s));
            out.push(label(left + 44.0 * s, y, format!("−{}", f.removed), Face::Mono, mpx, false, ERROR, 40.0 * s));
            // Keep the end of long paths: the file name matters most.
            let path_w = tw - 86.0 * s;
            let mut path = f.path.clone();
            while path.chars().count() > 1 && self.fonts.run(&path, Face::Mono, mpx, false).width > path_w {
                path = format!("…{}", path.chars().skip(2).collect::<String>());
            }
            out.push(label(left + 86.0 * s, y, path, Face::Mono, mpx, false, theme::MUTED, path_w));
            y += 20.0 * s;
        }
    }

    fn toggle_usage(&mut self) {
        self.usage_open = !self.usage_open;
        if self.usage_open {
            self.load_usage(self.usage_range);
        }
        self.window.request_redraw();
    }

    /// Compute the summary off the UI thread; the thread exits when done.
    fn load_usage(&mut self, range: usage::Range) {
        self.usage_range = range;
        self.usage_loading = true;
        let proxy = self.proxy.clone();
        let _ = std::thread::Builder::new().name("usage".into()).spawn(move || {
            let summary = usage::compute(range);
            let _ = proxy.send_event(UserEvent::Usage(range, Box::new(summary)));
        });
        self.window.request_redraw();
    }

    /// Full-window usage dashboard.
    fn usage_chrome(&mut self, out: &mut Vec<Chrome>) {
        let s = self.scale();
        let size = self.window.inner_size();
        let (w, h) = (size.width as f32, size.height as f32);
        out.push(Chrome::Rect { rect: Rect { x: 0.0, y: 0.0, w, h }, color: GUTTER });
        let m = 28.0 * s;
        let cw = w - 2.0 * m;
        let mut y = BAR_H * s;

        // Header: title, period, range switch, esc.
        out.push(label(m, y, "Uso de Claude Code".into(), Face::Sans, 18.0 * s, true, theme::FOREGROUND, cw / 2.0));
        let period = self.usage.as_ref().map_or(String::new(), |u| {
            let from = if u.days <= 1 { "hoy".to_string() } else { short_date(u.first_day) };
            if u.days <= 1 { from } else { format!("{from} – hoy") }
        });
        out.push(label(m, y + 26.0 * s, period, Face::Sans, 12.5 * s, false, theme::MUTED, cw / 2.0));
        let ranges = [(usage::Range::Days(7), "7 días"), (usage::Range::Days(30), "30 días"), (usage::Range::Days(90), "90 días"), (usage::Range::All, "Todo")];
        let px = 12.5 * s;
        let widths: Vec<f32> = ranges.iter().map(|(_, t)| self.fonts.run(t, Face::Sans, px, false).width + 24.0 * s).collect();
        let esc_w = self.fonts.run("esc", Face::Sans, 11.0 * s, false).width + 16.0 * s;
        let seg_w: f32 = widths.iter().sum::<f32>() + 6.0 * s;
        let seg_x = w - m - esc_w - 12.0 * s - seg_w;
        out.push(Chrome::Round { rect: Rect { x: seg_x, y: y + 2.0 * s, w: seg_w, h: 30.0 * s }, radius: 9.0 * s, color: CARD });
        let mut x = seg_x + 3.0 * s;
        for ((range, t), tw) in ranges.iter().zip(&widths) {
            let on = *range == self.usage_range;
            let r = Rect { x, y: y + 5.0 * s, w: *tw, h: 24.0 * s };
            if on {
                out.push(Chrome::Round { rect: r, radius: 7.0 * s, color: SELECTED });
            }
            out.push(label(x + 12.0 * s, r.y + (r.h - px * 1.3) / 2.0, (*t).into(), Face::Sans, px, on, if on { theme::FOREGROUND } else { theme::MUTED }, *tw));
            self.ui_hits.push((r, UiHit::Range(*range)));
            x += tw;
        }
        let esc = Rect { x: w - m - esc_w, y: y + 7.0 * s, w: esc_w, h: 20.0 * s };
        out.push(Chrome::Round { rect: esc, radius: 6.0 * s, color: CARD });
        out.push(label(esc.x + 8.0 * s, esc.y + (esc.h - 11.0 * s * 1.3) / 2.0, "esc".into(), Face::Sans, 11.0 * s, false, theme::MUTED, esc_w));
        self.ui_hits.push((esc, UiHit::CloseUsage));
        y += 60.0 * s;

        let Some(u) = self.usage.clone() else {
            out.push(label(m, y, "Leyendo transcripts…".into(), Face::Sans, 13.0 * s, false, theme::MUTED, cw));
            return;
        };
        if self.usage_loading {
            out.push(label(m + cw / 2.0 - 60.0 * s, BAR_H * s + 30.0 * s, "actualizando…".into(), Face::Sans, 11.5 * s, false, theme::MUTED, 120.0 * s));
        }

        // KPI cards.
        let gap = 14.0 * s;
        let card_w = (cw - 3.0 * gap) / 4.0;
        let card_h = 124.0 * s;
        let cards = [
            ("Tokens de entrada", usage::human(u.input.value), format!("{:.0}% desde caché", u.cache_share * 100.0), &u.input),
            ("Tokens de salida", usage::human(u.output.value), format!("{} por turno", usage::human(u.per_turn)), &u.output),
            ("Tareas", usage::human(u.tasks.value), format!("{} turnos", usage::human(u.turns as f64)), &u.tasks),
            ("Tiempo de agente", usage::duration(u.agent_hours.value), format!("hasta {} a la vez", u.max_concurrent), &u.agent_hours),
        ];
        for (i, (title, value, sub, kpi)) in cards.into_iter().enumerate() {
            let x = m + i as f32 * (card_w + gap);
            let r = Rect { x, y, w: card_w, h: card_h };
            out.push(Chrome::Round { rect: r, radius: 12.0 * s, color: PANEL });
            let pad = 16.0 * s;
            out.push(label(x + pad, y + 14.0 * s, title.to_uppercase(), Face::Sans, 10.5 * s, true, theme::MUTED, card_w - 2.0 * pad));
            if let Some(d) = usage::delta(kpi) {
                let (t, c) = if d >= 0.0 { (format!("+{d:.0}%"), SUCCESS) } else { (format!("{d:.0}%"), ERROR) };
                let pw = self.fonts.run(&t, Face::Sans, 10.5 * s, true).width + 14.0 * s;
                self.pill(out, x + card_w - pad - pw, y + 12.0 * s, &t, c, PANEL);
            }
            out.push(label(x + pad, y + 32.0 * s, value, Face::Sans, 30.0 * s, false, theme::FOREGROUND, card_w - 2.0 * pad));
            out.push(label(x + pad, y + 74.0 * s, sub, Face::Sans, 12.0 * s, false, theme::MUTED, card_w - 2.0 * pad));
            spark(out, &kpi.series, Rect { x: x + pad, y: y + 96.0 * s, w: card_w - 2.0 * pad, h: 18.0 * s }, mix(theme::MUTED, PANEL, 0.75), s);
        }
        y += card_h + 26.0 * s;

        // Tokens per day (60%) and heatmap (40%).
        let row_h = 190.0 * s;
        let left_w = (cw - gap) * 0.58;
        let right_x = m + left_w + 2.0 * gap;
        let right_w = cw - left_w - 2.0 * gap;
        let per_day = u.input.value / u.days.max(1) as f64;
        section_head(self, out, m, y, left_w, "Tokens por día", &format!("{} de {} días con actividad · {}/día", u.active_days, u.days, usage::human(per_day)), s);
        let busiest = (0..7).flat_map(|d| (0..24).map(move |hr| (d, hr))).max_by_key(|&(d, hr)| u.heat[d][hr]).unwrap_or((0, 0));
        let busy = if u.heat[busiest.0][busiest.1] > 0 { format!("más activo {} {} h", WEEKDAYS[busiest.0], busiest.1) } else { String::new() };
        section_head(self, out, right_x, y, right_w, "Cuándo trabajan", &busy, s);
        let top = y + 30.0 * s;
        self.tokens_chart(out, &u, Rect { x: m, y: top, w: left_w, h: row_h - 30.0 * s });
        self.heatmap(out, &u, Rect { x: right_x, y: top, w: right_w, h: row_h - 30.0 * s });
        y += row_h + 24.0 * s;

        // Models and projects.
        let half = (cw - 2.0 * gap) / 2.0;
        let models: Vec<usage::Row> = u.models.iter().map(|r| usage::Row { name: usage::model_name(&r.name), ..r.clone() }).collect();
        section_head(self, out, m, y, half, "Modelos", &format!("{} modelos", u.models_total), s);
        section_head(self, out, m + half + 2.0 * gap, y, half, "Proyectos", &format!("{} proyectos", u.projects_total), s);
        y += 30.0 * s;
        self.usage_table(out, &models, Rect { x: m, y, w: half, h: h - y - 16.0 * s }, "tareas", |v| usage::human(v), theme::ACCENT);
        self.usage_table(out, &u.projects, Rect { x: m + half + 2.0 * gap, y, w: half, h: h - y - 16.0 * s }, "tiempo", |v| usage::duration(v / 3600.0), mix(theme::FOREGROUND, GUTTER, 0.55));
    }

    fn tokens_chart(&mut self, out: &mut Vec<Chrome>, u: &usage::Summary, r: Rect) {
        let s = self.scale();
        let max_in = u.daily_in.iter().copied().max().unwrap_or(0).max(1) as f32;
        let max_out = u.daily_out.iter().copied().max().unwrap_or(0).max(1) as f32;
        let label_w = 64.0 * s;
        let plot = Rect { x: r.x + label_w, y: r.y, w: r.w - label_w, h: r.h - 22.0 * s };
        let mid = plot.y + plot.h * 0.66;
        let (up, down) = (plot.h * 0.64, plot.h * 0.32);
        out.push(label(r.x, plot.y, usage::human(max_in as f64), Face::Sans, 13.0 * s, true, theme::FOREGROUND, label_w));
        out.push(label(r.x, plot.y + 18.0 * s, "entrada".into(), Face::Sans, 11.5 * s, false, theme::MUTED, label_w));
        out.push(label(r.x, mid + 6.0 * s, usage::human(max_out as f64), Face::Sans, 13.0 * s, true, theme::FOREGROUND, label_w));
        out.push(label(r.x, mid + 24.0 * s, "salida".into(), Face::Sans, 11.5 * s, false, theme::ACCENT, label_w));
        let n = u.days.max(1) as f32;
        let slot = plot.w / n;
        let bar = (slot * 0.68).max(1.0);
        for (i, (a, b)) in u.daily_in.iter().zip(&u.daily_out).enumerate() {
            let x = plot.x + i as f32 * slot + (slot - bar) / 2.0;
            let hi = (*a as f32 / max_in * up).max(if *a > 0 { s } else { 0.0 });
            let ho = (*b as f32 / max_out * down).max(if *b > 0 { s } else { 0.0 });
            out.push(Chrome::Rect { rect: Rect { x, y: mid - hi, w: bar, h: hi }, color: [0x3D, 0x50, 0x59] });
            out.push(Chrome::Rect { rect: Rect { x, y: mid + s, w: bar, h: ho }, color: theme::ACCENT });
        }
        out.push(Chrome::Rect { rect: Rect { x: plot.x, y: mid, w: plot.w, h: s }, color: HAIRLINE });
        let ly = plot.y + plot.h + 6.0 * s;
        let px = 11.0 * s;
        out.push(label(plot.x, ly, short_date(u.first_day), Face::Sans, px, false, theme::MUTED, 80.0 * s));
        if u.days > 2 {
            let mid_day = u.first_day + (u.days / 2) as i64;
            let t = short_date(mid_day);
            let tw = self.fonts.run(&t, Face::Sans, px, false).width;
            out.push(label(plot.x + (u.days / 2) as f32 * slot + slot / 2.0 - tw / 2.0, ly, t, Face::Sans, px, false, theme::MUTED, tw + s));
        }
        let tw = self.fonts.run("hoy", Face::Sans, px, false).width;
        out.push(label(plot.x + plot.w - tw, ly, "hoy".into(), Face::Sans, px, false, theme::MUTED, tw + s));
    }

    fn heatmap(&mut self, out: &mut Vec<Chrome>, u: &usage::Summary, r: Rect) {
        let s = self.scale();
        let label_w = 18.0 * s;
        let cell = ((r.w - label_w) / 24.0).min((r.h - 40.0 * s) / 7.0).floor();
        let g = (cell * 0.12).max(s).round();
        let max = u.heat.iter().flatten().copied().max().unwrap_or(0).max(1) as f32;
        let px = 10.5 * s;
        for (d, row) in u.heat.iter().enumerate() {
            let y = r.y + d as f32 * cell;
            out.push(label(r.x, y + (cell - px * 1.3) / 2.0, WEEKDAYS[d][..1].to_uppercase(), Face::Sans, px, false, theme::MUTED, label_w));
            for (hr, v) in row.iter().enumerate() {
                let color = if *v == 0 { CARD } else { mix(theme::FOREGROUND, CARD, 0.15 + 0.85 * (*v as f32 / max).powf(0.6)) };
                let c = Rect { x: r.x + label_w + hr as f32 * cell, y, w: cell - g, h: cell - g };
                out.push(Chrome::Round { rect: c, radius: 2.0 * s, color });
            }
        }
        let ly = r.y + 7.0 * cell + 4.0 * s;
        for hr in [0, 6, 12, 18] {
            out.push(label(r.x + label_w + hr as f32 * cell, ly, hr.to_string(), Face::Sans, px, false, theme::MUTED, 30.0 * s));
        }
        // Legend: less → more.
        let lx = r.x + label_w + 24.0 * cell - 4.0 * (cell * 0.6 + 2.0 * s) - 70.0 * s;
        out.push(label(lx, ly + 16.0 * s, "menos".into(), Face::Sans, px, false, theme::MUTED, 40.0 * s));
        for i in 0..4 {
            let color = mix(theme::FOREGROUND, CARD, 0.15 + 0.85 * (i as f32 / 3.0).powf(0.6));
            let x = lx + 38.0 * s + i as f32 * (cell * 0.6 + 2.0 * s);
            out.push(Chrome::Round { rect: Rect { x, y: ly + 19.0 * s, w: cell * 0.6, h: cell * 0.6 }, radius: 2.0 * s, color });
        }
        out.push(label(lx + 42.0 * s + 4.0 * (cell * 0.6 + 2.0 * s), ly + 16.0 * s, "más".into(), Face::Sans, px, false, theme::MUTED, 30.0 * s));
    }

    #[allow(clippy::too_many_arguments)]
    fn usage_table(&mut self, out: &mut Vec<Chrome>, rows: &[usage::Row], r: Rect, third: &str, fmt: impl Fn(f64) -> String, bar_color: Rgb) {
        let s = self.scale();
        let px = 12.5 * s;
        let col = 72.0 * s;
        let (c3, c2, c1) = (r.x + r.w, r.x + r.w - col, r.x + r.w - 2.0 * col);
        let name_w = r.w * 0.36;
        let bar_x = r.x + name_w + 8.0 * s;
        let bar_max = (c1 - col - bar_x).max(10.0 * s);
        let right = |fonts: &mut Fonts, out: &mut Vec<Chrome>, x_end: f32, y: f32, t: String, color: Rgb, px: f32| {
            let tw = fonts.run(&t, Face::Sans, px, false).width;
            out.push(label(x_end - tw, y, t, Face::Sans, px, false, color, tw + 1.0));
        };
        let hy = r.y;
        for (x, t) in [(c1, "entrada"), (c2, "salida"), (c3, third)] {
            right(&mut self.fonts, out, x, hy, t.into(), theme::MUTED, 11.0 * s);
        }
        let mut y = hy + 22.0 * s;
        for row in rows {
            if y + 22.0 * s > r.y + r.h {
                break;
            }
            let rest = row.name.ends_with(" más");
            out.push(label(r.x, y, row.name.clone(), Face::Sans, px, false, if rest { theme::MUTED } else { theme::FOREGROUND }, name_w));
            let bw = (row.share as f32 * bar_max).max(3.0 * s);
            out.push(Chrome::Round { rect: Rect { x: bar_x, y: y + 6.0 * s, w: bw, h: 6.0 * s }, radius: 3.0 * s, color: if rest { HAIRLINE } else { bar_color } });
            right(&mut self.fonts, out, c1, y, usage::human(row.input as f64), theme::FOREGROUND, px);
            right(&mut self.fonts, out, c2, y, usage::human(row.output as f64), theme::FOREGROUND, px);
            right(&mut self.fonts, out, c3, y, fmt(row.third), theme::MUTED, px);
            y += 25.0 * s;
        }
    }

    /// Activate the tab holding `id` and focus it.
    fn jump_to(&mut self, id: PaneId) {
        if let Some(i) = self.tabs.list.iter().position(|t| t.root.contains(id)) {
            self.commit_rename();
            self.tabs.select(i);
            self.tabs.active_mut().focused = id;
            self.after_tab_change();
        }
    }

    fn update_title(&self) {
        if !self.tabs.list.is_empty() {
            self.window.set_title(&self.tab_title(self.tabs.active));
        }
    }

    /// Everything around the terminals: sidebar, panel, pill tabs, pane cards.
    /// Also records hit-boxes for clicks.
    fn chrome(&mut self) -> Vec<Chrome> {
        let s = self.scale();
        let width = self.window.inner_size().width as f32;
        let mut out = vec![];
        self.ui_hits.clear();
        self.sidebar_chrome(&mut out);
        self.panel_chrome(&mut out);
        self.panes_chrome(&mut out);

        self.tab_hits.clear();
        let n = self.tabs.list.len() as f32;
        let room = width - self.panel_w() - self.tabs_x() - PLUS_W * s - 2.0 * PAD * s;
        let tab_w = (room / n).min(TAB_MAX_W * s).floor();
        self.tab_w = tab_w;
        let (top, h) = (6.0 * s, BAR_H * s - 10.0 * s);
        let px = 12.5 * s;
        let text_y = top + (h - px * 1.3) / 2.0;
        let mut dragged_chrome = vec![];
        self.animating = false;
        let first_x = self.tabs_x();
        let last_x = first_x + (n - 1.0).max(0.0) * tab_w;
        for i in 0..self.tabs.list.len() {
            let slot_x = first_x + i as f32 * tab_w;
            let tab_id = self.tabs.list[i].id;
            let drag = self.dragging_tab.as_ref().filter(|d| d.moving && d.index == i);
            let x = match drag {
                Some(d) => (self.mouse.x as f32 - d.grab).clamp(first_x, last_x),
                None => {
                    // Ease towards the slot; Warp-like slide when tabs swap places.
                    let cur = self.tab_x.get(&tab_id).copied().unwrap_or(slot_x);
                    let next = cur + (slot_x - cur) * 0.35;
                    if (slot_x - next).abs() < 0.5 {
                        slot_x
                    } else {
                        self.animating = true;
                        next
                    }
                }
            };
            self.tab_x.insert(tab_id, x);
            let first = out.len();
            let rect = Rect { x, y: top, w: tab_w - 4.0 * s, h };
            let slot = Rect { x: slot_x, ..rect };
            let active = i == self.tabs.active;
            let hovered = self.hovered_tab == Some(i);
            if active || drag.is_some() {
                out.push(Chrome::Round { rect, radius: 8.0 * s, color: SELECTED });
            } else if hovered {
                out.push(Chrome::Round { rect, radius: 8.0 * s, color: CARD });
            }
            let show_close = active || hovered;
            let close_w = 22.0 * s;
            let mut text_x = x + TAB_PAD * s;
            let mut max_w = rect.w - 2.0 * TAB_PAD * s - if show_close { close_w } else { 0.0 };
            let (text, editing) = match &self.renaming {
                Some((j, buf)) if *j == i => (buf.clone(), true),
                _ => (self.tab_title(i), false),
            };
            if !editing && self.tab_waiting(i) {
                out.push(Chrome::Round { rect: Rect { x: text_x, y: top + h / 2.0 - 3.5 * s, w: 7.0 * s, h: 7.0 * s }, radius: 3.5 * s, color: WARM });
                text_x += 13.0 * s;
                max_w -= 13.0 * s;
            }
            if editing {
                // Caret after the text (clamped to the tab), in the accent colour.
                let tw = self.fonts.run(&text, Face::Sans, px, false).width.min(max_w);
                out.push(Chrome::Rect { rect: Rect { x: text_x + tw + s, y: text_y, w: 2.0 * s, h: px * 1.3 }, color: theme::ACCENT });
            }
            out.push(label(text_x, text_y, text, Face::Sans, px, active, if active { theme::FOREGROUND } else { theme::MUTED }, max_w));
            if show_close {
                let cx = x + rect.w - close_w - 4.0 * s;
                out.push(label(cx + 6.0 * s, top + (h - 14.0 * s * 1.3) / 2.0, "×".into(), Face::Sans, 14.0 * s, false, theme::MUTED, close_w));
                // Close target goes first so it wins over the tab body.
                let slot_cx = slot_x + rect.w - close_w - 4.0 * s;
                self.tab_hits.push((Rect { x: slot_cx, y: top, w: close_w, h }, TabHit::Close(i)));
            }
            self.tab_hits.push((slot, TabHit::Tab(i)));
            if drag.is_some() {
                // The dragged tab is drawn last, on top of the others.
                dragged_chrome = out.split_off(first);
            }
        }
        out.extend(dragged_chrome);
        let ids: Vec<u64> = self.tabs.list.iter().map(|t| t.id).collect();
        self.tab_x.retain(|id, _| ids.contains(id));
        let plus_x = first_x + n * tab_w;
        out.push(label(plus_x + 8.0 * s, top + (h - 16.0 * s * 1.3) / 2.0, "+".into(), Face::Sans, 16.0 * s, false, theme::MUTED, PLUS_W * s));
        self.tab_hits.push((Rect { x: plus_x, y: top, w: PLUS_W * s, h }, TabHit::New));
        out
    }

    /// Rounded cards for the active tab's panes, with a header and a focus ring when split.
    fn panes_chrome(&mut self, out: &mut Vec<Chrome>) {
        let s = self.scale();
        let (rects, _) = self.tab_layout(self.tabs.active);
        let split = rects.len() > 1;
        let focused = self.focused_id();
        for (id, r) in rects {
            if split && id == focused {
                let g = 1.5 * s;
                out.push(Chrome::Round { rect: Rect { x: r.x - g, y: r.y - g, w: r.w + 2.0 * g, h: r.h + 2.0 * g }, radius: 12.0 * s + g, color: theme::ACCENT });
            }
            out.push(Chrome::Round { rect: r, radius: 12.0 * s, color: theme::BACKGROUND });
            let px = 12.0 * s;
            let hy = r.y + (HEADER_H * s - px * 1.3) / 2.0;
            let close_w = 22.0 * s;
            let mut x = r.x + 12.0 * s;
            let max = r.x + r.w - close_w - 8.0 * s;
            let title = self.pane_label(id);
            let tw = self.fonts.run(&title, Face::Sans, px, true).width.min(max - x);
            out.push(label(x, hy, title, Face::Sans, px, true, theme::FOREGROUND, max - x));
            x += tw + 10.0 * s;
            if let Some(branch) = self.panes.get(&id).and_then(|p| p.ctx.as_ref()).and_then(|c| c.branch.clone()) {
                if x + 40.0 * s < max {
                    out.push(label(x, hy + s, "\u{e0a0}".into(), Face::Mono, 11.0 * s, false, theme::MUTED, 14.0 * s));
                    out.push(label(x + 14.0 * s, hy + s, branch, Face::Sans, 11.5 * s, false, theme::MUTED, max - x - 14.0 * s));
                }
            }
            let cx = r.x + r.w - close_w - 6.0 * s;
            out.push(label(cx + 6.0 * s, r.y + (HEADER_H * s - 14.0 * s * 1.3) / 2.0, "×".into(), Face::Sans, 14.0 * s, false, theme::MUTED, close_w));
            self.ui_hits.push((Rect { x: cx, y: r.y, w: close_w, h: HEADER_H * s }, UiHit::ClosePane(id)));
        }
    }

    fn pane_at(&self, x: f32, y: f32) -> Option<PaneId> {
        self.tab_layout(self.tabs.active).0.into_iter().find(|(_, r)| r.contains(x, y)).map(|(id, _)| id)
    }

    /// Grid point under the mouse inside `id` (line adjusted for scrollback).
    fn point_in(&self, id: PaneId) -> (Point, Side, usize, usize) {
        let pane = &self.panes[&id];
        let (ox, oy) = self.grid_origin(pane.rect);
        let ws = self.grid_size(pane.rect);
        let x = (self.mouse.x as f32 - ox) / self.fonts.cell_w;
        let y = (self.mouse.y as f32 - oy) / self.fonts.cell_h;
        let col = (x.max(0.0) as usize).min(ws.num_cols as usize - 1);
        let line = (y.max(0.0) as usize).min(ws.num_lines as usize - 1);
        let side = if x.fract() < 0.5 { Side::Left } else { Side::Right };
        let offset = pane.session.term.lock().grid().display_offset() as i32;
        (Point::new(Line(line as i32 - offset), Column(col)), side, col, line)
    }

    fn reset_blink(&mut self) {
        self.cursor_on = true;
        self.next_blink = Instant::now() + BLINK;
    }

    fn mouse_report(&self, id: PaneId, button: u8, pressed: bool, motion: bool) {
        let (_, _, col, line) = self.point_in(id);
        let session = &self.panes[&id].session;
        let mode = *session.term.lock().mode();
        let mut code = button + if motion { 32 } else { 0 };
        if self.mods.shift_key() {
            code += 4;
        }
        if self.mods.alt_key() {
            code += 8;
        }
        if self.mods.control_key() {
            code += 16;
        }
        let bytes = if mode.contains(TermMode::SGR_MOUSE) {
            format!("\x1b[<{code};{};{}{}", col + 1, line + 1, if pressed { 'M' } else { 'm' }).into_bytes()
        } else {
            let code = if pressed { code } else { 3 + (code & !3) };
            vec![0x1b, b'[', b'M', 32 + code, 32 + (col as u8).saturating_add(1), 32 + (line as u8).saturating_add(1)]
        };
        session.write(bytes);
    }

    fn mouse_mode(&self, id: PaneId) -> bool {
        !self.mods.shift_key() && self.panes[&id].session.term.lock().mode().intersects(TermMode::MOUSE_MODE)
    }

    fn copy(&mut self) {
        let text = self.focused().session.term.lock().selection_to_string();
        if let (Some(text), Some(cb)) = (text, self.clipboard.as_mut()) {
            let _ = cb.set_text(text);
        }
    }

    fn paste(&mut self) {
        let Some(text) = self.clipboard.as_mut().and_then(|cb| cb.get_text().ok()) else { return };
        let session = &self.panes[&self.focused_id()].session;
        let bracketed = session.term.lock().mode().contains(TermMode::BRACKETED_PASTE);
        let bytes = if bracketed {
            // Strip any end marker inside the text so it cannot break out of the paste.
            format!("\x1b[200~{}\x1b[201~", text.replace("\x1b[201~", ""))
        } else {
            text.replace("\r\n", "\r").replace('\n', "\r")
        };
        session.write(bytes.into_bytes());
        session.term.lock().scroll_display(Scroll::Bottom);
    }

    fn scroll(&mut self, id: PaneId, lines: i32) {
        if lines == 0 {
            return;
        }
        let session = &self.panes[&id].session;
        let mode = *session.term.lock().mode();
        if mode.intersects(TermMode::MOUSE_MODE) && !self.mods.shift_key() {
            let button = if lines > 0 { 64 } else { 65 };
            for _ in 0..lines.unsigned_abs().min(10) {
                self.mouse_report(id, button, true, false);
            }
        } else if mode.contains(TermMode::ALT_SCREEN | TermMode::ALTERNATE_SCROLL) {
            let key = if lines > 0 { "A" } else { "B" };
            let prefix = if mode.contains(TermMode::APP_CURSOR) { "\x1bO" } else { "\x1b[" };
            session.write(format!("{prefix}{key}").repeat(lines.unsigned_abs() as usize).into_bytes());
        } else {
            session.term.lock().scroll_display(Scroll::Delta(lines));
            self.window.request_redraw();
        }
    }

    fn set_font_size(&mut self, points: f32) {
        self.font_points = points.clamp(6.0, 72.0);
        self.fonts.set_size(self.font_points, self.scale());
        self.renderer.clear_glyphs();
        for p in self.panes.values_mut() {
            p.rect = Rect::default(); // force a PTY resize: the cell size changed
        }
        self.relayout();
    }

    fn focus(&mut self, id: PaneId) {
        if self.focused_id() != id {
            self.tabs.active_mut().focused = id;
            self.reset_blink();
            self.update_title();
            self.window.request_redraw();
        }
    }

    /// Cmd shortcuts. Returns true when the key was consumed.
    fn shortcut(&mut self, key: &Key, plain: &Key, physical: PhysicalKey, event_loop: &ActiveEventLoop) -> bool {
        let m = self.mods;
        if m.control_key() && !m.super_key() && *key == Key::Named(NamedKey::Tab) {
            self.tabs.cycle(!m.shift_key());
            self.after_tab_change();
            return true;
        }
        if !m.super_key() {
            return false;
        }
        if m.shift_key() && !m.alt_key() {
            let a = self.tabs.active;
            let to = match key {
                Key::Named(NamedKey::ArrowLeft) => a.checked_sub(1),
                Key::Named(NamedKey::ArrowRight) => Some(a + 1),
                _ => None,
            };
            if let Some(to) = to {
                self.tabs.move_tab(a, to);
                self.window.request_redraw();
                return true;
            }
        }
        if m.alt_key() {
            let arrow = match key {
                Key::Named(NamedKey::ArrowLeft) => Arrow::Left,
                Key::Named(NamedKey::ArrowRight) => Arrow::Right,
                Key::Named(NamedKey::ArrowUp) => Arrow::Up,
                Key::Named(NamedKey::ArrowDown) => Arrow::Down,
                _ => return false,
            };
            let (rects, _) = self.tab_layout(self.tabs.active);
            if let Some(id) = layout::neighbour(&rects, self.focused_id(), arrow) {
                self.focus(id);
            }
            return true;
        }
        match physical {
            PhysicalKey::Code(KeyCode::BracketLeft) if m.shift_key() => {
                self.tabs.cycle(false);
                self.after_tab_change();
                return true;
            }
            PhysicalKey::Code(KeyCode::BracketRight) if m.shift_key() => {
                self.tabs.cycle(true);
                self.after_tab_change();
                return true;
            }
            _ => {}
        }
        let Key::Character(c) = plain else { return false };
        match c.to_lowercase().as_str() {
            "c" => self.copy(),
            "v" => self.paste(),
            "q" => event_loop.exit(),
            "t" => self.new_tab(),
            "w" => {
                if self.close_pane(self.focused_id()) {
                    event_loop.exit();
                }
            }
            "d" => self.split(if m.shift_key() { Dir::Down } else { Dir::Right }),
            "r" if m.shift_key() => self.start_rename(self.tabs.active),
            "b" => {
                self.sidebar = !self.sidebar;
                self.relayout();
            }
            "i" => {
                self.panel = !self.panel;
                self.relayout();
            }
            "u" => self.toggle_usage(),
            "=" | "+" => self.set_font_size(self.font_points + 1.0),
            "-" => self.set_font_size(self.font_points - 1.0),
            "0" => self.set_font_size(self.config.font.size),
            d @ ("1" | "2" | "3" | "4" | "5" | "6" | "7" | "8") => {
                self.tabs.select(d.parse::<usize>().unwrap() - 1);
                self.after_tab_change();
            }
            "9" => {
                self.tabs.select(self.tabs.list.len() - 1);
                self.after_tab_change();
            }
            _ => return false,
        }
        true
    }

    fn start_rename(&mut self, i: usize) {
        self.commit_rename();
        self.tabs.select(i);
        let current = self.tabs.list[i].name.clone().unwrap_or_else(|| self.tab_title(i));
        self.renaming = Some((i, current));
        self.window.request_redraw();
    }

    /// Save the edited name; an empty name goes back to the automatic title.
    fn commit_rename(&mut self) {
        if let Some((i, name)) = self.renaming.take() {
            if let Some(tab) = self.tabs.list.get_mut(i) {
                let name = name.trim();
                tab.name = (!name.is_empty()).then(|| name.to_string());
            }
            self.update_title();
            self.window.request_redraw();
        }
    }

    fn after_tab_change(&mut self) {
        self.reset_blink();
        self.update_title();
        self.window.request_redraw();
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("cardumen")
            .with_inner_size(LogicalSize::new(1320.0, 820.0))
            .with_titlebar_transparent(true)
            .with_fullsize_content_view(true)
            .with_title_hidden(true);
        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));
        let fonts = Fonts::new(&self.config.font.family, self.config.font.size, window.scale_factor() as f32);
        let renderer = Renderer::new(window.clone());
        let mut st = State {
            window,
            renderer,
            fonts,
            config: self.config.clone(),
            proxy: self.proxy.clone(),
            clipboard: arboard::Clipboard::new().ok(),
            font_points: self.config.font.size,
            panes: HashMap::new(),
            tabs: Tabs::default(),
            next_id: 0,
            tab_hits: vec![],
            mods: ModifiersState::empty(),
            window_focused: true,
            cursor_on: true,
            next_blink: Instant::now() + BLINK,
            mouse: PhysicalPosition::new(0.0, 0.0),
            selecting: None,
            last_click: None,
            scroll_px: 0.0,
            pressed_button: None,
            renaming: None,
            last_tab_click: None,
            dragging_tab: None,
            tab_x: HashMap::new(),
            animating: false,
            hovered_tab: None,
            tab_w: 0.0,
            socket: self.socket.clone(),
            sidebar: true,
            sidebar_hits: vec![],
            next_tick: Instant::now(),
            panel: true,
            panel_changes: false,
            usage_open: false,
            usage_range: usage::Range::Days(30),
            usage: None,
            usage_loading: false,
            footer_hover: false,
            ui_hits: vec![],
            ctx_tx: {
                let proxy = self.proxy.clone();
                context::spawn(move |u| proxy.send_event(UserEvent::Context(u)).is_ok(), session::cwd_of)
            },
        };
        st.new_tab();
        if st.tabs.list.is_empty() {
            event_loop.exit();
            return;
        }
        self.state = Some(st);
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        let Some(st) = self.state.as_mut() else { return };
        let (id, event) = match event {
            UserEvent::Term(id, event) => (id, event),
            UserEvent::Usage(range, summary) => {
                if range == st.usage_range {
                    st.usage = Some(summary);
                    st.usage_loading = false;
                    st.window.request_redraw();
                }
                return;
            }
            UserEvent::Context(update) => {
                match update {
                    context::Update::Context(id, ctx) => {
                        if let Some(p) = st.panes.get_mut(&id) {
                            p.ctx = Some(ctx);
                        }
                    }
                    context::Update::LastMessage(id, text) => {
                        if let Some(p) = st.panes.get_mut(&id) {
                            p.last_message = Some(text);
                        }
                    }
                }
                st.window.request_redraw();
                return;
            }
            UserEvent::Status(id, claude, hook) => {
                let Some(pane) = st.panes.get_mut(&id) else { return };
                match claude {
                    Some(status::Claude::Waiting) => pane.ask = hook.ask.clone(),
                    _ => pane.ask = None,
                }
                if matches!(claude, Some(status::Claude::Done | status::Claude::Waiting)) {
                    if let Some(path) = hook.transcript.clone() {
                        let _ = st.ctx_tx.send(context::Request::Transcript(id, path));
                    }
                    let _ = st.ctx_tx.send(context::Request::Refresh(id));
                }
                let prev = pane.claude.map(|c| c.claude);
                if prev != claude {
                    pane.claude = claude.map(|claude| status::State { claude, since: Instant::now() });
                    if claude == Some(status::Claude::Waiting) && !st.window_focused {
                        st.window.request_user_attention(Some(winit::window::UserAttentionType::Informational));
                    }
                }
                st.window.request_redraw();
                return;
            }
        };
        if let Event::Exit | Event::ChildExit(_) = event {
            if st.close_pane(id) {
                event_loop.exit();
            }
            return;
        }
        let Some(pane) = st.panes.get_mut(&id) else { return };
        match event {
            Event::Wakeup | Event::CursorBlinkingChange | Event::MouseCursorDirty => st.window.request_redraw(),
            Event::Title(t) => {
                pane.title = Some(t);
                st.update_title();
                st.window.request_redraw();
            }
            Event::ResetTitle => {
                pane.title = None;
                st.update_title();
                st.window.request_redraw();
            }
            Event::ClipboardStore(_, text) => {
                if let Some(cb) = st.clipboard.as_mut() {
                    let _ = cb.set_text(text);
                }
            }
            Event::ClipboardLoad(_, format) => {
                let text = st.clipboard.as_mut().and_then(|cb| cb.get_text().ok()).unwrap_or_default();
                st.panes[&id].session.write(format(&text).into_bytes());
            }
            Event::ColorRequest(index, format) => {
                let rgb = match index {
                    256 => theme::FOREGROUND,
                    257 => theme::BACKGROUND,
                    258 => theme::ACCENT,
                    i if i < 256 => theme::indexed(i as u8),
                    _ => theme::FOREGROUND,
                };
                let reply = format(alacritty_terminal::vte::ansi::Rgb { r: rgb[0], g: rgb[1], b: rgb[2] });
                pane.session.write(reply.into_bytes());
            }
            Event::PtyWrite(text) => pane.session.write(text.into_bytes()),
            Event::TextAreaSizeRequest(format) => {
                let size = st.grid_size(st.panes[&id].rect);
                st.panes[&id].session.write(format(size).into_bytes());
            }
            Event::Bell => st.window.request_user_attention(None),
            Event::Exit | Event::ChildExit(_) => unreachable!("handled above"),
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(st) = self.state.as_mut() else { return };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                st.renderer.resize(size.width, size.height);
                st.relayout();
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                let points = st.font_points;
                st.set_font_size(points);
            }
            WindowEvent::ModifiersChanged(m) => st.mods = m.state(),
            WindowEvent::Focused(focused) => {
                st.window_focused = focused;
                let session = &st.focused().session;
                if session.term.lock().mode().contains(TermMode::FOCUS_IN_OUT) {
                    session.write(if focused { &b"\x1b[I"[..] } else { &b"\x1b[O"[..] });
                }
                st.reset_blink();
                st.window.request_redraw();
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                if st.usage_open {
                    if event.logical_key == Key::Named(NamedKey::Escape) {
                        st.toggle_usage();
                    } else if st.mods.super_key() {
                        let plain = event.key_without_modifiers();
                        if let Key::Character(c) = &plain {
                            match c.as_str() {
                                "u" => st.toggle_usage(),
                                "q" => event_loop.exit(),
                                _ => {}
                            }
                        }
                    }
                    return;
                }
                if let Some((_, buf)) = st.renaming.as_mut() {
                    match input::edit_line(buf, &event.logical_key, event.text.as_deref(), st.mods) {
                        input::Edit::Continue => st.window.request_redraw(),
                        input::Edit::Commit => st.commit_rename(),
                        input::Edit::Cancel => {
                            st.renaming = None;
                            st.window.request_redraw();
                        }
                    }
                    return;
                }
                let plain = event.key_without_modifiers();
                if st.shortcut(&event.logical_key, &plain, event.physical_key, event_loop) {
                    return;
                }
                let session = &st.panes[&st.focused_id()].session;
                let app_cursor = session.term.lock().mode().contains(TermMode::APP_CURSOR);
                if let Some(bytes) = input::encode(&event.logical_key, &plain, event.text.as_deref(), st.mods, app_cursor) {
                    session.write(bytes);
                    let mut term = session.term.lock();
                    term.scroll_display(Scroll::Bottom);
                    term.selection = None;
                    drop(term);
                    st.reset_blink();
                    st.window.request_redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                st.mouse = position;
                let (x, y, s) = (position.x as f32, position.y as f32, st.scale());
                let hovered = st.tab_hits.iter().find_map(|(r, h)| match h {
                    TabHit::Tab(i) | TabHit::Close(i) if r.contains(x, y) => Some(*i),
                    _ => None,
                });
                if hovered != st.hovered_tab {
                    st.hovered_tab = hovered;
                    st.window.request_redraw();
                }
                // Hover state of the sidebar's "Uso" button.
                let in_footer = x < st.sidebar_w() && y > st.window.inner_size().height as f32 - 52.0 * s;
                if in_footer != st.footer_hover {
                    st.footer_hover = in_footer;
                    st.window.request_redraw();
                }
                let tabs_x = st.tabs_x();
                if let Some(drag) = st.dragging_tab.as_mut() {
                    if !drag.moving && (x - drag.press_x).abs() > 4.0 * s {
                        drag.moving = true;
                        st.renaming = None;
                    }
                    if drag.moving && st.tab_w > 0.0 {
                        // Swap once the dragged tab's centre crosses into a neighbour's slot.
                        let n = st.tabs.list.len();
                        let centre = x - drag.grab + st.tab_w / 2.0 - tabs_x;
                        let to = ((centre / st.tab_w).floor().max(0.0) as usize).min(n - 1);
                        let from = drag.index;
                        drag.index = to;
                        if to != from {
                            st.tabs.move_tab(from, to);
                        }
                        st.window.request_redraw();
                    }
                    return;
                }
                if let Some(id) = st.selecting {
                    let (point, side, _, _) = st.point_in(id);
                    if let Some(sel) = st.panes[&id].session.term.lock().selection.as_mut() {
                        sel.update(point, side);
                    }
                    st.window.request_redraw();
                } else if let Some((id, b)) = st.pressed_button {
                    if st.panes.get(&id).is_some_and(|p| {
                        p.session.term.lock().mode().intersects(TermMode::MOUSE_DRAG | TermMode::MOUSE_MOTION)
                    }) {
                        st.mouse_report(id, b, true, true);
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let pressed = state == ElementState::Pressed;
                let (x, y) = (st.mouse.x as f32, st.mouse.y as f32);
                if pressed && button == MouseButton::Left {
                    let hit = st.ui_hits.iter().find(|(r, _)| r.contains(x, y)).map(|(_, h)| *h);
                    match hit {
                        Some(UiHit::Range(range)) => {
                            st.load_usage(range);
                            return;
                        }
                        Some(UiHit::CloseUsage | UiHit::OpenUsage) => {
                            st.toggle_usage();
                            return;
                        }
                        Some(UiHit::PanelTab(c)) => {
                            st.panel_changes = c;
                            st.window.request_redraw();
                            return;
                        }
                        Some(UiHit::ClosePane(id)) => {
                            if st.close_pane(id) {
                                event_loop.exit();
                            }
                            return;
                        }
                        None => {}
                    }
                }
                if st.usage_open {
                    return;
                }
                if pressed && button == MouseButton::Left && x < st.sidebar_w() {
                    if let Some(id) = st.sidebar_hits.iter().find(|(r, _)| r.contains(x, y)).map(|(_, id)| *id) {
                        st.jump_to(id);
                    } else if y < BAR_H * st.scale() {
                        let _ = st.window.drag_window();
                    }
                    return;
                }
                if pressed && button == MouseButton::Left && y < BAR_H * st.scale() {
                    match st.tab_hits.iter().find(|(r, _)| r.contains(x, y)).map(|(_, h)| h) {
                        Some(TabHit::Tab(i)) => {
                            let i = *i;
                            let now = Instant::now();
                            let double = st.last_tab_click.is_some_and(|(t, j)| j == i && now - t < MULTI_CLICK);
                            st.last_tab_click = Some((now, i));
                            let slot_x = st.tabs_x() + i as f32 * st.tab_w;
                            st.dragging_tab = Some(TabDrag { index: i, press_x: x, grab: x - slot_x, moving: false });
                            if double {
                                st.start_rename(i);
                            } else if st.renaming.as_ref().is_none_or(|(j, _)| *j != i) {
                                st.commit_rename();
                                st.tabs.select(i);
                                st.after_tab_change();
                            }
                        }
                        Some(TabHit::Close(i)) => {
                            let i = *i;
                            st.commit_rename();
                            if st.close_tab(i) {
                                event_loop.exit();
                            }
                        }
                        Some(TabHit::New) => st.new_tab(),
                        None => {
                            let _ = st.window.drag_window();
                        }
                    }
                    return;
                }
                if pressed {
                    st.commit_rename();
                }
                if !pressed {
                    if st.dragging_tab.take().is_some_and(|d| d.moving) {
                        st.window.request_redraw();
                    }
                    // Copy on select: whatever was just selected goes to the clipboard.
                    if let Some(id) = st.selecting.take() {
                        let text = st.panes.get(&id).and_then(|p| p.session.term.lock().selection_to_string());
                        if let (Some(text), Some(cb)) = (text.filter(|t| !t.is_empty()), st.clipboard.as_mut()) {
                            let _ = cb.set_text(text);
                        }
                    }
                    if let Some((id, code)) = st.pressed_button.take() {
                        if st.panes.contains_key(&id) {
                            st.mouse_report(id, code, false, false);
                        }
                    }
                    return;
                }
                let Some(id) = st.pane_at(x, y) else { return };
                st.focus(id);
                let code = match button {
                    MouseButton::Left => 0,
                    MouseButton::Middle => 1,
                    MouseButton::Right => 2,
                    _ => return,
                };
                if st.mouse_mode(id) {
                    st.pressed_button = Some((id, code));
                    st.mouse_report(id, code, true, false);
                    return;
                }
                if button != MouseButton::Left {
                    return;
                }
                let (point, side, _, _) = st.point_in(id);
                let now = Instant::now();
                let clicks = match st.last_click {
                    Some((t, p, n)) if now - t < MULTI_CLICK && p == point => n % 3 + 1,
                    _ => 1,
                };
                st.last_click = Some((now, point, clicks));
                let ty = match clicks {
                    2 => SelectionType::Semantic,
                    3 => SelectionType::Lines,
                    _ => SelectionType::Simple,
                };
                st.panes[&id].session.term.lock().selection = Some(Selection::new(ty, point, side));
                st.selecting = Some(id);
                st.window.request_redraw();
            }
            WindowEvent::CursorLeft { .. } => {
                if st.hovered_tab.take().is_some() {
                    st.window.request_redraw();
                }
            }
            WindowEvent::MouseWheel { .. } if st.usage_open => {}
            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => (y * 3.0) as i32,
                    MouseScrollDelta::PixelDelta(p) => {
                        st.scroll_px += p.y;
                        let ch = st.fonts.cell_h as f64;
                        let n = (st.scroll_px / ch).trunc();
                        st.scroll_px -= n * ch;
                        n as i32
                    }
                };
                let id = st.pane_at(st.mouse.x as f32, st.mouse.y as f32).unwrap_or(st.focused_id());
                st.scroll(id, lines);
            }
            WindowEvent::RedrawRequested => {
                if st.usage_open {
                    st.ui_hits.clear();
                    let mut chrome = vec![];
                    st.usage_chrome(&mut chrome);
                    st.renderer.render(&[], &chrome, &mut st.fonts, GUTTER);
                    return;
                }
                let chrome = st.chrome();
                let focused = st.focused_id();
                let (rects, _) = st.tab_layout(st.tabs.active);
                let frames: Vec<(Frame, (f32, f32))> = rects
                    .iter()
                    .filter_map(|(id, rect)| {
                        let pane = st.panes.get(id)?;
                        let is_focused = *id == focused;
                        let term = pane.session.term.lock();
                        let frame = Frame::capture(&term, st.window_focused && is_focused, !is_focused || st.cursor_on);
                        Some((frame, st.grid_origin(*rect)))
                    })
                    .collect();
                let refs: Vec<(&Frame, (f32, f32))> = frames.iter().map(|(f, o)| (f, *o)).collect();
                if st.renderer.render(&refs, &chrome, &mut st.fonts, GUTTER) || st.animating {
                    st.window.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let Some(st) = self.state.as_mut() else { return };
        let blinking = st.window_focused && st.focused().session.term.lock().cursor_style().blinking;
        // Elapsed times in the sidebar change every minute; refresh twice a minute.
        let now = Instant::now();
        if now >= st.next_tick {
            st.next_tick = now + Duration::from_secs(30);
            if st.sidebar && st.panes.values().any(|p| p.claude.is_some()) {
                st.window.request_redraw();
            }
        }
        if !blinking {
            st.cursor_on = true;
            event_loop.set_control_flow(ControlFlow::WaitUntil(st.next_tick));
            return;
        }
        if Instant::now() >= st.next_blink {
            st.cursor_on = !st.cursor_on;
            st.next_blink = Instant::now() + BLINK;
            st.window.request_redraw();
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(st.next_blink.min(st.next_tick)));
    }
}

const WEEKDAYS: [&str; 7] = ["lun", "mar", "mié", "jue", "vie", "sáb", "dom"];
const MONTHS: [&str; 12] = ["ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sep", "oct", "nov", "dic"];

/// Day number (days since 1970-01-01) → "23 oct".
fn short_date(day: i64) -> String {
    // Civil from days (Howard Hinnant).
    let z = day + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{d} {}", MONTHS[(m - 1) as usize])
}

/// Section title on the left, muted info on the right, hairline between.
#[allow(clippy::too_many_arguments)]
fn section_head(st: &mut State, out: &mut Vec<Chrome>, x: f32, y: f32, w: f32, title: &str, info: &str, s: f32) {
    let px = 10.5 * s;
    let t = title.to_uppercase();
    let tw = st.fonts.run(&t, Face::Sans, px, true).width;
    let iw = st.fonts.run(info, Face::Sans, 11.5 * s, false).width;
    out.push(label(x, y, t, Face::Sans, px, true, theme::MUTED, w));
    out.push(label(x + w - iw, y - s, info.into(), Face::Sans, 11.5 * s, false, theme::MUTED, iw + s));
    let line_x = x + tw + 10.0 * s;
    let line_w = w - tw - iw - 20.0 * s;
    if line_w > 0.0 {
        out.push(Chrome::Rect { rect: Rect { x: line_x, y: y + px * 0.65, w: line_w, h: s }, color: HAIRLINE });
    }
}

/// Mini bar chart of a series.
fn spark(out: &mut Vec<Chrome>, series: &[f64], r: Rect, color: Rgb, s: f32) {
    let max = series.iter().copied().fold(0.0, f64::max);
    if series.is_empty() || max <= 0.0 {
        return;
    }
    let slot = r.w / series.len() as f32;
    let bar = (slot * 0.65).max(1.0);
    for (i, v) in series.iter().enumerate() {
        let h = (*v / max) as f32 * r.h;
        if h > 0.0 {
            let h = h.max(s);
            out.push(Chrome::Rect { rect: Rect { x: r.x + i as f32 * slot, y: r.y + r.h - h, w: bar, h }, color });
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn label(x: f32, y: f32, text: String, face: Face, px: f32, bold: bool, color: Rgb, max_w: f32) -> Chrome {
    Chrome::Label { x, y, text, face, px, bold, color, max_w }
}

/// `color` at `alpha` over `base`, as an opaque colour.
fn mix(color: Rgb, base: Rgb, alpha: f32) -> Rgb {
    [0, 1, 2].map(|i| (color[i] as f32 * alpha + base[i] as f32 * (1.0 - alpha)).round() as u8)
}

fn folder_name(cwd: Option<PathBuf>) -> String {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    match cwd {
        Some(p) if Some(&p) == home.as_ref() => "~".into(),
        Some(p) => p.file_name().map_or("/".into(), |n| n.to_string_lossy().into_owned()),
        None => "zsh".into(),
    }
}

/// Name of the git repository containing `cwd`, else the folder name.
fn project_name(cwd: Option<PathBuf>) -> String {
    let Some(dir) = cwd else { return "zsh".into() };
    match dir.ancestors().find(|d| d.join(".git").exists()) {
        Some(root) => folder_name(Some(root.to_path_buf())),
        None => folder_name(Some(dir)),
    }
}

fn main() {
    let config = config::load();
    let event_loop = EventLoop::<UserEvent>::with_user_event().build().expect("event loop");
    let proxy = event_loop.create_proxy();
    let socket = status::socket_path();
    if let Err(e) = status::listen(socket.clone(), proxy.clone()) {
        eprintln!("cardumen: status socket unavailable ({e}); the sidebar will not show Claude Code states");
    }
    let mut app = App { socket: socket.clone(), config, proxy, state: None };
    event_loop.run_app(&mut app).expect("run");
    let _ = std::fs::remove_file(socket);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_and_mix() {
        assert_eq!(short_date(0), "1 ene");
        assert_eq!(short_date(20_781), "24 nov"); // 2026-11-24
        assert_eq!(mix([255, 0, 0], [0, 0, 0], 0.5), [128, 0, 0]);
    }
}
