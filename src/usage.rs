//! Claude Code usage from the local transcripts (`~/.claude/projects/**/*.jsonl`),
//! with an incremental on-disk cache so later loads only parse new bytes.
use std::collections::{HashMap, HashSet};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use serde_json::Value;

/// One assistant message (tokens) or one user turn (`turn`, no tokens).
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Rec {
    pub ts: i64,
    pub file: u32,
    pub session: u32,
    pub project: u32,
    pub model: u32,
    pub input: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub output: u64,
    pub turn: bool,
}

#[derive(Clone, PartialEq, Debug, Default)]
struct FileState {
    size: u64,
    offset: u64,
    /// Last message id seen, to drop streaming duplicates across appends.
    last_id: Option<u32>,
}

#[derive(Default)]
pub struct Store {
    strings: Vec<String>,
    index: HashMap<String, u32>,
    files: HashMap<String, FileState>,
    pub recs: Vec<Rec>,
}

impl Store {
    fn intern(&mut self, s: &str) -> u32 {
        if let Some(&i) = self.index.get(s) {
            return i;
        }
        let i = self.strings.len() as u32;
        self.strings.push(s.to_string());
        self.index.insert(s.to_string(), i);
        i
    }

    pub fn name(&self, i: u32) -> &str {
        self.strings.get(i as usize).map_or("", String::as_str)
    }
}

/// "2026-10-23T20:10:37.102Z" → unix seconds (UTC).
pub fn parse_ts(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() < 19 {
        return None;
    }
    let num = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    let (y, m, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (hh, mm, ss) = (num(11..13)?, num(14..16)?, num(17..19)?);
    // Days from civil (Howard Hinnant).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + hh * 3600 + mm * 60 + ss)
}

/// Git repository name containing `cwd`, else its folder name.
fn project_of(cwd: &str) -> String {
    let dir = Path::new(cwd);
    let root = dir.ancestors().find(|d| d.join(".git").exists()).unwrap_or(dir);
    root.file_name().map_or("/".into(), |n| n.to_string_lossy().into_owned())
}

/// Is this user line something the person typed (not a tool result, command or meta)?
fn is_turn(v: &Value) -> bool {
    if v["isMeta"].as_bool() == Some(true) {
        return false;
    }
    match &v["message"]["content"] {
        Value::String(t) => {
            let t = t.trim_start();
            !t.is_empty() && !t.starts_with('<')
        }
        Value::Array(items) => {
            !items.iter().any(|i| i["type"] == "tool_result") && items.iter().any(|i| i["type"] == "text")
        }
        _ => false,
    }
}

/// Parse complete lines of one file into records.
fn parse_chunk(store: &mut Store, file: u32, state: &mut FileState, text: &str, projects: &mut HashMap<String, u32>) {
    for line in text.lines() {
        let assistant = line.contains("\"type\":\"assistant\"") && line.contains("\"usage\"");
        let user = !assistant && line.contains("\"type\":\"user\"") && !line.contains("\"tool_result\"");
        if !assistant && !user {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        let Some(ts) = v["timestamp"].as_str().and_then(parse_ts) else { continue };
        let session = store.intern(v["sessionId"].as_str().unwrap_or("?"));
        let cwd = v["cwd"].as_str().unwrap_or("");
        let project = match projects.get(cwd) {
            Some(&p) => p,
            None => {
                let p = store.intern(&project_of(cwd));
                projects.insert(cwd.to_string(), p);
                p
            }
        };
        if user {
            if is_turn(&v) {
                store.recs.push(Rec { ts, file, session, project, turn: true, ..Default::default() });
            }
            continue;
        }
        let msg = &v["message"];
        let id = store.intern(msg["id"].as_str().unwrap_or(""));
        if state.last_id == Some(id) {
            continue; // same message, another content block
        }
        state.last_id = Some(id);
        // Claude Code's own placeholder messages (errors, interruptions) carry no real usage.
        if msg["model"] == "<synthetic>" {
            continue;
        }
        let u = &msg["usage"];
        let n = |k: &str| u[k].as_u64().unwrap_or(0);
        let model = store.intern(msg["model"].as_str().unwrap_or("?"));
        store.recs.push(Rec {
            ts,
            file,
            session,
            project,
            model,
            input: n("input_tokens"),
            cache_read: n("cache_read_input_tokens"),
            cache_write: n("cache_creation_input_tokens"),
            output: n("output_tokens"),
            turn: false,
        });
    }
}

fn jsonl_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        match e.file_type() {
            Ok(t) if t.is_dir() => jsonl_files(&p, out),
            Ok(t) if t.is_file() && p.extension().is_some_and(|x| x == "jsonl") => out.push(p),
            _ => {}
        }
    }
}

