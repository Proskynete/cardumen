//! Fuzzy matching for the quick switcher (Cmd+P): the query's letters in order,
//! ignoring case and accents, preferring consecutive letters and word starts.

/// Lowercase without accents, so `contab` matches "Contabilidád".
fn fold(c: char) -> char {
    match c.to_lowercase().next().unwrap_or(c) {
        'á' | 'à' | 'ä' | 'â' => 'a',
        'é' | 'è' | 'ë' | 'ê' => 'e',
        'í' | 'ì' | 'ï' | 'î' => 'i',
        'ó' | 'ò' | 'ö' | 'ô' => 'o',
        'ú' | 'ù' | 'ü' | 'û' => 'u',
        'ñ' => 'n',
        c => c,
    }
}

/// `None` when `text` lacks the query's letters in order; higher is a better match.
/// Greedy left to right: good enough for tab titles, not for long text.
pub fn score(query: &str, text: &str) -> Option<i32> {
    let mut q = query.chars().filter(|c| !c.is_whitespace()).map(fold).peekable();
    let (mut total, mut prev_hit, mut prev) = (0, false, ' ');
    for c in text.chars() {
        let f = fold(c);
        let hit = q.peek() == Some(&f);
        if hit {
            q.next();
            total += 1 + if prev_hit { 3 } else { 0 } + if !prev.is_alphanumeric() { 2 } else { 0 };
        }
        prev_hit = hit;
        prev = c;
    }
    q.peek().is_none().then_some(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches() {
        assert_eq!(score("", "cardumen"), Some(0));
        assert!(score("cnt", "contabilidad").is_some());
        assert!(score("contab", "Contabilidád · sii").is_some());
        assert_eq!(score("xyz", "cardumen"), None);
        assert_eq!(score("ca", "ac"), None);
        // Prefix beats letters scattered over the text.
        assert!(score("car", "cardumen").unwrap() > score("car", "contabilidad report").unwrap());
        // Word start beats the middle of a word.
        assert!(score("b", "mi blog").unwrap() > score("b", "abc").unwrap());
    }
}
