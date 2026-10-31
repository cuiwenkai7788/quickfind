use pinyin::to_pinyin_vec;
use std::collections::HashMap;

/// Pinyin match result
#[derive(Debug, Clone)]
pub struct PinyinMatch {
    /// Original text
    pub original: String,
    /// Full pinyin (with tones)
    pub full_pinyin: String,
    /// Pinyin initials (first letters)
    pub initials: String,
    /// Pinyin without tones
    pub plain_pinyin: String,
}

/// Pinyin search index for a string
pub struct PinyinIndex {
    /// Mapping from string to pinyin data
    cache: HashMap<String, PinyinMatch>,
}

impl PinyinIndex {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    /// Convert Chinese text to pinyin
    pub fn convert(&mut self, text: &str) -> PinyinMatch {
        // Check cache
        if let Some(cached) = self.cache.get(text) {
            return cached.clone();
        }

        let result = self.do_convert(text);
        self.cache.insert(text.to_string(), result.clone());
        result
    }

    fn do_convert(&self, text: &str) -> PinyinMatch {
        let mut full_pinyin = String::new();
        let mut initials = String::new();
        let mut plain_pinyin = String::new();

        let pinyin_vec = to_pinyin_vec(text, pinyin::PinyinFormat::WithTone);

        for p in pinyin_vec {
            match p {
                pinyin::PinyinOrOther::Pinyin(py) => {
                    let py_str = py.with_tone();
                    full_pinyin.push_str(&py_str);
                    full_pinyin.push(' ');

                    // Initial
                    let initial = py_str.chars().next().unwrap_or(' ');
                    initials.push(initial);

                    // Plain (without tone)
                    let plain = py.plain();
                    plain_pinyin.push_str(plain);
                    plain_pinyin.push(' ');
                }
                pinyin::PinyinOrOther::Other(c) => {
                    // Non-Chinese character
                    if c.is_alphabetic() {
                        full_pinyin.push(c);
                        initials.push(c.to_ascii_lowercase());
                        plain_pinyin.push(c.to_ascii_lowercase());
                    }
                }
            }
        }

        PinyinMatch {
            original: text.to_string(),
            full_pinyin: full_pinyin.trim().to_string(),
            initials: initials.trim().to_string(),
            plain_pinyin: plain_pinyin.trim().to_string(),
        }
    }

    /// Check if query matches the text (supports pinyin)
    pub fn matches(&mut self, text: &str, query: &str) -> Option<i64> {
        let query_lower = query.to_lowercase();

        // 1. Direct match
        if text.to_lowercase().contains(&query_lower) {
            return Some(100);
        }

        // 2. Convert to pinyin and match
        let pinyin = self.convert(text);

        // Match initials (e.g., "xts" matches "系统设置")
        if pinyin.initials.contains(&query_lower) {
            return Some(80);
        }

        // Match plain pinyin (e.g., "xitong" matches "系统")
        if pinyin.plain_pinyin.to_lowercase().contains(&query_lower) {
            return Some(70);
        }

        // Match full pinyin with tone
        if pinyin.full_pinyin.to_lowercase().contains(&query_lower) {
            return Some(60);
        }

        None
    }

    /// Extract Chinese characters from text
    pub fn extract_chinese(text: &str) -> String {
        text.chars()
            .filter(|c| c >= '\u{4E00}' && c <= '\u{9FFF}')
            .collect()
    }

    /// Check if text contains Chinese characters
    pub fn has_chinese(text: &str) -> bool {
        text.chars().any(|c| c >= '\u{4E00}' && c <= '\u{9FFF}')
    }
}

impl Default for PinyinIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pinyin_conversion() {
        let mut index = PinyinIndex::new();
        let result = index.convert("系统设置");

        assert!(!result.initials.is_empty());
        assert!(result.plain_pinyin.contains("xi"));
    }

    #[test]
    fn test_pinyin_match() {
        let mut index = PinyinIndex::new();

        // Initials match
        assert!(index.matches("系统设置", "xts").is_some());

        // Direct match
        assert!(index.matches("系统设置", "系统").is_some());

        // Plain pinyin match
        assert!(index.matches("系统设置", "xitong").is_some());
    }
}