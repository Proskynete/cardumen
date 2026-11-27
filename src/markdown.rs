//! Just enough Markdown for Claude's last message in the side panel:
//! headings, bullets, quotes, fenced code, **bold**, `code` and [links](url).
//! shortcut: no italics, tables or nesting; add them if Claude's summaries need them.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Plain,
    Bold,
    Code,
    Link,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub style: Style,
    /// Target of a `[text](url)` link.
    pub url: Option<String>,
}

/// One source line, already stripped of its block markers.
#[derive(Debug, PartialEq, Eq)]
pub struct Line {
    pub prefix: &'static str,
    pub spans: Vec<Span>,
}

pub fn parse(md: &str) -> Vec<Line> {
    let mut out = vec![];
    let mut in_fence = false;
    for raw in md.lines() {
        let t = raw.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            out.push(Line { prefix: "", spans: vec![Span { text: raw.to_string(), style: Style::Code, url: None }] });
            continue;
        }
        let hashes = t.chars().take_while(|&c| c == '#').count();
        let line = if (1..=6).contains(&hashes) && t[hashes..].starts_with(' ') {
            let spans = inline(t[hashes..].trim());
            let bold = |s: Span| Span { style: if s.style == Style::Plain { Style::Bold } else { s.style }, ..s };
            Line { prefix: "", spans: spans.into_iter().map(bold).collect() }
        } else if let Some(rest) = ["- ", "* ", "+ "].iter().find_map(|m| t.strip_prefix(m)) {
            Line { prefix: "• ", spans: inline(rest) }
        } else if let Some(rest) = t.strip_prefix('>') {
            Line { prefix: "│ ", spans: inline(rest.trim_start()) }
        } else if t.chars().all(|c| matches!(c, '-' | '*' | '_')) && t.len() >= 3 {
            continue; // horizontal rule
        } else {
            Line { prefix: "", spans: inline(t) }
        };
        out.push(line);
    }
    out
}

fn inline(s: &str) -> Vec<Span> {
    let mut spans: Vec<Span> = vec![];
    let mut rest = s;
    let mut bold = false;
    while !rest.is_empty() {
        let style = if bold { Style::Bold } else { Style::Plain };
        if let Some(r) = rest.strip_prefix("**") {
            // Only toggle on when a closing marker exists, so "2 ** 3" stays literal.
            if bold || r.contains("**") {
                bold = !bold;
            } else {
                push(&mut spans, "**", style);
            }
            rest = r;
        } else if let Some(r) = rest.strip_prefix('`') {
            match r.find('`') {
                Some(end) => {
                    push(&mut spans, &r[..end], Style::Code);
                    rest = &r[end + 1..];
                }
                None => {
                    push(&mut spans, "`", style);
                    rest = r;
                }
            }
        } else if let Some((text, url, after)) = rest.strip_prefix('[').and_then(link) {
            spans.push(Span { text: text.to_string(), style: Style::Link, url: Some(url.to_string()) });
            rest = after;
        } else {
            let n = rest.chars().next().map_or(1, char::len_utf8);
            push(&mut spans, &rest[..n], style);
            rest = &rest[n..];
        }
    }
    spans
}

/// Appends `text`, merging with the previous span when it has the same style.
fn push(spans: &mut Vec<Span>, text: &str, style: Style) {
    if text.is_empty() {
        return;
    }
    match spans.last_mut() {
        Some(last) if last.style == style && last.url.is_none() => last.text.push_str(text),
        _ => spans.push(Span { text: text.to_string(), style, url: None }),
    }
}

/// `text](url)rest` (after the `[`) into `(text, url, rest)`.
fn link(s: &str) -> Option<(&str, &str, &str)> {
    let close = s.find("](")?;
    let end = s[close + 2..].find(')')? + close + 2;
    Some((&s[..close], &s[close + 2..end], &s[end + 1..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(text: &str, style: Style) -> Span {
        Span { text: text.into(), style, url: None }
    }

    #[test]
    fn inline_styles() {
        assert_eq!(
            inline("Run **cargo test** and `git push` per [docs](http://x)."),
            vec![
                span("Run ", Style::Plain),
                span("cargo test", Style::Bold),
                span(" and ", Style::Plain),
                span("git push", Style::Code),
                span(" per ", Style::Plain),
                Span { text: "docs".into(), style: Style::Link, url: Some("http://x".into()) },
                span(".", Style::Plain),
            ]
        );
        assert_eq!(inline("2 ** 3 and `open"), vec![span("2 ** 3 and `open", Style::Plain)]);
        assert_eq!(inline("añadí **ñ**"), vec![span("añadí ", Style::Plain), span("ñ", Style::Bold)]);
    }

    #[test]
    fn blocks() {
        let lines = parse("## Hecho\n- uno\n> nota\n---\n```rust\nlet x = 1;\n```\nfin");
        let flat: Vec<(&str, Vec<(&str, Style)>)> =
            lines.iter().map(|l| (l.prefix, l.spans.iter().map(|s| (s.text.as_str(), s.style)).collect())).collect();
        assert_eq!(
            flat,
            vec![
                ("", vec![("Hecho", Style::Bold)]),
                ("• ", vec![("uno", Style::Plain)]),
                ("│ ", vec![("nota", Style::Plain)]),
                ("", vec![("let x = 1;", Style::Code)]),
                ("", vec![("fin", Style::Plain)]),
            ]
        );
    }
}
