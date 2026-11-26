//! Welcome banner: Tiburoncín in colour half-blocks plus a fastfetch-style info block.
use std::ffi::{CStr, CString};
use std::fmt::Write;

use crate::theme;

/// Tiburoncín, 22 × 22, from eduardoalvarez.dev (`src/lib/tiburoncin.ts`).
/// `.` empty · `k` outline · `b` body · `w` foam · `r` tongue.
const PIXELS: [&str; 22] = [
    "....kk................",
    "...kbbkk..............",
    "...kbbbbk.............",
    "...kbbbbbk............",
    "...kbbbbbkkkkkk.......",
    "...kbbkkbbbbbbbkk.....",
    "...kkkbbbbbbwbbbbkk...",
    "...kkbbbbbbbbbbbbbk...",
    "..kkbwbbbbbbbbbbwbbk..",
    "..kbbbbbbwbbbbbbbbbbk.",
    ".kbbbbbbbbbbbbbbbbwbk.",
    ".kbbbbbbbbbwbbbbbkwkbk",
    "kbbbbbwbbbbbbbbbbkkkbk",
    "kbbbbkbkbbbwwwwwwwbbbk",
    "kbbbbbbbbwwwkkkkwwwwbk",
    "kbbbbbwwwwkkkkkkwwwww.",
    "kkbbwwwwwwrrrrkwwwwwk.",
    ".kbbwwwwwwwrrrwwwwwk..",
    ".kkbwwwwwwwwwwwwwwkk..",
    "..kkwwwwwwwwwwwwwk....",
    "....kkwwwwwwwwkkk.....",
    ".......kkkkkk.........",
];


const MUTED: &str = "\x1b[38;2;113;145;156m";
const PRIMARY: &str = "\x1b[38;2;237;244;243m";
const ACCENT: &str = "\x1b[38;2;53;214;192m";
const WARM: &str = "\x1b[38;2;242;166;90m";
const ERROR: &str = "\x1b[38;2;225;87;87m";
const RESET: &str = "\x1b[0m";

/// Visible width of the info boxes, in columns.
const BOX_W: usize = 48;
const GAP: usize = 4;
const MARGIN: usize = 2;

fn pixel(c: u8) -> Option<[u8; 3]> {
    match c {
        b'k' => Some([0x1A, 0x3A, 0x57]),
        b'b' => Some([0x3E, 0x7C, 0xB1]),
        b'w' => Some([0xED, 0xF4, 0xF3]),
        b'r' => Some([0xF4, 0x73, 0x6B]),
        _ => None,
    }
}

fn fg(c: [u8; 3]) -> String {
    format!("\x1b[38;2;{};{};{}m", c[0], c[1], c[2])
}

/// Small Tiburoncín: half-blocks, 22 columns × 11 rows.
pub fn shark_rows() -> Vec<String> {
    PIXELS
        .chunks(2)
        .map(|pair| {
            let (top, bottom) = (pair[0].as_bytes(), pair[1].as_bytes());
            let mut row = String::new();
            for col in 0..top.len() {
                let bg = |c: [u8; 3]| format!("\x1b[48;2;{};{};{}m", c[0], c[1], c[2]);
                match (pixel(top[col]), pixel(bottom[col])) {
                    (None, None) => row.push_str("\x1b[0m "),
                    (Some(t), None) => write!(row, "\x1b[49m{}▀", fg(t)).unwrap(),
                    (None, Some(b)) => write!(row, "\x1b[49m{}▄", fg(b)).unwrap(),
                    (Some(t), Some(b)) => write!(row, "{}{}▀", fg(t), bg(b)).unwrap(),
                }
            }
            row.push_str(RESET);
            row
        })
        .collect()
}

pub struct SysInfo {
    pub name: String,
    pub cpu: String,
    pub display: Option<String>,
    pub disk: (u64, u64),
    pub memory: (u64, u64),
    pub swap: (u64, u64),
    pub macos: String,
    pub kernel: String,
    pub shell: String,
    pub packages: usize,
    pub font: String,
    pub os_installed: Option<i64>,
    pub uptime_secs: u64,
    pub last_update: Option<i64>,
}

