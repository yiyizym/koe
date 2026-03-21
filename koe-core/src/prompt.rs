use std::path::Path;

/// Load system prompt from file, or return built-in default.
/// cbindgen:ignore
pub fn load_system_prompt(path: &Path) -> String {
    match std::fs::read_to_string(path) {
        Ok(content) => {
            let trimmed = content.trim();
            if trimmed.is_empty() {
                log::warn!("system prompt file is empty, using built-in default");
                build_default_system_prompt()
            } else {
                log::info!("loaded system prompt from {}", path.display());
                trimmed.to_string()
            }
        }
        Err(e) => {
            log::warn!("failed to load system prompt from {}: {e}, using built-in default", path.display());
            build_default_system_prompt()
        }
    }
}

/// Load user prompt template from file, or return built-in default.
/// The template should contain {{asr_text}} and {{dictionary_entries}} placeholders.
/// cbindgen:ignore
pub fn load_user_prompt_template(path: &Path) -> String {
    match std::fs::read_to_string(path) {
        Ok(content) => {
            let trimmed = content.trim();
            if trimmed.is_empty() {
                log::warn!("user prompt file is empty, using built-in default");
                build_default_user_prompt_template()
            } else {
                log::info!("loaded user prompt template from {}", path.display());
                trimmed.to_string()
            }
        }
        Err(e) => {
            log::warn!("failed to load user prompt from {}: {e}, using built-in default", path.display());
            build_default_user_prompt_template()
        }
    }
}

/// Render the user prompt by replacing placeholders in the template.
/// cbindgen:ignore
pub fn render_user_prompt(template: &str, asr_text: &str, dictionary_entries: &[String]) -> String {
    let dict_str = if dictionary_entries.is_empty() {
        String::from("（无）")
    } else {
        dictionary_entries.join("\n")
    };

    template
        .replace("{{asr_text}}", asr_text)
        .replace("{{dictionary_entries}}", &dict_str)
}

/// Built-in default system prompt.
/// cbindgen:ignore
fn build_default_system_prompt() -> String {
    String::from(
        "You are a speech-to-text post-processor for a software developer. Your task is to apply minimal corrections to ASR output that may contain a mix of Chinese and English, with frequent technical terminology.\n\
         \n\
         Rules:\n\
         1. Preserve the original meaning. Do not expand, summarize, or restyle.\n\
         2. Mixed Chinese-English is intentional. Keep the speaker's language choices as-is. Do not translate Chinese to English or vice versa.\n\
         3. Capitalization: fix English words to their correct casing. This is especially important for technical terms:\n\
         - Programming languages: Python, JavaScript, TypeScript, Rust, Go, Java, C++, Ruby, Swift, Kotlin\n\
         - Brands/services: GitHub, GitLab, Cloudflare, AWS, GCP, Azure, Docker, Kubernetes, Redis, PostgreSQL, MySQL, MongoDB, Nginx, Node.js, Next.js, Vercel, Supabase, Firebase, Terraform, Ansible\n\
         - Protocols/formats: HTTP, HTTPS, SSH, TCP, UDP, DNS, API, REST, GraphQL, gRPC, JSON, YAML, TOML, XML, HTML, CSS, SQL, WebSocket\n\
         - Tools/concepts: CLI, SDK, IDE, CI/CD, DevOps, macOS, iOS, Linux, Ubuntu, npm, pip, cargo, Git, VS Code, Xcode, Vim, Neovim\n\
         - Acronyms: URL, URI, CDN, VPN, LLM, ASR, TTS, OCR, NLP, AI, ML, GPU, CPU, RAM, SSD, IP, OAuth, JWT, CORS\n\
         - Always capitalize the first letter of sentences.\n\
         4. Spacing: insert a half-width space between Chinese and English/numbers (e.g. \"使用Python\" -> \"使用 Python\", \"有3个\" -> \"有 3 个\"). No space between English words and Chinese punctuation.\n\
         5. Punctuation: use Chinese punctuation in Chinese context (，。！？：；) and English punctuation in English context. Do not mix. Use \"……\" instead of \"...\". Chinese sentences must end with punctuation.\n\
         6. Prefer terms, proper nouns, and spellings from the user dictionary when provided. The dictionary takes highest priority.\n\
         7. Remove filler words that carry no semantic meaning, such as 嗯, 啊, 哦, 呃, 这个, 那个, 就是, well, like, you know, um, uh, so basically.\n\
         8. Do not remove words that are clearly names, terms, titles, quoted content, or fixed expressions.\n\
         9. Code-related terms should keep their conventional form: e.g. \"main 函数\" not \"mian 函数\", \"npm install\" not \"NPM install\", \"git push\" not \"Git Push\" (subcommands stay lowercase).\n\
         10. Output only the corrected text. No explanations, no JSON, no quotation marks.",
    )
}

/// Built-in default user prompt template.
/// cbindgen:ignore
fn build_default_user_prompt_template() -> String {
    String::from("ASR transcript:\n{{asr_text}}\n\nUser dictionary:\n{{dictionary_entries}}\n\nOutput the corrected text only.")
}

/// Filter dictionary candidates to reduce prompt size.
/// When dictionary has more than `max_candidates` entries,
/// keep only those with character overlap with the ASR text.
/// cbindgen:ignore
pub fn filter_dictionary_candidates(
    dictionary: &[String],
    asr_text: &str,
    max_candidates: usize,
) -> Vec<String> {
    if dictionary.len() <= max_candidates {
        return dictionary.to_vec();
    }

    let asr_lower = asr_text.to_lowercase();
    let asr_chars: std::collections::HashSet<char> = asr_lower.chars().collect();

    let mut scored: Vec<(usize, &String)> = dictionary
        .iter()
        .map(|entry| {
            let entry_lower = entry.to_lowercase();
            let overlap = entry_lower
                .chars()
                .filter(|c| asr_chars.contains(c))
                .count();
            let substring_bonus = if asr_lower.contains(&entry_lower)
                || entry_lower.contains(&asr_lower)
            {
                entry.len() * 10
            } else {
                0
            };
            (overlap + substring_bonus, entry)
        })
        .collect();

    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored
        .into_iter()
        .take(max_candidates)
        .map(|(_, entry)| entry.clone())
        .collect()
}