/// Bring the store up to date with the transcripts under `root`.
pub fn refresh(store: &mut Store, root: &Path) {
    let mut paths = vec![];
    jsonl_files(root, &mut paths);
    let mut projects = HashMap::new();
    for path in paths {
        let key = path.to_string_lossy().into_owned();
        let Ok(size) = std::fs::metadata(&path).map(|m| m.len()) else { continue };
        let file = store.intern(&key);
        let mut state = store.files.get(&key).cloned().unwrap_or_default();
        if size < state.offset {
            // Rewritten or truncated: forget it and read again.
            store.recs.retain(|r| r.file != file);
            state = FileState::default();
        }
        if size == state.offset {
            continue;
        }
        let Ok(mut f) = std::fs::File::open(&path) else { continue };
        if f.seek(SeekFrom::Start(state.offset)).is_err() {
            continue;
        }
        let mut bytes = Vec::with_capacity((size - state.offset) as usize);
        if f.take(size - state.offset).read_to_end(&mut bytes).is_err() {
            continue;
        }
        // Only complete lines; a half-written last line waits for next time.
        let Some(end) = bytes.iter().rposition(|&b| b == b'\n').map(|i| i + 1) else { continue };
        let text = String::from_utf8_lossy(&bytes[..end]);
        parse_chunk(store, file, &mut state, &text, &mut projects);
        state.offset += end as u64;
        state.size = size;
        store.files.insert(key, state);
    }
}

// ---------- cache file ----------

const MAGIC: &[u8; 4] = b"CDU2";

pub fn cache_path() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Caches/cardumen/usage.bin"))
}

pub fn save(store: &Store, path: &Path) -> std::io::Result<()> {
    let mut b: Vec<u8> = Vec::with_capacity(64 + store.recs.len() * 64);
    let put_str = |b: &mut Vec<u8>, s: &str| {
        b.extend((s.len() as u32).to_le_bytes());
        b.extend(s.as_bytes());
    };
    b.extend(MAGIC);
    b.extend((store.strings.len() as u32).to_le_bytes());
    for s in &store.strings {
        put_str(&mut b, s);
    }
    b.extend((store.files.len() as u32).to_le_bytes());
    for (path, f) in &store.files {
        put_str(&mut b, path);
        b.extend(f.size.to_le_bytes());
        b.extend(f.offset.to_le_bytes());
        b.extend(f.last_id.unwrap_or(u32::MAX).to_le_bytes());
    }
    b.extend((store.recs.len() as u64).to_le_bytes());
    for r in &store.recs {
        b.extend(r.ts.to_le_bytes());
        for v in [r.file, r.session, r.project, r.model] {
            b.extend(v.to_le_bytes());
        }
        for v in [r.input, r.cache_read, r.cache_write, r.output] {
            b.extend(v.to_le_bytes());
        }
        b.push(r.turn as u8);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // Write then rename, so a crash never leaves a half-written cache.
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, b)?;
    std::fs::rename(tmp, path)
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let s = self.b.get(self.at..self.at + n)?;
        self.at += n;
        Some(s)
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }
    fn string(&mut self) -> Option<String> {
        let n = self.u32()? as usize;
        String::from_utf8(self.take(n)?.to_vec()).ok()
    }
}

