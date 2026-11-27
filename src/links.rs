//! Finding and opening URLs: terminal text, panel Markdown links, the PR card.

/// The http(s) URL covering char index `col` of `line`, if any.
pub fn url_at(line: &str, col: usize) -> Option<String> {
    let chars: Vec<char> = line.chars().collect();
    let starts_with = |i: usize, p: &str| p.chars().enumerate().all(|(k, c)| chars.get(i + k) == Some(&c));
    let mut i = 0;
    while i < chars.len() {
        if !(starts_with(i, "https://") || starts_with(i, "http://")) {
            i += 1;
            continue;
        }
        let start = i;
        let mut end = start;
        while end < chars.len() && !chars[end].is_whitespace() && !"<>\"'`│".contains(chars[end]) {
            end += 1;
        }
        // Trailing punctuation belongs to the sentence; a `)` only to a URL that opened one.
        while end > start {
            let (c, url) = (chars[end - 1], &chars[start..end]);
            let count = |x: char| url.iter().filter(|&&u| u == x).count();
            if ".,;:!?".contains(c) || (c == ')' && count('(') < count(')')) || (c == ']' && count('[') < count(']')) {
                end -= 1;
            } else {
                break;
            }
        }
        if (start..end).contains(&col) {
            return Some(chars[start..end].iter().collect());
        }
        i = end.max(start + 1);
    }
    None
}

const IMAGE_EXTS: [&str; 6] = ["png", "jpg", "jpeg", "gif", "webp", "bmp"];

/// The image path covering char index `col` of `line` (as typed: maybe relative,
/// `~/`, quoted or with backslash-escaped spaces), unescaped.
pub fn image_path_at(line: &str, col: usize) -> Option<String> {
    let chars: Vec<char> = line.chars().collect();
    let escaped = |i: usize| i > 0 && chars[i - 1] == '\\';
    let part = |i: usize| !chars[i].is_whitespace() || escaped(i);
    if col >= chars.len() || !part(col) {
        return None;
    }
    let (mut start, mut end) = (col, col + 1);
    while start > 0 && part(start - 1) {
        start -= 1;
    }
    while end < chars.len() && part(end) {
        end += 1;
    }
    let token: String = (start..end).filter(|&i| chars[i] != '\\' || escaped(i)).map(|i| chars[i]).collect();
    let token = token.trim_start_matches(|c: char| "\"'`([<".contains(c)).trim_end_matches(|c: char| "\"'`)]>,;:.".contains(c));
    let ext = token.rsplit_once('.')?.1.to_ascii_lowercase();
    IMAGE_EXTS.contains(&ext.as_str()).then(|| token.to_string())
}

/// The repo's open PR/MR list page, from one PR/MR's URL.
pub fn pr_list_url(pr_url: &str) -> Option<String> {
    if let Some((base, _)) = pr_url.rsplit_once("/pull/") {
        return Some(format!("{base}/pulls"));
    }
    let (base, _) = pr_url.rsplit_once("/-/merge_requests/")?;
    Some(format!("{base}/-/merge_requests"))
}

/// Only web links: terminal output and Claude's text are untrusted, and `open`
/// would otherwise launch any app registered for a scheme (or run a file:// path).
pub fn is_web(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

pub fn open(url: &str) {
    if is_web(url) {
        let _ = std::process::Command::new("open").arg(url).spawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_url_under_column() {
        let line = "ver https://github.com/a/b/pull/2. y (https://x.dev/a_(b)) fin";
        assert_eq!(url_at(line, 4).as_deref(), Some("https://github.com/a/b/pull/2"));
        assert_eq!(url_at(line, 32).as_deref(), Some("https://github.com/a/b/pull/2"));
        assert_eq!(url_at(line, 33), None); // the period
        assert_eq!(url_at(line, 40).as_deref(), Some("https://x.dev/a_(b)"));
        assert_eq!(url_at(line, 0), None);
        assert_eq!(url_at("añadí http://ñ.cl/é ok", 8).as_deref(), Some("http://ñ.cl/é"));
    }

    #[test]
    fn finds_image_paths() {
        let line = r"ver /Users/a/Captura\ de\ pantalla.png y 'img/logo.JPG'. fin";
        assert_eq!(image_path_at(line, 10).as_deref(), Some("/Users/a/Captura de pantalla.png"));
        assert_eq!(image_path_at(line, 24).as_deref(), Some("/Users/a/Captura de pantalla.png"));
        assert_eq!(image_path_at(line, 44).as_deref(), Some("img/logo.JPG"));
        assert_eq!(image_path_at(line, 1), None);
        assert_eq!(image_path_at("src/main.rs", 3), None);
    }

    #[test]
    fn pr_list_urls() {
        assert_eq!(pr_list_url("https://github.com/a/b/pull/12").as_deref(), Some("https://github.com/a/b/pulls"));
        assert_eq!(pr_list_url("https://gl.dev/g/p/-/merge_requests/7").as_deref(), Some("https://gl.dev/g/p/-/merge_requests"));
        assert_eq!(pr_list_url(""), None);
    }

    #[test]
    fn only_web_schemes() {
        assert!(is_web("https://a.b"));
        assert!(!is_web("file:///etc/passwd"));
        assert!(!is_web("javascript:alert(1)"));
    }
}
