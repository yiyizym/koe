/// Aggregates ASR interim and final results into a single final transcript.
pub struct TranscriptAggregator {
    interim_text: String,
    final_text: String,
    has_final: bool,
}

impl TranscriptAggregator {
    pub fn new() -> Self {
        Self {
            interim_text: String::new(),
            final_text: String::new(),
            has_final: false,
        }
    }

    /// Update with an interim result (replaces previous interim).
    pub fn update_interim(&mut self, text: &str) {
        if !text.is_empty() {
            self.interim_text = text.to_string();
        }
    }

    /// Update with a final result (appends to final text).
    pub fn update_final(&mut self, text: &str) {
        self.has_final = true;
        if !text.is_empty() {
            if !self.final_text.is_empty() {
                self.final_text.push_str(text);
            } else {
                self.final_text = text.to_string();
            }
        }
    }

    /// Get the best available text.
    /// Returns final text if available, otherwise the last interim text.
    pub fn best_text(&self) -> &str {
        if self.has_final {
            &self.final_text
        } else {
            &self.interim_text
        }
    }

    pub fn has_final_result(&self) -> bool {
        self.has_final
    }

    pub fn has_any_text(&self) -> bool {
        !self.final_text.is_empty() || !self.interim_text.is_empty()
    }
}
