pub mod extractor;

use crate::Authorship;
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EpistemicOrigin {
    UserRecorded,
    SourceBacked,
    AiInferred,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    #[serde(alias = "question", alias = "Question", alias = "QUESTION")]
    Question,
    #[serde(alias = "decision", alias = "Decision", alias = "DECISION")]
    Decision,
    #[serde(alias = "finding", alias = "Finding", alias = "FINDING")]
    Finding,
    #[serde(alias = "hypothesis", alias = "Hypothesis", alias = "HYPOTHESIS")]
    Hypothesis,
    #[serde(alias = "rejected_approach", alias = "RejectedApproach", alias = "REJECTED_APPROACH")]
    RejectedApproach,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeLifecycle {
    Open,
    Resolved,
    Superseded,
    Stale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewState {
    Unreviewed,
    Confirmed,
    Dismissed,
}

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum QuoteError {
    #[error("Quote too short (minimum 12 characters)")]
    TooShort,
    #[error("Quote not found in artifact")]
    NotFound,
    #[error("Quote is inside a blockquote")]
    InsideBlockquote,
    #[error("Quote is inside a code block")]
    InsideCodeBlock,
    #[error("Basis content hash mismatch")]
    BasisMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedQuote {
    pub text: String,
    pub start: usize,
    pub end: usize,
    pub quote_hash: String,
    pub basis_hash: String,
}

pub fn normalize_quote_text(raw: &str) -> String {
    let s = raw
        .replace(['“', '”'], "\"")
        .replace(['‘', '’'], "'")
        .replace("\r\n", "\n")
        .replace('\r', "\n");

    // Collapse whitespace runs
    let mut collapsed = String::with_capacity(s.len());
    let mut in_ws = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !in_ws {
                collapsed.push(' ');
                in_ws = true;
            }
        } else {
            collapsed.push(ch);
            in_ws = false;
        }
    }
    collapsed.trim().nfc().collect()
}

pub fn is_mid_sentence(body: &str) -> bool {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return false;
    }

    // Check terminal characters
    let last_char = trimmed.chars().last().unwrap();
    if matches!(last_char, '.' | '?' | '!' | '`' | '#' | '>' | '-' | '*' | ':' | '}') {
        return false;
    }

    // Check if ends with fenced block or table or horizontal rule
    if trimmed.ends_with("---") || trimmed.ends_with("```") || trimmed.ends_with('|') {
        return false;
    }

    true
}

const HEDGE_WORDS: &[&str] = &[
    "maybe", "might", "could", "perhaps", "probably", "possibly", "tbd",
];

const HEDGE_PHRASES: &[&str] = &[
    "what if", "should we", "thinking about", "not sure",
];

pub fn is_hedged(text: &str) -> bool {
    let lower = text.to_lowercase();
    for phrase in HEDGE_PHRASES {
        if lower.contains(phrase) {
            return true;
        }
    }

    // Tokenized word boundary check for single words
    for word in lower
        .split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|w| !w.is_empty())
    {
        if HEDGE_WORDS.contains(&word) {
            return true;
        }
    }

    false
}

impl VerifiedQuote {
    pub fn verify(artifact_body: &str, basis_hash: &str, claimed: &str) -> Result<Self, QuoteError> {
        let norm_claimed = normalize_quote_text(claimed);
        if norm_claimed.len() < 12 {
            return Err(QuoteError::TooShort);
        }

        // Build line-by-line regions to forbid blockquotes and code fences
        let mut in_code_fence = false;
        let mut byte_offset = 0;
        let mut forbidden_spans: Vec<(usize, usize, QuoteError)> = Vec::new();

        for line in artifact_body.lines() {
            let line_len = line.len();
            let line_start = byte_offset;
            let line_end = byte_offset + line_len;
            let trimmed = line.trim();

            if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                in_code_fence = !in_code_fence;
                forbidden_spans.push((line_start, line_end, QuoteError::InsideCodeBlock));
            } else if in_code_fence {
                forbidden_spans.push((line_start, line_end, QuoteError::InsideCodeBlock));
            } else if trimmed.starts_with('>') {
                forbidden_spans.push((line_start, line_end, QuoteError::InsideBlockquote));
            }

            // +1 for newline character in lines()
            byte_offset = line_end + 1;
        }