impl SysInfo {
    pub fn read(user_name: Option<&str>, font: &str, display: Option<String>) -> Self {
        let now = unix_now();
        let chip = sysctl_string("machdep.cpu.brand_string").unwrap_or_else(|| "unknown".into());
        let p = sysctl_u64("hw.perflevel0.physicalcpu").unwrap_or(0);
        let e = sysctl_u64("hw.perflevel1.physicalcpu").unwrap_or(0);
        let cpu = if p + e > 0 { format!("{chip} · {} núcleos ({p}P + {e}E)", p + e) } else { chip };
        let total_mem = sysctl_u64("hw.memsize").unwrap_or(0);
        let page = sysctl_u64("hw.pagesize").unwrap_or(16_384);
        // Free + file cache + purgeable is memory macOS hands out on demand; the rest is "used".
        let reclaimable = ["vm.page_free_count", "vm.page_pageable_external_count", "vm.page_purgeable_count"]
            .iter()
            .filter_map(|n| sysctl_u64(n))
            .sum::<u64>()
            * page;
        let version = sysctl_string("kern.osproductversion").unwrap_or_default();
        let shell = std::env::var("SHELL").unwrap_or_default();
        Self {
            name: user_name.map(str::to_owned).unwrap_or_else(full_name),
            cpu,
            display,
            disk: disk_usage("/System/Volumes/Data").unwrap_or((0, 0)),
            memory: (total_mem.saturating_sub(reclaimable), total_mem),
            swap: swap_usage().unwrap_or((0, 0)),
            macos: format!("macOS {} {version}", codename(&version)).replace("  ", " "),
            kernel: format!("Darwin {}", sysctl_string("kern.osrelease").unwrap_or_default()),
            shell: shell.rsplit('/').next().unwrap_or("zsh").to_string(),
            packages: ["/opt/homebrew/Cellar", "/opt/homebrew/Caskroom"]
                .iter()
                .filter_map(|d| std::fs::read_dir(d).ok())
                .map(|d| d.count())
                .sum(),
            font: font.to_string(),
            os_installed: birth_time("/var/db/.AppleSetupDone"),
            uptime_secs: boot_time().map_or(0, |b| now.saturating_sub(b)),
            last_update: modified_time("/Library/Receipts/InstallHistory.plist"),
        }
    }
}

fn codename(version: &str) -> &'static str {
    match version.split('.').next().unwrap_or("") {
        "26" => "Tahoe",
        "15" => "Sequoia",
        "14" => "Sonoma",
        "13" => "Ventura",
        _ => "",
    }
}

pub fn format_uptime(secs: u64) -> String {
    let (d, h, m) = (secs / 86_400, secs / 3_600 % 24, secs / 60 % 60);
    match (d, h) {
        (0, 0) => format!("{m}m"),
        (0, _) => format!("{h}h {m}m"),
        _ => format!("{d}d {h}h {m}m"),
    }
}

fn gib(bytes: u64) -> String {
    let g = bytes as f64 / (1u64 << 30) as f64;
    if g >= 100.0 { format!("{g:.0} GiB") } else { format!("{g:.1} GiB") }
}

/// "used / total (pct%)", with the percentage coloured by how full it is.
pub fn usage(used: u64, total: u64) -> String {
    let pct = if total == 0 { 0 } else { (used as f64 / total as f64 * 100.0).round() as u64 };
    let color = match pct {
        90.. => ERROR,
        70..=89 => WARM,
        _ => ACCENT,
    };
    format!("{} / {} ({color}{pct}%{PRIMARY})", gib(used), gib(total))
}

const MONTHS: [&str; 12] = ["ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sep", "oct", "nov", "dic"];

/// Local date like "13 nov 2026, 19:57" (short enough to fit the box).
fn format_date(ts: i64) -> String {
    // SAFETY: localtime_r writes into the zeroed tm we own.
    let tm = unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&ts, &mut tm);
        tm
    };
    format!(
        "{} {} {}, {:02}:{:02}",
        tm.tm_mday,
        MONTHS[tm.tm_mon as usize % 12],
        tm.tm_year + 1900,
        tm.tm_hour,
        tm.tm_min
    )
}

fn format_age(secs: u64) -> String {
    let days = secs / 86_400;
    match days {
        0..=60 => format!("{days} días"),
        _ if days < 730 => format!("{} meses", days / 30),
        _ => format!("{} años, {} meses", days / 365, days % 365 / 30),
    }
}

/// A box: `┌── title ──┐`, one `│ icon  value` line per item, `└────┘`.
fn section(title: &str, items: &[(char, String)]) -> Vec<String> {
    let inner = BOX_W - 2;
    let label = format!(" {title} ");
    let left = (inner - label.chars().count()) / 2;
    let right = inner - left - label.chars().count();
    let mut out = vec![format!("{MUTED}┌{}{ACCENT}{label}{MUTED}{}┐{RESET}", "─".repeat(left), "─".repeat(right))];
    for (icon, value) in items {
        out.push(format!("{MUTED}│ {ACCENT}{icon}  {PRIMARY}{value}{RESET}"));
    }
    out.push(format!("{MUTED}└{}┘{RESET}", "─".repeat(inner)));
    out
}