/// `None` on any problem: the caller starts from an empty store.
pub fn load(path: &Path) -> Option<Store> {
    let bytes = std::fs::read(path).ok()?;
    let mut r = Reader { b: &bytes, at: 0 };
    if r.take(4)? != MAGIC {
        return None;
    }
    let mut store = Store::default();
    for _ in 0..r.u32()? {
        let s = r.string()?;
        store.index.insert(s.clone(), store.strings.len() as u32);
        store.strings.push(s);
    }
    for _ in 0..r.u32()? {
        let path = r.string()?;
        let (size, offset, last) = (r.u64()?, r.u64()?, r.u32()?);
        store.files.insert(path, FileState { size, offset, last_id: (last != u32::MAX).then_some(last) });
    }
    let n = r.u64()?;
    store.recs.reserve(n as usize);
    for _ in 0..n {
        let ts = r.u64()? as i64;
        let (file, session, project, model) = (r.u32()?, r.u32()?, r.u32()?, r.u32()?);
        let (input, cache_read, cache_write, output) = (r.u64()?, r.u64()?, r.u64()?, r.u64()?);
        let turn = r.take(1)?[0] == 1;
        store.recs.push(Rec { ts, file, session, project, model, input, cache_read, cache_write, output, turn });
    }
    Some(store)
}

// ---------- summary ----------

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Range {
    Days(i64),
    All,
}

#[derive(Clone, Debug, Default)]
pub struct Kpi {
    pub value: f64,
    /// Same metric for the previous period of equal length (`None` for "all").
    pub prev: Option<f64>,
    pub series: Vec<f64>,
}

#[derive(Clone, Debug, Default)]
pub struct Row {
    pub name: String,
    pub input: u64,
    pub output: u64,
    /// Tasks for models, agent seconds for projects.
    pub third: f64,
    pub share: f64,
}

#[derive(Clone, Debug, Default)]
pub struct Summary {
    pub first_day: i64,
    pub days: usize,
    pub active_days: usize,
    pub input: Kpi,
    pub cache_share: f64,
    pub output: Kpi,
    pub per_turn: f64,
    pub tasks: Kpi,
    pub turns: u64,
    pub agent_hours: Kpi,
    pub max_concurrent: usize,
    pub daily_in: Vec<u64>,
    pub daily_out: Vec<u64>,
    /// [weekday Monday=0][hour] message counts.
    pub heat: [[u32; 24]; 7],
    pub models: Vec<Row>,
    pub projects: Vec<Row>,
    pub models_total: usize,
    pub projects_total: usize,
}

const GAP: i64 = 300;

fn day_of(ts: i64, tz: i64) -> i64 {
    (ts + tz).div_euclid(86_400)
}

struct Totals {
    input: u64,
    cache_read: u64,
    output: u64,
    sessions: HashSet<u32>,
    turns: u64,
    agent_secs: f64,
}

/// Seconds of activity per session: gaps of at most 5 minutes between consecutive events.
fn agent_time(recs: &[&Rec], tz: i64, first_day: i64, days: usize) -> (HashMap<u32, f64>, Vec<f64>, usize) {
    let mut by_session: HashMap<u32, Vec<i64>> = HashMap::new();
    for r in recs {
        by_session.entry(r.session).or_default().push(r.ts);
    }
    let mut per_session = HashMap::new();
    let mut per_day = vec![0.0; days];
    let mut buckets: HashMap<i64, HashSet<u32>> = HashMap::new();
    for (session, mut ts) in by_session {
        ts.sort_unstable();
        let mut secs = 0.0;
        for w in ts.windows(2) {
            let gap = w[1] - w[0];
            if gap <= GAP {
                secs += gap as f64;
                let d = (day_of(w[1], tz) - first_day) as usize;
                if let Some(slot) = per_day.get_mut(d) {
                    *slot += gap as f64;
                }
            }
        }
        for t in &ts {
            buckets.entry(t / GAP).or_default().insert(session);
        }
        per_session.insert(session, secs);
    }
    let concurrent = buckets.values().map(HashSet::len).max().unwrap_or(0);
    (per_session, per_day, concurrent)
}