        // Exact match check first
        if let Some(pos) = artifact_body.find(&norm_claimed) {
            let end_pos = pos + norm_claimed.len();

            // Check if intersects forbidden spans
            for &(f_start, f_end, ref err) in &forbidden_spans {
                if pos < f_end && end_pos > f_start {
                    return Err(err.clone());
                }
            }

            let mut hasher = Sha256::new();
            hasher.update(norm_claimed.as_bytes());
            let quote_hash = format!("{:x}", hasher.finalize());

            return Ok(Self {
                text: norm_claimed,
                start: pos,
                end: end_pos,
                quote_hash,
                basis_hash: basis_hash.to_string(),
            });
        }

        // If not exact byte-for-byte, search via sliding normalized line window
        let lines: Vec<&str> = artifact_body.lines().collect();
        let mut line_offset = 0;

        for line in &lines {
            let line_len = line.len();
            let norm_line = normalize_quote_text(line);

            if let Some(line_pos) = norm_line.find(&norm_claimed) {
                let start_byte = line_offset;
                let end_byte = line_offset + line_len;

                for &(f_start, f_end, ref err) in &forbidden_spans {
                    if start_byte < f_end && end_byte > f_start {
                        return Err(err.clone());
                    }
                }

                let mut hasher = Sha256::new();
                hasher.update(norm_claimed.as_bytes());
                let quote_hash = format!("{:x}", hasher.finalize());

                return Ok(Self {
                    text: norm_claimed.clone(),
                    start: line_offset + line_pos.min(line_len),
                    end: (line_offset + line_pos + norm_claimed.len()).min(line_offset + line_len),
                    quote_hash,
                    basis_hash: basis_hash.to_string(),
                });
            }

            line_offset += line_len + 1;
        }

        // Deterministic Sentence-Bounded Quote Snap
        // Over non-blockquote, non-code-fence candidate sentences
        let candidate_sentences = collect_candidate_sentences(artifact_body, &forbidden_spans);
        let claimed_words = tokenize_words(&norm_claimed);

        if !claimed_words.is_empty() {
            let mut matching_candidates: Vec<&CandidateSentence> = Vec::new();

            for cand in &candidate_sentences {
                let cand_words = tokenize_words(cand.text);

                // Condition (a): first 3 and last 3 normalized words appear in order
                let match_a = if claimed_words.len() >= 6 {
                    let first_3 = &claimed_words[0..3];
                    let last_3 = &claimed_words[claimed_words.len() - 3..];
                    let mut check_seq = Vec::with_capacity(6);
                    check_seq.extend_from_slice(first_3);
                    check_seq.extend_from_slice(last_3);
                    matches_ordered_subsequence(&cand_words, &check_seq)
                } else {
                    matches_ordered_subsequence(&cand_words, &claimed_words)
                };

                // Condition (b): >= 80% token overlap + critical numeric/negation verbatim preservation
                let match_b = matches_token_overlap_and_critical_tokens(&cand_words, &claimed_words);

                if match_a || match_b {
                    matching_candidates.push(cand);
                }
            }

            if matching_candidates.len() == 1 {
                let winner = matching_candidates[0];

                // If candidate is inside forbidden blockquote or code block, enforce rejection
                if let Some(ref err) = winner.forbidden_err {
                    return Err(err.clone());
                }

                // Snap start and end to the exact verbatim span in artifact_body
                let (snap_start, snap_end) = find_anchor_subspan(
                    artifact_body,
                    winner.start,
                    winner.end,
                    &claimed_words,
                );

                let verbatim_span = &artifact_body[snap_start..snap_end];
                let mut hasher = Sha256::new();
                hasher.update(verbatim_span.as_bytes());
                let quote_hash = format!("{:x}", hasher.finalize());

                return Ok(Self {
                    text: verbatim_span.to_string(),
                    start: snap_start,
                    end: snap_end,
                    quote_hash,
                    basis_hash: basis_hash.to_string(),
                });
            }
        }

        Err(QuoteError::NotFound)
    }
}

#[derive(Debug, Clone)]
struct CandidateSentence<'a> {
    start: usize,
    end: usize,
    text: &'a str,
    forbidden_err: Option<QuoteError>,
}