pub fn info_lines(info: &SysInfo) -> Vec<String> {
    let now = unix_now() as i64;
    let mut hardware = vec![('\u{f2db}', info.cpu.clone())];
    if let Some(d) = &info.display {
        hardware.push(('\u{f108}', d.clone()));
    }
    hardware.push(('\u{f0a0}', format!("{}  disco", usage(info.disk.0, info.disk.1))));
    hardware.push(('\u{f233}', format!("{}  memoria", usage(info.memory.0, info.memory.1))));
    hardware.push(('\u{f0ec}', format!("{}  swap", usage(info.swap.0, info.swap.1))));
    let dots: String = [theme::ACCENT, [0xF2, 0xA6, 0x5A], [0x4F, 0xB4, 0x77], [0xE1, 0x57, 0x57], [0x3E, 0x7C, 0xB1], theme::FOREGROUND]
        .iter()
        .map(|c| format!("{}●", fg(*c)))
        .collect();
    let software = [
        ('\u{f179}', info.macos.clone()),
        ('\u{f013}', info.kernel.clone()),
        ('\u{f120}', info.shell.clone()),
        ('\u{f2d0}', format!("cardumen {}", env!("CARGO_PKG_VERSION"))),
        ('\u{f0fc}', format!("{} (brew)", info.packages)),
        ('\u{f1fc}', format!("arrecife {dots}")),
        ('\u{f031}', info.font.clone()),
    ];
    let mut time = vec![];
    if let Some(t) = info.os_installed {
        time.push(('\u{f254}', format!("{MUTED}instalado {PRIMARY}hace {}", format_age(now.saturating_sub(t) as u64))));
    }
    time.push(('\u{f017}', format!("{MUTED}encendido {PRIMARY}{}", format_uptime(info.uptime_secs))));
    if let Some(t) = info.last_update {
        time.push(('\u{f021}', format!("{MUTED}actualizado {PRIMARY}{}", format_date(t))));
    }
    let mut lines = vec![format!("\x1b[1m{PRIMARY}{}{RESET}", info.name)];
    lines.extend(section("Hardware", &hardware));
    lines.extend(section("Software", &software));
    lines.extend(section("Tiempo", &time));
    lines
}

/// Banner as terminal bytes (CRLF line endings, since it bypasses the tty).
pub fn banner(info: &SysInfo, cols: usize) -> String {
    let right = info_lines(info);
    let (left, left_w) = if cols >= MARGIN + 22 + GAP + BOX_W {
        (shark_rows(), 22)
    } else {
        (vec![], 0)
    };
    let rows = left.len().max(right.len());
    let offset = (rows - left.len()) / 2;
    let gap = if left_w > 0 { " ".repeat(GAP) } else { String::new() };
    let mut out = String::from("\r\n");
    for i in 0..rows {
        let l = i.checked_sub(offset).and_then(|j| left.get(j)).cloned().unwrap_or_else(|| " ".repeat(left_w));
        let r = right.get(i).map_or("", |s| s.as_str());
        write!(out, "{}{l}{gap}{r}\r\n", " ".repeat(MARGIN)).unwrap();
    }
    out.push_str("\r\n");
    out
}

fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn sysctl_raw(name: &str) -> Option<Vec<u8>> {
    let cname = CString::new(name).ok()?;
    let mut len = 0usize;
    // SAFETY: first call only queries the size; second writes at most `len` bytes into `buf`.
    unsafe {
        if libc::sysctlbyname(cname.as_ptr(), std::ptr::null_mut(), &mut len, std::ptr::null_mut(), 0) != 0 {
            return None;
        }
        let mut buf = vec![0u8; len];
        if libc::sysctlbyname(cname.as_ptr(), buf.as_mut_ptr().cast(), &mut len, std::ptr::null_mut(), 0) != 0 {
            return None;
        }
        buf.truncate(len);
        Some(buf)
    }
}

fn sysctl_string(name: &str) -> Option<String> {
    let raw = sysctl_raw(name)?;
    Some(String::from_utf8_lossy(&raw).trim_end_matches('\0').trim().to_string())
}

/// Integer sysctl; some are 32-bit (page counts, core counts), some 64-bit.
fn sysctl_u64(name: &str) -> Option<u64> {
    let raw = sysctl_raw(name)?;
    match raw.len() {
        4 => Some(u32::from_ne_bytes(raw[..4].try_into().ok()?) as u64),
        8 => Some(u64::from_ne_bytes(raw[..8].try_into().ok()?)),
        _ => None,
    }
}

fn boot_time() -> Option<u64> {
    let raw = sysctl_raw("kern.boottime")?;
    // struct timeval starts with a 64-bit tv_sec on Apple Silicon.
    Some(i64::from_ne_bytes(raw.get(..8)?.try_into().ok()?) as u64)
}

