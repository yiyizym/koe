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
    /// Maximum edit distance ratio for hotword/dictionary matching (default: 0.4).
    pub hotword_max_distance_ratio: f32,
    /// Absolute maximum edit distance cap for hotword/dictionary matching (default: 5).
    pub hotword_max_distance_cap: usize,
}

impl Default for SpellFixConfig {
    fn default() -> Self {
        Self {
            min_word_length: 3,
            max_distance_ratio: 0.3,
            max_distance_cap: 3,
            hotword_max_distance_ratio: 0.4,
            hotword_max_distance_cap: 5,
        }
    }
}

/// Dictionary-based English spell corrector for mixed Chinese-English ASR output.
pub struct SpellFixer {
    word_set: HashSet<String>,
    word_list: Vec<String>,
    /// Hotword entries from user dictionary: (lowercase, original_casing).
    /// These are matched with more lenient distance and preserve dictionary casing.
    hotwords: Vec<(String, String)>,
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
            hotwords: Vec::new(),
            config,
        }
    }

    /// Create a new SpellFixer that also treats user dictionary entries as known-good words
    /// and uses them as priority hotword correction targets with original casing preserved.
    pub fn new_with_user_dict(config: SpellFixConfig, user_dict: &[String]) -> Self {
        let mut words: Vec<String> = WORDLIST
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .map(|l| l.to_string())
            .collect();

        let mut hotwords: Vec<(String, String)> = Vec::new();

        // Add user dictionary entries to the known-good set and hotword list
        for entry in user_dict {
            let trimmed = entry.trim();
            let lower = trimmed.to_lowercase();
            if !lower.is_empty() && lower.chars().all(|c| c.is_ascii_alphabetic()) {
                words.push(lower.clone());
                hotwords.push((lower, trimmed.to_string()));
            }
        }

        let word_set: HashSet<String> = words.iter().cloned().collect();
        Self {
            word_set,
            word_list: words,
            hotwords,
            config,
        }
    }

    /// Correct English words in mixed Chinese-English text.
    /// Returns the corrected string and the number of corrections made.
    ///
    /// For each unknown word, hotword/dictionary entries are tried first with
    /// more lenient distance thresholds. If a hotword matches, its original
    /// casing from the dictionary is used (e.g., "Kubernetes", "PostgreSQL").
    /// Otherwise, falls back to general word list matching.
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

                    // Try hotword/dictionary match first (lenient, preserves dictionary casing)
                    if let Some(hotword) = self.find_hotword_match(&lower) {
                        log::debug!("spellfix(hotword): \"{}\" -> \"{}\"", word, hotword);
                        result.push_str(&hotword);
                        corrections += 1;
                    } else if let Some(corrected) = self.find_best_match(&lower) {
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

    /// Find the best hotword/dictionary match for a misspelled lowercase word.
    /// Uses more lenient distance thresholds than general matching.
    /// Returns the original-cased form from the dictionary (e.g., "Kubernetes").
    fn find_hotword_match(&self, word: &str) -> Option<String> {
        if self.hotwords.is_empty() {
            return None;
        }

        let max_dist = self.hotword_max_distance_for(word.len());
        let first_char = word.as_bytes()[0];

        let mut best: Option<&str> = None;
        let mut best_dist = max_dist + 1;
        let mut best_len = usize::MAX;

        for (lower, _original) in &self.hotwords {
            if lower.as_bytes()[0] != first_char {
                continue;
            }
            let len_diff = word.len().abs_diff(lower.len());
            if len_diff > max_dist {
                continue;
            }

            let dist = levenshtein(word, lower, max_dist);
            if dist < best_dist || (dist == best_dist && lower.len() < best_len) {
                best_dist = dist;
                best_len = lower.len();
                best = Some(lower.as_str());
            }
        }

        if best_dist > max_dist {
            return None;
        }

        // Return the original-cased form for the best match
        best.and_then(|matched| {
            self.hotwords
                .iter()
                .find(|(l, _)| l == matched)
                .map(|(_, original)| original.clone())
        })
    }

    /// Compute the maximum allowed edit distance for hotword matching.
    fn hotword_max_distance_for(&self, word_len: usize) -> usize {
        let by_ratio = (word_len as f32 * self.config.hotword_max_distance_ratio) as usize;
        let at_least_one = by_ratio.max(1);
        at_least_one.min(self.config.hotword_max_distance_cap)
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

    #[test]
    fn test_hotword_match_preserves_dictionary_casing() {
        let user_dict = vec!["Kubernetes".to_string(), "PostgreSQL".to_string()];
        let fixer = SpellFixer::new_with_user_dict(SpellFixConfig::default(), &user_dict);
        // ASR garbles "Kubernetes" -> "kubernatis" (edit distance 2)
        let (result, count) = fixer.correct_text("deploy to kubernatis");
        assert_eq!(result, "deploy to Kubernetes");
        assert_eq!(count, 1);
    }

    #[test]
    fn test_hotword_match_lenient_distance() {
        let user_dict = vec!["Kubernetes".to_string()];
        let config = SpellFixConfig {
            hotword_max_distance_ratio: 0.4,
            hotword_max_distance_cap: 5,
            ..Default::default()
        };
        let fixer = SpellFixer::new_with_user_dict(config, &user_dict);
        // "kubernetees" has distance 2 from "kubernetes" (extra 'e')
        let (result, count) = fixer.correct_text("kubernetees cluster");
        assert_eq!(result, "Kubernetes cluster");
        assert_eq!(count, 1);
    }

    #[test]
    fn test_hotword_match_mixed_chinese_english() {
        let user_dict = vec!["Kubernetes".to_string()];
        let fixer = SpellFixer::new_with_user_dict(SpellFixConfig::default(), &user_dict);
        let (result, count) = fixer.correct_text("\u{90e8}\u{7f72}\u{5230}kubernatis\u{96c6}\u{7fa4}");
        assert_eq!(result, "\u{90e8}\u{7f72}\u{5230}Kubernetes\u{96c6}\u{7fa4}");
        assert_eq!(count, 1);
    }

    #[test]
    fn test_hotword_exact_match_no_correction() {
        let user_dict = vec!["Kubernetes".to_string()];
        let fixer = SpellFixer::new_with_user_dict(SpellFixConfig::default(), &user_dict);
        // Exact match (lowercase) is in word_set, should be kept as-is
        let (result, count) = fixer.correct_text("kubernetes is great");
        assert_eq!(result, "kubernetes is great");
        assert_eq!(count, 0);
    }

    #[test]
    fn test_hotword_priority_over_general_wordlist() {
        // If a dictionary entry matches, it should be preferred over general words
        let user_dict = vec!["Nginx".to_string()];
        let fixer = SpellFixer::new_with_user_dict(SpellFixConfig::default(), &user_dict);
        // "nginy" is close to "nginx" (distance 2)
        let (result, count) = fixer.correct_text("configure nginy");
        assert_eq!(result, "configure Nginx");
        assert_eq!(count, 1);
    }
}