fn collect_candidate_sentences<'a>(
    artifact_body: &'a str,
    forbidden_spans: &[(usize, usize, QuoteError)],
) -> Vec<CandidateSentence<'a>> {
    let mut sentences = Vec::new();
    let mut byte_offset = 0;

    for line in artifact_body.lines() {
        let line_len = line.len();
        let line_start = byte_offset;
        let line_end = byte_offset + line_len;
        let trimmed = line.trim();

        if !trimmed.is_empty() {
            let mut line_forbidden: Option<QuoteError> = None;
            for &(f_start, f_end, ref err) in forbidden_spans {
                if line_start < f_end && line_end > f_start {
                    line_forbidden = Some(err.clone());
                    break;
                }
            }

            let trimmed_start = line_start + (line.len() - line.trim_start().len());
            let trimmed_end = line_start + line.trim_end().len();

            let mut sent_start = trimmed_start;
            let line_slice = &artifact_body[trimmed_start..trimmed_end];

            for (idx, ch) in line_slice.char_indices() {
                if matches!(ch, '.' | '?' | '!') {
                    let next_char = line_slice[idx + ch.len_utf8()..].chars().next();
                    let prev_is_digit = idx > 0 && line_slice[..idx].chars().last().is_some_and(|c| c.is_ascii_digit());
                    let next_is_digit = next_char.is_some_and(|c| c.is_ascii_digit());
                    if prev_is_digit && next_is_digit {
                        continue;
                    }

                    if next_char.is_none() || next_char.is_some_and(|c| c.is_whitespace()) {
                        let sent_end = trimmed_start + idx + ch.len_utf8();
                        let s_text = &artifact_body[sent_start..sent_end];
                        if !s_text.trim().is_empty() {
                            sentences.push(CandidateSentence {
                                start: sent_start,
                                end: sent_end,
                                text: s_text.trim(),
                                forbidden_err: line_forbidden.clone(),
                            });
                        }
                        let mut rem_start = sent_end;
                        while rem_start < trimmed_end && artifact_body.as_bytes()[rem_start].is_ascii_whitespace() {
                            rem_start += 1;
                        }
                        sent_start = rem_start;
                    }
                }
            }

            if sent_start < trimmed_end {
                let s_text = &artifact_body[sent_start..trimmed_end];
                if !s_text.trim().is_empty() {
                    sentences.push(CandidateSentence {
                        start: sent_start,
                        end: trimmed_end,
                        text: s_text.trim(),
                        forbidden_err: line_forbidden,
                    });
                }
            }
        }

        byte_offset = line_end + 1;
    }

    sentences
}

fn tokenize_words(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_alphanumeric() && c != '_')
                .to_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

fn matches_ordered_subsequence(haystack: &[String], needle: &[String]) -> bool {
    if needle.is_empty() {
        return false;
    }
    let mut h_idx = 0;
    for n in needle {
        let mut found = false;
        while h_idx < haystack.len() {
            if &haystack[h_idx] == n {
                found = true;
                h_idx += 1;
                break;
            }
            h_idx += 1;
        }
        if !found {
            return false;
        }
    }
    true
}

const NEGATION_TOKENS: &[&str] = &[
    "not", "no", "never", "neither", "nor", "none", "without", "cannot", "n't", "dont", "cant", "wont",
];

fn is_numeric_or_version_token(token: &str) -> bool {
    token.chars().any(|c| c.is_ascii_digit()) || token.contains('_')
}

fn is_negation_token(token: &str) -> bool {
    let lower = token.to_lowercase();
    NEGATION_TOKENS.contains(&lower.as_str()) || lower.ends_with("n't")
}

fn matches_token_overlap_and_critical_tokens(
    candidate_words: &[String],
    claimed_words: &[String],
) -> bool {
    if claimed_words.is_empty() || candidate_words.is_empty() {
        return false;
    }

    let cand_set: std::collections::HashSet<&String> = candidate_words.iter().collect();
    let overlap_count = claimed_words.iter().filter(|w| cand_set.contains(w)).count();
    let overlap_ratio = overlap_count as f64 / claimed_words.len() as f64;
    if overlap_ratio < 0.80 {
        return false;
    }

    for word in claimed_words {
        if is_numeric_or_version_token(word) && !cand_set.contains(word) {
            return false;
        }
    }

    for word in claimed_words {
        if is_negation_token(word) && !cand_set.contains(word) {
            return false;
        }
    }

    true
}