fn totals(recs: &[&Rec], tz: i64, first_day: i64, days: usize) -> Totals {
    let (agent, _, _) = agent_time(recs, tz, first_day, days);
    Totals {
        input: recs.iter().map(|r| r.input + r.cache_read + r.cache_write).sum(),
        cache_read: recs.iter().map(|r| r.cache_read).sum(),
        output: recs.iter().map(|r| r.output).sum(),
        sessions: recs.iter().map(|r| r.session).collect(),
        turns: recs.iter().filter(|r| r.turn).count() as u64,
        agent_secs: agent.values().sum(),
    }
}

fn table(rows: HashMap<u32, Row>, total_input: u64, store: &Store, limit: usize) -> Vec<Row> {
    let mut rows: Vec<Row> = rows
        .into_iter()
        .map(|(k, mut r)| {
            r.name = store.name(k).to_string();
            r.share = if total_input == 0 { 0.0 } else { r.input as f64 / total_input as f64 };
            r
        })
        .collect();
    rows.sort_by(|a, b| b.input.cmp(&a.input).then(b.output.cmp(&a.output)));
    if rows.len() > limit + 1 {
        let rest = rows.split_off(limit);
        rows.push(Row {
            name: format!("{} más", rest.len()),
            input: rest.iter().map(|r| r.input).sum(),
            output: rest.iter().map(|r| r.output).sum(),
            third: rest.iter().map(|r| r.third).sum(),
            share: rest.iter().map(|r| r.share).sum(),
        });
    }
    rows
}

pub fn summarize(store: &Store, range: Range, now: i64, tz: i64) -> Summary {
    let today = day_of(now, tz);
    let first_day = match range {
        Range::Days(n) => today - n + 1,
        Range::All => store.recs.iter().map(|r| day_of(r.ts, tz)).min().unwrap_or(today),
    };
    let days = (today - first_day + 1).max(1) as usize;
    let in_range: Vec<&Rec> = store.recs.iter().filter(|r| (first_day..=today).contains(&day_of(r.ts, tz))).collect();
    let prev: Option<Vec<&Rec>> = match range {
        Range::Days(n) => {
            Some(store.recs.iter().filter(|r| (first_day - n..first_day).contains(&day_of(r.ts, tz))).collect())
        }
        Range::All => None,
    };

    let t = totals(&in_range, tz, first_day, days);
    let p = prev.as_ref().map(|p| totals(p, tz, first_day - days as i64, days));
    let (agent, agent_daily, concurrent) = agent_time(&in_range, tz, first_day, days);

    let mut s = Summary { first_day, days, max_concurrent: concurrent, ..Default::default() };
    s.daily_in = vec![0; days];
    s.daily_out = vec![0; days];
    let mut daily_sessions: Vec<HashSet<u32>> = vec![HashSet::new(); days];
    let mut models: HashMap<u32, Row> = HashMap::new();
    let mut model_sessions: HashMap<u32, HashSet<u32>> = HashMap::new();
    let mut projects: HashMap<u32, Row> = HashMap::new();
    let mut session_project: HashMap<u32, u32> = HashMap::new();
    for r in &in_range {
        let d = (day_of(r.ts, tz) - first_day) as usize;
        daily_sessions[d].insert(r.session);
        session_project.entry(r.session).or_insert(r.project);
        if r.turn {
            continue;
        }
        let input = r.input + r.cache_read + r.cache_write;
        s.daily_in[d] += input;
        s.daily_out[d] += r.output;
        let local = r.ts + tz;
        let weekday = ((local.div_euclid(86_400) + 3).rem_euclid(7)) as usize; // 1970-01-01 was a Thursday
        let hour = (local.rem_euclid(86_400) / 3600) as usize;
        s.heat[weekday][hour] += 1;
        let m = models.entry(r.model).or_default();
        m.input += input;
        m.output += r.output;
        model_sessions.entry(r.model).or_default().insert(r.session);
        let pr = projects.entry(r.project).or_default();
        pr.input += input;
        pr.output += r.output;
    }
    for (model, sessions) in model_sessions {
        if let Some(m) = models.get_mut(&model) {
            m.third = sessions.len() as f64;
        }
    }
    for (session, secs) in &agent {
        if let Some(project) = session_project.get(session) {
            projects.entry(*project).or_default().third += secs;
        }
    }

    s.active_days = s.daily_in.iter().filter(|v| **v > 0).count();
    s.input = Kpi { value: t.input as f64, prev: p.as_ref().map(|p| p.input as f64), series: s.daily_in.iter().map(|v| *v as f64).collect() };
    s.cache_share = if t.input == 0 { 0.0 } else { t.cache_read as f64 / t.input as f64 };
    s.output = Kpi { value: t.output as f64, prev: p.as_ref().map(|p| p.output as f64), series: s.daily_out.iter().map(|v| *v as f64).collect() };
    s.turns = t.turns;
    s.per_turn = if t.turns == 0 { 0.0 } else { t.output as f64 / t.turns as f64 };
    s.tasks = Kpi {
        value: t.sessions.len() as f64,
        prev: p.as_ref().map(|p| p.sessions.len() as f64),
        series: daily_sessions.iter().map(|d| d.len() as f64).collect(),
    };
    s.agent_hours = Kpi { value: t.agent_secs / 3600.0, prev: p.as_ref().map(|p| p.agent_secs / 3600.0), series: agent_daily };
    s.models_total = models.len();
    s.projects_total = projects.len();
    s.models = table(models, t.input, store, 6);
    s.projects = table(projects, t.input, store, 6);
    s
}