/// `vm.swapusage` is `struct xsw_usage { u64 total, avail, used; ... }`.
fn swap_usage() -> Option<(u64, u64)> {
    let raw = sysctl_raw("vm.swapusage")?;
    let field = |i: usize| raw.get(i * 8..i * 8 + 8).and_then(|b| b.try_into().ok()).map(u64::from_ne_bytes);
    Some((field(2)?, field(0)?))
}

fn disk_usage(path: &str) -> Option<(u64, u64)> {
    let cpath = CString::new(path).ok()?;
    // SAFETY: statfs fills the zeroed struct we own.
    let st = unsafe {
        let mut st: libc::statfs = std::mem::zeroed();
        if libc::statfs(cpath.as_ptr(), &mut st) != 0 {
            return None;
        }
        st
    };
    let total = st.f_blocks * st.f_bsize as u64;
    Some((total - st.f_bavail * st.f_bsize as u64, total))
}

fn birth_time(path: &str) -> Option<i64> {
    let meta = std::fs::metadata(path).ok()?;
    meta.created().ok()?.duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_secs() as i64)
}

fn modified_time(path: &str) -> Option<i64> {
    let meta = std::fs::metadata(path).ok()?;
    meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_secs() as i64)
}

fn full_name() -> String {
    // SAFETY: getpwuid returns a pointer to static storage or null; we copy out immediately.
    unsafe {
        let pw = libc::getpwuid(libc::getuid());
        if !pw.is_null() && !(*pw).pw_gecos.is_null() {
            let gecos = CStr::from_ptr((*pw).pw_gecos).to_string_lossy();
            let name = gecos.split(',').next().unwrap_or("").trim();
            if !name.is_empty() {
                return name.to_string();
            }
        }
    }
    std::env::var("USER").unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn visible(s: &str) -> usize {
        // Strip SGR sequences, then count chars.
        let mut n = 0;
        let mut in_esc = false;
        for c in s.chars() {
            match (in_esc, c) {
                (false, '\x1b') => in_esc = true,
                (true, 'm') => in_esc = false,
                (true, _) => {}
                _ => n += 1,
            }
        }
        n
    }

    #[test]
    fn shark_is_22_by_11() {
        let rows = shark_rows();
        assert_eq!(rows.len(), 11);
        assert!(rows.iter().all(|r| visible(r) == 22));
    }

    #[test]
    fn opaque_pairs_use_upper_half_block() {
        let row = &shark_rows()[6]; // rows 12+13
        assert!(row.contains("\x1b[38;2;62;124;177m\x1b[48;2;62;124;177m▀"));
        assert!(row.contains("\x1b[38;2;62;124;177m\x1b[48;2;26;58;87m▀"), "b over k");
    }

    #[test]
    fn transparent_pair_is_plain_space() {
        assert!(shark_rows()[0].starts_with("\x1b[0m \x1b[0m \x1b[0m "));
    }

    #[test]
    fn uptime_and_age_format() {
        assert_eq!(format_uptime(59), "0m");
        assert_eq!(format_uptime(3_660), "1h 1m");
        assert_eq!(format_uptime(90_000), "1d 1h 0m");
        assert_eq!(format_age(10 * 86_400), "10 días");
        assert_eq!(format_age(1_675 * 86_400), "4 años, 7 meses");
    }

    #[test]
    fn usage_colours_by_fullness() {
        assert!(usage(95, 100).contains(&format!("{ERROR}95%")));
        assert!(usage(75, 100).contains(&format!("{WARM}75%")));
        assert!(usage(10, 100).contains(&format!("{ACCENT}10%")));
    }

    #[test]
    fn section_borders_have_box_width() {
        let s = section("Hardware", &[('x', "v".into())]);
        assert_eq!(visible(&s[0]), BOX_W);
        assert_eq!(visible(s.last().unwrap()), BOX_W);
    }

    #[test]
    fn layout_degrades_with_width() {
        let info = SysInfo::read(Some("Test"), "Font", None);
        assert!(banner(&info, 80).contains('▀'));
        assert!(!banner(&info, 60).contains('▀'));
    }

    #[test]
    fn reads_real_hardware() {
        let info = SysInfo::read(None, "f", None);
        assert!(info.cpu.starts_with("Apple"), "{}", info.cpu);
        assert!(info.memory.1 >= 8 << 30 && info.memory.0 > 0 && info.memory.0 < info.memory.1);
        assert!(info.disk.1 > 0 && info.disk.0 > 0);
        assert!(info.uptime_secs > 0);
        assert!(info.macos.starts_with("macOS"));
    }
}
