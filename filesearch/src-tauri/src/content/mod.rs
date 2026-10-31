use grep::searcher::Searcher;
use grep::regex::RegexMatcher;
use std::path::Path;
use std::fs;

/// Content search result
#[derive(Debug, Clone, serde::Serialize)]
pub struct ContentMatch {
    pub file_path: String,
    pub line_number: u64,
    pub line_content: String,
    pub match_start: usize,
    pub match_end: usize,
    pub context_before: Option<String>,
    pub context_after: Option<String>,
}

/// Content search options
#[derive(Debug, Clone)]
pub struct ContentSearchOptions {
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub regex_mode: bool,
    pub max_results: usize,
    pub context_lines: usize,
    pub max_file_size: u64, // bytes
    pub include_patterns: Vec<String>,
    pub exclude_patterns: Vec<String>,
}

impl Default for ContentSearchOptions {
    fn default() -> Self {
        Self {
            case_sensitive: false,
            whole_word: false,
            regex_mode: false,
            max_results: 100,
            context_lines: 2,
            max_file_size: 10 * 1024 * 1024, // 10MB
            include_patterns: vec![
                "*.txt".to_string(),
                "*.md".to_string(),
                "*.json".to_string(),
                "*.js".to_string(),
                "*.ts".to_string(),
                "*.py".to_string(),
                "*.rs".to_string(),
                "*.java".to_string(),
                "*.cpp".to_string(),
                "*.c".to_string(),
                "*.go".to_string(),
                "*.html".to_string(),
                "*.css".to_string(),
                "*.xml".to_string(),
                "*.yaml".to_string(),
                "*.yml".to_string(),
                "*.toml".to_string(),
                "*.sh".to_string(),
                "*.bat".to_string(),
            ],
            exclude_patterns: vec![
                "node_modules".to_string(),
                ".git".to_string(),
                "target".to_string(),
                "dist".to_string(),
                "build".to_string(),
            ],
        }
    }
}

/// Content search engine
pub struct ContentSearcher {
    options: ContentSearchOptions,
}

impl ContentSearcher {
    pub fn new(options: ContentSearchOptions) -> Self {
        Self { options }
    }

    /// Search for content in files
    pub fn search(
        &self,
        pattern: &str,
        files: &[String],
    ) -> Result<Vec<ContentMatch>, String> {
        let mut results = Vec::new();

        // Build regex pattern
        let search_pattern = if self.options.whole_word {
            format!(r"\b{}\b", regex::escape(pattern))
        } else if self.options.regex_mode {
            pattern.to_string()
        } else {
            regex::escape(pattern)
        };

        let matcher = RegexMatcher::new(&search_pattern)
            .map_err(|e| format!("Invalid pattern: {}", e))?;

        for file_path in files {
            // Check file size
            if let Ok(metadata) = fs::metadata(file_path) {
                if metadata.len() > self.options.max_file_size {
                    continue;
                }
            }

            // Check include/exclude patterns
            if !self.should_search_file(file_path) {
                continue;
            }

            // Search in file
            let file_results = self.search_file(file_path, &matcher)?;

            results.extend(file_results);

            if results.len() >= self.options.max_results {
                break;
            }
        }

        Ok(results)
    }

    /// Search in a single file
    fn search_file(
        &self,
        file_path: &str,
        matcher: &RegexMatcher,
    ) -> Result<Vec<ContentMatch>, String> {
        let mut results = Vec::new();
        let mut lines: Vec<(u64, String)> = Vec::new();

        // Read all lines first
        if let Ok(content) = fs::read_to_string(file_path) {
            for (i, line) in content.lines().enumerate() {
                lines.push((i as u64 + 1, line.to_string()));
            }
        } else {
            // Binary file or unreadable
            return Ok(results);
        }

        // Search each line
        for (line_num, line) in &lines {
            if results.len() >= self.options.max_results {
                break;
            }

            // Find all matches in line
            let line_lower = if !self.options.case_sensitive {
                line.to_lowercase()
            } else {
                line.clone()
            };

            let search_term = if !self.options.case_sensitive {
                matcher.pattern().to_lowercase()
            } else {
                matcher.pattern().to_string()
            };

            for (pos, _) in line_lower.match_indices(&search_term) {
                let context_before = if *line_num > 1 && self.options.context_lines > 0 {
                    lines.iter()
                        .find(|(n, _)| *n == line_num - 1)
                        .map(|(_, l)| l.clone())
                } else {
                    None
                };

                let context_after = lines.iter()
                    .find(|(n, _)| *n == line_num + 1)
                    .map(|(_, l)| l.clone());

                results.push(ContentMatch {
                    file_path: file_path.to_string(),
                    line_number: *line_num,
                    line_content: line.clone(),
                    match_start: pos,
                    match_end: pos + search_term.len(),
                    context_before,
                    context_after,
                });
            }
        }

        Ok(results)
    }

    /// Check if file should be searched based on patterns
    fn should_search_file(&self, path: &str) -> bool {
        let path_lower = path.to_lowercase();

        // Check exclude patterns
        for exclude in &self.options.exclude_patterns {
            if path_lower.contains(&exclude.to_lowercase()) {
                return false;
            }
        }

        // Check include patterns
        if self.options.include_patterns.is_empty() {
            return true;
        }

        for include in &self.options.include_patterns {
            if let Some(ext) = include.strip_prefix("*.") {
                if path_lower.ends_with(&format!(".{}", ext.to_lowercase())) {
                    return true;
                }
            }
        }

        false
    }
}

impl Default for ContentSearcher {
    fn default() -> Self {
        Self::new(ContentSearchOptions::default())
    }
}

/// Quick file content preview
pub fn preview_file(path: &str, max_lines: usize) -> Result<String, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    let lines: Vec<&str> = content.lines().take(max_lines).collect();

    if lines.len() < max_lines {
        Ok(content)
    } else {
        Ok(format!("{}\n... (truncated)", lines.join("\n")))
    }
}

/// Detect file encoding (basic)
pub fn detect_encoding(data: &[u8]) -> &'static str {
    // UTF-8 BOM
    if data.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return "UTF-8-BOM";
    }

    // UTF-16 LE
    if data.starts_with(&[0xFF, 0xFE]) {
        return "UTF-16-LE";
    }

    // UTF-16 BE
    if data.starts_with(&[0xFE, 0xFF]) {
        return "UTF-16-BE";
    }

    // Try UTF-8
    if std::str::from_utf8(data).is_ok() {
        return "UTF-8";
    }

    // Fallback
    "Unknown"
}