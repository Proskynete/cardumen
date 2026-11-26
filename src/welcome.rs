//! Welcome banner: Tiburoncín in colour half-blocks plus a fastfetch-style info block.
use std::ffi::{CStr, CString};
use std::fmt::Write;

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
const RESET: &str = "\x1b[0m";

fn pixel(c: u8) -> Option<[u8; 3]> {
    match c {
        b'k' => Some([0x1A, 0x3A, 0x57]),
        b'b' => Some([0x3E, 0x7C, 0xB1]),
        b'w' => Some([0xED, 0xF4, 0xF3]),
        b'r' => Some([0xF4, 0x73, 0x6B]),
        _ => None,
    }
}

/// One string per text row (11 rows × 22 columns), each ending in a reset.
pub fn shark_rows() -> Vec<String> {
    PIXELS
        .chunks(2)
        .map(|pair| {
            let (top, bottom) = (pair[0].as_bytes(), pair[1].as_bytes());
            let mut row = String::new();
            for col in 0..top.len() {
                let fg = |c: [u8; 3]| format!("\x1b[38;2;{};{};{}m", c[0], c[1], c[2]);
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
    pub chip: String,
    pub ram_gb: u64,
    pub macos: String,
    pub uptime_secs: u64,
    pub shell: String,
    pub font: String,
}

impl SysInfo {
    pub fn read(user_name: Option<&str>, font: &str) -> Self {
        let boot = boot_time().unwrap_or(0);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let shell = std::env::var("SHELL").unwrap_or_default();
        Self {
            name: user_name.map(str::to_owned).unwrap_or_else(full_name),
            chip: sysctl_string("machdep.cpu.brand_string").unwrap_or_else(|| "unknown".into()),
            ram_gb: sysctl_u64("hw.memsize").unwrap_or(0) >> 30,
            macos: sysctl_string("kern.osproductversion").unwrap_or_default(),
            uptime_secs: now.saturating_sub(boot),
            shell: shell.rsplit('/').next().unwrap_or("zsh").to_string(),
            font: font.to_string(),
        }
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

/// Banner as terminal bytes (CRLF line endings, since it bypasses the tty).
pub fn banner(info: &SysInfo, cols: usize) -> String {
    let field = |label: &str, value: &str| format!("{MUTED}{label:<8}{PRIMARY}{value}{RESET}");
    let info_lines = [
        format!("\x1b[1m{PRIMARY}{}{RESET}", info.name),
        format!("{ACCENT}{}{RESET}", "─".repeat(info.name.chars().count())),
        field("chip", &info.chip),
        field("ram", &format!("{} GB", info.ram_gb)),
        field("macos", &info.macos),
        field("uptime", &format_uptime(info.uptime_secs)),
        field("shell", &info.shell),
        field("font", &info.font),
    ];
    let mut out = String::from("\r\n");
    if cols < 60 {
        for line in &info_lines {
            write!(out, "  {line}\r\n").unwrap();
        }
    } else {
        let shark = shark_rows();
        let offset = (shark.len() - info_lines.len()) / 2;
        for (i, row) in shark.iter().enumerate() {
            let right = i.checked_sub(offset).and_then(|j| info_lines.get(j));
            write!(out, "  {row}    {}\r\n", right.map_or("", |s| s.as_str())).unwrap();
        }
    }
    out.push_str("\r\n");
    out
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

fn sysctl_u64(name: &str) -> Option<u64> {
    let raw = sysctl_raw(name)?;
    Some(u64::from_ne_bytes(raw.get(..8)?.try_into().ok()?))
}

fn boot_time() -> Option<u64> {
    let raw = sysctl_raw("kern.boottime")?;
    // struct timeval starts with a 64-bit tv_sec on Apple Silicon.
    Some(i64::from_ne_bytes(raw.get(..8)?.try_into().ok()?) as u64)
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

    #[test]
    fn shark_is_22_by_11() {
        let rows = shark_rows();
        assert_eq!(rows.len(), 11);
        for row in &rows {
            let visible = row.chars().filter(|c| matches!(c, ' ' | '▀' | '▄')).count();
            assert_eq!(visible, 22);
        }
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
    fn uptime_format() {
        assert_eq!(format_uptime(59), "0m");
        assert_eq!(format_uptime(3_660), "1h 1m");
        assert_eq!(format_uptime(90_000), "1d 1h 0m");
    }

    #[test]
    fn narrow_banner_has_no_shark() {
        let info = SysInfo::read(Some("Test"), "Font");
        assert!(!banner(&info, 40).contains('▀'));
        assert!(banner(&info, 100).contains('▀'));
    }

    #[test]
    fn reads_real_hardware() {
        let info = SysInfo::read(None, "f");
        assert!(info.chip.starts_with("Apple"), "{}", info.chip);
        assert!(info.ram_gb >= 8);
        assert!(info.uptime_secs > 0);
    }
}
