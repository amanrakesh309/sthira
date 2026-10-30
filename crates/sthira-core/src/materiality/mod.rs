use crate::vault::normalize_content;
use crate::Materiality;

fn levenshtein_distance(a: &[char], b: &[char]) -> usize {
    let mut prev = (0..=b.len()).collect::<Vec<usize>>();
    let mut curr = vec![0; b.len() + 1];

    for (i, &ca) in a.iter().enumerate() {
        curr[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            curr[j + 1] = (prev[j + 1] + 1).min(curr[j] + 1).min(prev[j] + cost);
        }
        prev.copy_from_slice(&curr);
    }

    prev[b.len()]
}

pub fn compute_materiality(old_content: &str, new_content: &str) -> Materiality {
    let norm_old = normalize_content(old_content);
    let norm_new = normalize_content(new_content);

    if norm_old == norm_new {
        return Materiality::None;
    }

    // 1. Digit/number changes (e.g. "32GB" -> "8GB")
    let old_digits: Vec<char> = norm_old.chars().filter(|c| c.is_ascii_digit()).collect();
    let new_digits: Vec<char> = norm_new.chars().filter(|c| c.is_ascii_digit()).collect();
    if old_digits != new_digits {
        return Materiality::Substantive;
    }

    // 2. Negation token changes (e.g. "will use" -> "will not use")
    const NEGATION_WORDS: &[&str] = &[
        "not", "no", "never", "don't", "dont", "won't", "wont", "cannot", "cant", "can't",
    ];
    let extract_negations = |text: &str| -> Vec<String> {
        text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric() && c != '\'')
            .filter(|w| NEGATION_WORDS.contains(w))
            .map(|s| s.to_string())
            .collect()
    };
    if extract_negations(&norm_old) != extract_negations(&norm_new) {
        return Materiality::Substantive;
    }

    // 3. Technical identifier changes (e.g. "sqlite-vec", "token-2.5")
    let extract_identifiers = |text: &str| -> Vec<String> {
        text.split_whitespace()
            .filter_map(|w| {
                let clean = w.trim_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '_' && c != '.');
                let inner = clean.trim_matches(|c: char| c == '.' || c == ',' || c == '!' || c == '?' || c == ':' || c == ';');
                if inner.is_empty() {
                    return None;
                }
                let has_internal_punct = inner.contains('-')
                    || inner.contains('_')
                    || (inner.contains('.') && !inner.starts_with('.') && !inner.ends_with('.'));
                let has_mixed = inner.chars().any(|c| c.is_uppercase()) && inner.chars().any(|c| c.is_lowercase());
                let has_alpha_num = inner.chars().any(|c| c.is_alphabetic()) && inner.chars().any(|c| c.is_ascii_digit());
                if has_internal_punct || (has_mixed && has_alpha_num) {
                    Some(inner.to_string())
                } else {
                    None
                }
            })
            .collect()
    };
    if extract_identifiers(&norm_old) != extract_identifiers(&norm_new) {
        return Materiality::Substantive;
    }

    // 4. Non-whitespace edit distance
    let old_non_ws: Vec<char> = norm_old.chars().filter(|c| !c.is_whitespace()).collect();
    let new_non_ws: Vec<char> = norm_new.chars().filter(|c| !c.is_whitespace()).collect();

    let diff = (old_non_ws.len() as isize - new_non_ws.len() as isize).unsigned_abs();
    if diff >= 8 {
        return Materiality::Substantive;
    }

    let lev = levenshtein_distance(&old_non_ws, &new_non_ws);
    if lev >= 8 {
        Materiality::Substantive
    } else {
        Materiality::Minor
    }
}