/// Local UTC offset in seconds.
pub fn tz_offset() -> i64 {
    // SAFETY: localtime_r writes into the zeroed tm we own.
    unsafe {
        let now = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&now, &mut tm);
        tm.tm_gmtoff
    }
}

/// Load the cache, read new transcript bytes, save, and summarize. Runs off the UI thread.
pub fn compute(range: Range) -> Summary {
    let root = std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".claude/projects")).unwrap_or_default();
    let cache = cache_path();
    let mut store = cache.as_deref().and_then(load).unwrap_or_default();
    refresh(&mut store, &root);
    if let Some(c) = cache.as_deref() {
        if let Err(e) = save(&store, c) {
            eprintln!("cardumen: cannot write usage cache: {e}");
        }
    }
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
    summarize(&store, range, now, tz_offset())
}

/// "claude-opus-5-5" → "Opus 5.5".
pub fn model_name(id: &str) -> String {
    let rest = id.strip_prefix("claude-").unwrap_or(id);
    let mut parts = rest.split('-');
    let Some(family) = parts.next() else { return id.to_string() };
    let mut name: String = family.chars().take(1).flat_map(char::to_uppercase).chain(family.chars().skip(1)).collect();
    let version: Vec<&str> = parts.take_while(|p| p.len() <= 2 && p.chars().all(|c| c.is_ascii_digit())).collect();
    if !version.is_empty() {
        name.push(' ');
        name.push_str(&version.join("."));
    }
    name
}

/// "302 h", "23 min".
pub fn duration(hours: f64) -> String {
    if hours >= 1.0 { format!("{} h", human(hours.round())) } else { format!("{} min", (hours * 60.0).round()) }
}

