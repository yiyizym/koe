use std::collections::HashSet;

/// Embedded word list (~20K common English words + tech terms).
const WORDLIST: &str = include_str!("wordlist.txt");

/// Configuration for the spell fixer.
pub struct SpellFixConfig {
    /// Minimum word length to attempt correction (default: 3).
    pub min_word_length: usize,
    /// Maximum edit distance as a fraction of word length (default: 0.3).
    pub max_distance_ratio: f32,
    /// Absolute maximum edit distance cap (default: 3).
    pub max_distance_cap: usize,
}

impl Default for SpellFixConfig {
    fn default() -> Self {
        Self {
            min_word_length: 3,
            max_distance_ratio: 0.3,
            max_distance_cap: 3,
        }
    }
}

/// Dictionary-based English spell corrector for mixed Chinese-English ASR output.
pub struct SpellFixer {
    word_set: HashSet<String>,
    word_list: Vec<String>,
    config: SpellFixConfig,
}

impl SpellFixer {
    /// Create a new SpellFixer with only the embedded word list.
    #[allow(dead_code)]
    pub fn new(config: SpellFixConfig) -> Self {
        let words: Vec<String> = WORDLIST
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .map(|l| l.to_string())
            .collect();
        let word_set: HashSet<String> = words.iter().cloned().collect();
        Self {
            word_set,
            word_list: words,
            config,
        }
    }

    /// Create a new SpellFixer that also treats user dictionary entries as known-good words.
    pub fn new_with_user_dict(config: SpellFixConfig, user_dict: &[String]) -> Self {
        let mut words: Vec<String> = WORDLIST
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .map(|l| l.to_string())
            .collect();

        // Add user dictionary entries (lowercased) to the known-good set
        for entry in user_dict {
            let lower = entry.trim().to_lowercase();
            if !lower.is_empty() && lower.chars().all(|c| c.is_ascii_alphabetic()) {
                words.push(lower);
            }
        }

        let word_set: HashSet<String> = words.iter().cloned().collect();
        Self {
            word_set,
            word_list: words,
            config,
        }
    }

    /// Correct English words in mixed Chinese-English text.
    /// Returns the corrected string and the number of corrections made.
    pub fn correct_text(&self, text: &str) -> (String, usize) {
        let tokens = tokenize(text);
        let mut result = String::with_capacity(text.len());
        let mut corrections = 0;

        for token in tokens {
            match token {
                Token::Other(s) => result.push_str(s),
                Token::EnglishWord(word) => {
                    if word.len() < self.config.min_word_length {
                        result.push_str(word);
                        continue;
                    }

                    let lower = word.to_lowercase();
                    if self.word_set.contains(&lower) {
                        result.push_str(word);
                        continue;
                    }

                    if let Some(corrected) = self.find_best_match(&lower) {
                        let cased = apply_case_pattern(word, &corrected);
                        log::debug!("spellfix: \"{}\" -> \"{}\"", word, cased);
                        result.push_str(&cased);
                        corrections += 1;
                    } else {
                        result.push_str(word);
                    }
                }
            }
        }

        (result, corrections)
    }

    /// Find the best dictionary match for a misspelled lowercase word.
    /// When multiple candidates tie on edit distance, prefer the shorter one
    /// (ASR duplication errors always add characters, so the correct word is shorter).
    fn find_best_match(&self, word: &str) -> Option<String> {
        let max_dist = self.max_distance_for(word.len());
        let first_char = word.as_bytes()[0];

        let mut best: Option<&str> = None;
        let mut best_dist = max_dist + 1;
        let mut best_len = usize::MAX;
        let mut ambiguous = false;

        for candidate in &self.word_list {
            // First-letter filter: ASR rarely garbles the first character
            if candidate.as_bytes()[0] != first_char {
                continue;
            }
            // Length filter: skip candidates too different in length
            let len_diff = word.len().abs_diff(candidate.len());
            if len_diff > max_dist {
                continue;
            }

            let dist = levenshtein(word, candidate, max_dist);
            if dist < best_dist || (dist == best_dist && candidate.len() < best_len) {
                best_dist = dist;
                best_len = candidate.len();
                best = Some(candidate);
                ambiguous = false;
            } else if dist == best_dist
                && candidate.len() == best_len
                && best.is_some()
                && best.unwrap() != candidate.as_str()
            {
                ambiguous = true;
            }
        }

        if ambiguous || best_dist > max_dist {
            None
        } else {
            best.map(|s| s.to_string())
        }
    }

    /// Compute the maximum allowed edit distance for a word of the given length.
    fn max_distance_for(&self, word_len: usize) -> usize {
        let by_ratio = (word_len as f32 * self.config.max_distance_ratio) as usize;
        let at_least_one = by_ratio.max(1);
        at_least_one.min(self.config.max_distance_cap)
    }
}

// ─── Tokenizer ──────────────────────────────────────────────────────

/// Token types in mixed-language text.
enum Token<'a> {
    /// Consecutive ASCII alphabetic characters.
    EnglishWord(&'a str),
    /// Everything else (CJK, digits, punctuation, whitespace).
    Other(&'a str),
}

/// Tokenize mixed Chinese-English text into alternating English/non-English segments.
fn tokenize(text: &str) -> Vec<Token<'_>> {
    let mut tokens = Vec::new();
    let mut chars = text.char_indices().peekable();
    let mut seg_start = 0;
    let mut in_english = false;

    while let Some(&(idx, ch)) = chars.peek() {
        let is_alpha = ch.is_ascii_alphabetic();
        if is_alpha != in_english {
            if idx > seg_start {
                let slice = &text[seg_start..idx];
                tokens.push(if in_english {
                    Token::EnglishWord(slice)
                } else {
                    Token::Other(slice)
                });
            }
            seg_start = idx;
            in_english = is_alpha;
        }
        chars.next();
    }
    // Final segment
    if seg_start < text.len() {
        let slice = &text[seg_start..];
        tokens.push(if in_english {
            Token::EnglishWord(slice)
        } else {
            Token::Other(slice)
        });
    }
    tokens
}

