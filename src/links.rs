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
    fn only_web_schemes() {
        assert!(is_web("https://a.b"));
        assert!(!is_web("file:///etc/passwd"));
        assert!(!is_web("javascript:alert(1)"));
    }
}