fn find_anchor_subspan(
    candidate_body: &str,
    cand_start: usize,
    cand_end: usize,
    claimed_words: &[String],
) -> (usize, usize) {
    if claimed_words.is_empty() {
        return (cand_start, cand_end);
    }

    let first_word = &claimed_words[0];
    let last_word = &claimed_words[claimed_words.len() - 1];
    let text = &candidate_body[cand_start..cand_end];
    let text_lower = text.to_lowercase();

    let first_pos = text_lower.find(first_word.as_str());
    let last_pos = text_lower.rfind(last_word.as_str());

    if let (Some(f_idx), Some(l_idx)) = (first_pos, last_pos) {
        if l_idx >= f_idx {
            let start = cand_start + f_idx;
            let end_token = cand_start + l_idx + last_word.len();
            let mut end = end_token.min(cand_end);
            if end < cand_end {
                let rem = &candidate_body[end..cand_end];
                if rem.starts_with('.') || rem.starts_with('?') || rem.starts_with('!') {
                    end += 1;
                }
            }
            if end > start && end - start >= 12 {
                return (start, end);
            }
        }
    }

    (cand_start, cand_end)
}

pub fn derive_provenance(
    authorship: Authorship,
    verified_quote: Result<&VerifiedQuote, &QuoteError>,
) -> EpistemicOrigin {
    match (authorship, verified_quote) {
        (Authorship::AiGenerated, _) => EpistemicOrigin::AiInferred,
        (Authorship::User, Ok(_)) => EpistemicOrigin::UserRecorded,
        (Authorship::External, Ok(_)) => EpistemicOrigin::SourceBacked,
        (_, Err(_)) => EpistemicOrigin::AiInferred,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_quote_match() {
        let body = "Decision: We are locking the target tier to 14B Q4_K_M models.";
        let hash = "hash1";
        let res = VerifiedQuote::verify(body, hash, "We are locking the target tier to 14B Q4_K_M models.");
        assert!(res.is_ok());
        let vq = res.unwrap();
        assert_eq!(vq.text, "We are locking the target tier to 14B Q4_K_M models.");
    }

    #[test]
    fn test_sentence_anchor_snap_on_paraphrase() {
        let body = "Decision: We are locking the target tier to 14B Q4_K_M models because local cache is capped at 64MB.\n\nMaybe llama3.2:latest could handle the router?";
        let hash = "hash1";
        // Claimed omits "Decision: " and slightly trims
        let claimed = "We are locking the target tier to 14B Q4_K_M models because local cache is capped at 64MB";
        let res = VerifiedQuote::verify(body, hash, claimed);
        assert!(res.is_ok());
        let vq = res.unwrap();
        // Snaps to true verbatim span in document
        assert!(vq.text.contains("14B Q4_K_M"));
        assert!(vq.text.contains("64MB"));
    }

    #[test]
    fn test_numeric_token_mismatch_rejected() {
        let body = "Decision: We are locking the target tier to 14B Q4_K_M models because local cache is capped at 64MB.";
        let hash = "hash1";
        // Claimed changes 64MB to 128MB
        let claimed = "We are locking the target tier to 14B Q4_K_M models because local cache is capped at 128MB.";
        let res = VerifiedQuote::verify(body, hash, claimed);
        assert_eq!(res.err(), Some(QuoteError::NotFound));
    }

    #[test]
    fn test_negation_token_mismatch_rejected() {
        let body = "We decided to keep SQLite for local storage.";
        let hash = "hash1";
        // Claimed introduces "not"
        let claimed = "We decided not to keep SQLite for local storage.";
        let res = VerifiedQuote::verify(body, hash, claimed);
        assert_eq!(res.err(), Some(QuoteError::NotFound));
    }

    #[test]
    fn test_blockquote_defense_rejected() {
        let body = "# Notes\n> We decided to abandon SQLite in favor of Postgres.\n\nConclusion: SQLite is fast.";
        let hash = "hash1";
        let claimed = "We decided to abandon SQLite in favor of Postgres.";
        let res = VerifiedQuote::verify(body, hash, claimed);
        assert_eq!(res.err(), Some(QuoteError::InsideBlockquote));
    }
}