// ─── Levenshtein Distance ───────────────────────────────────────────

/// Compute Levenshtein edit distance between two ASCII strings.
/// Returns early if the distance exceeds `max` to save computation.
fn levenshtein(a: &str, b: &str, max: usize) -> usize {
    let a_len = a.len();
    let b_len = b.len();

    if a_len.abs_diff(b_len) > max {
        return max + 1;
    }
    if a == b {
        return 0;
    }

    // Single-row DP
    let mut prev_row: Vec<usize> = (0..=b_len).collect();

    for i in 1..=a_len {
        let mut prev_diag = prev_row[0];
        prev_row[0] = i;
        let a_byte = a.as_bytes()[i - 1];
        let mut row_min = prev_row[0];

        for j in 1..=b_len {
            let old_diag = prev_diag;
            prev_diag = prev_row[j];

            let cost = if a_byte == b.as_bytes()[j - 1] { 0 } else { 1 };
            prev_row[j] = (old_diag + cost)
                .min(prev_row[j] + 1)
                .min(prev_row[j - 1] + 1);

            row_min = row_min.min(prev_row[j]);
        }

        if row_min > max {
            return max + 1;
        }
    }

    prev_row[b_len]
}

// ─── Case Preservation ──────────────────────────────────────────────

/// Apply the casing pattern of `original` to `corrected`.
fn apply_case_pattern(original: &str, corrected: &str) -> String {
    if original.chars().all(|c| c.is_ascii_uppercase()) {
        // ALL CAPS -> ALL CAPS
        corrected.to_uppercase()
    } else if original
        .chars()
        .next()
        .map_or(false, |c| c.is_ascii_uppercase())
        && original.chars().skip(1).all(|c| c.is_ascii_lowercase())
    {
        // Title Case -> Title Case
        let mut chars = corrected.chars();
        match chars.next() {
            Some(first) => {
                let upper: String = first.to_uppercase().collect();
                format!("{}{}", upper, chars.as_str())
            }
            None => corrected.to_string(),
        }
    } else {
        // lowercase or mixed -> keep dictionary form (lowercase)
        corrected.to_string()
    }
}

// ─── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_levenshtein_identical() {
        assert_eq!(levenshtein("hello", "hello", 5), 0);
    }

    #[test]
    fn test_levenshtein_insertion() {
        assert_eq!(levenshtein("settings", "settingss", 5), 1);
    }

    #[test]
    fn test_levenshtein_doubled_syllable() {
        assert_eq!(levenshtein("profile", "profilele", 5), 2);
    }

    #[test]
    fn test_levenshtein_early_termination() {
        // "abc" vs "xyz" has distance 3, but max is 1 so returns > 1
        assert!(levenshtein("abc", "xyz", 1) > 1);
    }

    #[test]
    fn test_correct_doubled_letter() {
        let fixer = SpellFixer::new(SpellFixConfig::default());
        let (result, count) = fixer.correct_text("check your settingss");
        assert_eq!(result, "check your settings");
        assert_eq!(count, 1);
    }

    #[test]
    fn test_correct_doubled_syllable() {
        let fixer = SpellFixer::new(SpellFixConfig::default());
        let (result, count) = fixer.correct_text("update your profilele");
        assert_eq!(result, "update your profile");
        assert_eq!(count, 1);
    }

    #[test]
    fn test_valid_words_unchanged() {
        let fixer = SpellFixer::new(SpellFixConfig::default());
        let (result, count) = fixer.correct_text("hello world");
        assert_eq!(result, "hello world");
        assert_eq!(count, 0);
    }

    #[test]
    fn test_skip_short_words() {
        let fixer = SpellFixer::new(SpellFixConfig::default());
        let (result, count) = fixer.correct_text("I am ok");
        assert_eq!(result, "I am ok");
        assert_eq!(count, 0);
    }

    #[test]
    fn test_mixed_chinese_english() {
        let fixer = SpellFixer::new(SpellFixConfig::default());
        let (result, count) = fixer.correct_text("\u{6253}\u{5f00}settingss\u{9875}\u{9762}");
        assert_eq!(result, "\u{6253}\u{5f00}settings\u{9875}\u{9762}");
        assert_eq!(count, 1);
    }

    #[test]
    fn test_preserve_case_upper() {
        assert_eq!(apply_case_pattern("HELLO", "world"), "WORLD");
    }

    #[test]
    fn test_preserve_case_title() {
        assert_eq!(apply_case_pattern("Hello", "world"), "World");
    }

    #[test]
    fn test_preserve_case_lower() {
        assert_eq!(apply_case_pattern("hello", "world"), "world");
    }

    #[test]
    fn test_user_dictionary_words_preserved() {
        let user_dict = vec!["kubectl".to_string(), "nginx".to_string()];
        let fixer = SpellFixer::new_with_user_dict(SpellFixConfig::default(), &user_dict);
        let (result, count) = fixer.correct_text("run kubectl on nginx");
        assert_eq!(result, "run kubectl on nginx");
        assert_eq!(count, 0);
    }
}