/// "11.6B", "61.3M", "22.9K", "1,110".
pub fn human(n: f64) -> String {
    let n = n.max(0.0);
    let short = |v: f64, unit: &str| if v >= 100.0 { format!("{v:.0}{unit}") } else { format!("{v:.1}{unit}") };
    match n {
        n if n >= 1e9 => short(n / 1e9, "B"),
        n if n >= 1e6 => short(n / 1e6, "M"),
        n if n >= 1e4 => short(n / 1e3, "K"),
        n => {
            let s = format!("{:.0}", n);
            let mut out = String::new();
            for (i, c) in s.chars().enumerate() {
                if i > 0 && (s.len() - i) % 3 == 0 {
                    out.push(',');
                }
                out.push(c);
            }
            out
        }
    }
}

/// "+387%" style change; `None` when there is nothing to compare.
pub fn delta(k: &Kpi) -> Option<f64> {
    let prev = k.prev?;
    (prev > 0.0).then(|| (k.value - prev) / prev * 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps() {
        assert_eq!(parse_ts("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_ts("2026-10-23T20:10:37.102Z"), Some(1_792_786_237));
        assert_eq!(parse_ts("2000-02-29T12:00:00Z"), Some(951_825_600));
        assert_eq!(parse_ts("bad"), None);
    }

    fn assistant(id: &str, ts: &str, session: &str, out: u64) -> String {
        format!(
            r#"{{"type":"assistant","timestamp":"{ts}","sessionId":"{session}","cwd":"/tmp","message":{{"id":"{id}","model":"claude-opus-5-5","usage":{{"input_tokens":2,"cache_read_input_tokens":100,"cache_creation_input_tokens":10,"output_tokens":{out}}}}}}}"#
        )
    }

    #[test]
    fn parses_with_dedup_and_turns() {
        let mut store = Store::default();
        let mut state = FileState::default();
        let lines = [
            r#"{"type":"user","timestamp":"2026-11-01T10:00:00Z","sessionId":"s1","cwd":"/tmp","message":{"role":"user","content":"Hola, arregla el bug"}}"#.to_string(),
            r#"{"type":"user","timestamp":"2026-11-01T10:00:01Z","sessionId":"s1","cwd":"/tmp","message":{"role":"user","content":"<command-name>/clear</command-name>"}}"#.to_string(),
            r#"{"type":"user","timestamp":"2026-11-01T10:00:02Z","sessionId":"s1","cwd":"/tmp","isMeta":true,"message":{"role":"user","content":"meta"}}"#.to_string(),
            assistant("m1", "2026-11-01T10:00:05Z", "s1", 50),
            assistant("m1", "2026-11-01T10:00:05Z", "s1", 50),
            assistant("m2", "2026-11-01T10:01:00Z", "s1", 7),
            r#"{"type":"user","timestamp":"2026-11-01T10:01:01Z","sessionId":"s1","cwd":"/tmp","message":{"role":"user","content":[{"type":"tool_result","content":"ok"}]}}"#.to_string(),
        ]
        .join("\n");
        parse_chunk(&mut store, 0, &mut state, &lines, &mut HashMap::new());
        let turns = store.recs.iter().filter(|r| r.turn).count();
        let msgs: Vec<&Rec> = store.recs.iter().filter(|r| !r.turn).collect();
        assert_eq!(turns, 1);
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].output + msgs[1].output, 57);
        assert_eq!(msgs[0].cache_read, 100);
    }

    #[test]
    fn agent_time_and_summary() {
        let mut store = Store::default();
        let s = store.intern("s");
        let p = store.intern("proj");
        let m = store.intern("opus");
        let base = parse_ts("2026-11-20T10:00:00Z").unwrap();
        for (dt, out) in [(0, 10), (180, 10), (1800, 10)] {
            store.recs.push(Rec { ts: base + dt, session: s, project: p, model: m, input: 1, output: out, ..Default::default() });
        }
        store.recs.push(Rec { ts: base - 10, session: s, project: p, turn: true, ..Default::default() });
        let now = parse_ts("2026-11-25T12:00:00Z").unwrap();
        let sum = summarize(&store, Range::Days(7), now, 0);
        // 10s (turn→first) + 180s; the 27-minute gap does not count.
        assert!((sum.agent_hours.value * 3600.0 - 190.0).abs() < 0.01, "{}", sum.agent_hours.value * 3600.0);
        assert_eq!(sum.output.value, 30.0);
        assert_eq!(sum.tasks.value, 1.0);
        assert_eq!(sum.turns, 1);
        assert_eq!(sum.days, 7);
        assert_eq!(sum.models[0].name, "opus");
        assert_eq!(sum.projects[0].name, "proj");
        // 2026-11-20 is a Friday → weekday 4, 10h UTC.
        assert_eq!(sum.heat[4][10], 3);
        assert_eq!(sum.output.prev, Some(0.0));
        assert_eq!(delta(&sum.output), None);
    }

    #[test]
    fn cache_round_trip() {
        let mut store = Store::default();
        let a = store.intern("x");
        store.files.insert("/f.jsonl".into(), FileState { size: 10, offset: 8, last_id: Some(a) });
        store.recs.push(Rec { ts: 5, input: 1, cache_read: 2, cache_write: 3, output: 4, turn: true, ..Default::default() });
        let path = std::env::temp_dir().join(format!("cardumen-usage-test-{}.bin", std::process::id()));
        save(&store, &path).unwrap();
        let back = load(&path).unwrap();
        assert_eq!(back.recs, store.recs);
        assert_eq!(back.files, store.files);
        assert_eq!(back.name(a), "x");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn incremental_refresh() {
        let dir = std::env::temp_dir().join(format!("cardumen-usage-dir-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("p/sub")).unwrap();
        let file = dir.join("p/sub/a.jsonl");
        std::fs::write(&file, assistant("m1", "2026-11-01T10:00:00Z", "s", 1) + "\n").unwrap();
        let mut store = Store::default();
        refresh(&mut store, &dir);
        assert_eq!(store.recs.len(), 1);
        // Append one more message plus a half-written line.
        let mut more = assistant("m2", "2026-11-01T10:01:00Z", "s", 2) + "\n";
        more.push_str("{\"type\":\"assist");
        std::fs::write(&file, std::fs::read_to_string(&file).unwrap() + &more).unwrap();
        refresh(&mut store, &dir);
        assert_eq!(store.recs.len(), 2);
        refresh(&mut store, &dir);
        assert_eq!(store.recs.len(), 2, "nothing new");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn names_and_durations() {
        assert_eq!(model_name("claude-opus-5-5"), "Opus 5.5");
        assert_eq!(model_name("claude-haiku-5-5-20261001"), "Haiku 5.5");
        assert_eq!(model_name("gpt-6"), "Gpt 6");
        assert_eq!(duration(302.4), "302 h");
        assert_eq!(duration(0.38), "23 min");
    }

    #[test]
    fn number_format() {
        assert_eq!(human(11_600_000_000.0), "11.6B");
        assert_eq!(human(61_300_000.0), "61.3M");
        assert_eq!(human(22_900.0), "22.9K");
        assert_eq!(human(1110.0), "1,110");
        assert_eq!(human(402_000_000.0), "402M");
    }
}

#[cfg(test)]
mod bench {
    /// `cargo test --release usage_real -- --ignored --nocapture`: timings on this machine's data.
    #[test]
    #[ignore]
    fn usage_real() {
        for label in ["cold/first", "warm"] {
            let t = std::time::Instant::now();
            let s = super::compute(super::Range::Days(30));
            println!(
                "{label}: {:?} · in {} out {} tasks {} turns {} agent {:.0}h conc {} models {:?}",
                t.elapsed(),
                super::human(s.input.value),
                super::human(s.output.value),
                s.tasks.value,
                s.turns,
                s.agent_hours.value,
                s.max_concurrent,
                s.models.iter().map(|m| m.name.as_str()).collect::<Vec<_>>()
            );
        }
    }
}
