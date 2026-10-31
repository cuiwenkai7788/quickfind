use crate::FileEntry;
use fuzzy_matcher::FuzzyMatcher;
use rayon::prelude::*;
use std::collections::HashMap;

/// Search result with score
#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchResult {
    pub file: FileEntry,
    pub score: i64,
    pub highlights: Vec<(usize, usize)>,
}

/// File search engine
pub struct SearchEngine {
    matcher: fuzzy_matcher::skim::SkimMatcherV2,
    /// File type filters
    type_filters: HashMap<String, Vec<String>>,
}

impl SearchEngine {
    pub fn new() -> Self {
        let mut type_filters = HashMap::new();

        // Define file type categories
        type_filters.insert("document".to_string(), vec![
            "pdf".to_string(), "doc".to_string(), "docx".to_string(),
            "xls".to_string(), "xlsx".to_string(), "ppt".to_string(),
            "pptx".to_string(), "txt".to_string(), "md".to_string(),
            "rtf".to_string(), "odt".to_string(),
        ]);

        type_filters.insert("image".to_string(), vec![
            "jpg".to_string(), "jpeg".to_string(), "png".to_string(),
            "gif".to_string(), "bmp".to_string(), "svg".to_string(),
            "webp".to_string(), "ico".to_string(), "tiff".to_string(),
        ]);

        type_filters.insert("video".to_string(), vec![
            "mp4".to_string(), "avi".to_string(), "mkv".to_string(),
            "mov".to_string(), "wmv".to_string(), "flv".to_string(),
            "webm".to_string(), "m4v".to_string(),
        ]);

        type_filters.insert("audio".to_string(), vec![
            "mp3".to_string(), "wav".to_string(), "flac".to_string(),
            "aac".to_string(), "ogg".to_string(), "wma".to_string(),
            "m4a".to_string(),
        ]);

        type_filters.insert("archive".to_string(), vec![
            "zip".to_string(), "rar".to_string(), "7z".to_string(),
            "tar".to_string(), "gz".to_string(), "bz2".to_string(),
        ]);

        type_filters.insert("code".to_string(), vec![
            "rs".to_string(), "js".to_string(), "ts".to_string(),
            "py".to_string(), "java".to_string(), "cpp".to_string(),
            "c".to_string(), "go".to_string(), "rb".to_string(),
            "php".to_string(), "cs".to_string(), "swift".to_string(),
            "kt".to_string(), "sh".to_string(), "bat".to_string(),
        ]);

        Self {
            matcher: fuzzy_matcher::skim::SkimMatcherV2::default(),
            type_filters,
        }
    }

    /// Search files with query
    pub fn search(&self, query: &str, files: &[FileEntry], limit: usize) -> Vec<SearchResult> {
        let query = query.trim();

        if query.is_empty() {
            return Vec::new();
        }

        // Parse query for type filters (e.g., "video:mp4" or "doc:")
        let (type_filter, search_query) = self.parse_query(query);

        // Score and filter files in parallel
        let results: Vec<SearchResult> = files
            .par_iter()
            .filter(|file| {
                // Apply type filter if present
                if let Some(ref types) = type_filter {
                    if !types.is_empty() {
                        return file.ext.as_ref()
                            .map(|ext| types.contains(ext))
                            .unwrap_or(false);
                    }
                }
                true
            })
            .filter_map(|file| {
                self.score_file(file, search_query)
            })
            .collect();

        // Sort by score and limit
        let mut sorted = results;
        sorted.sort_by(|a, b| b.score.cmp(&a.score));
        sorted.truncate(limit);

        sorted
    }

    /// Parse query for type filter and search terms
    fn parse_query(&self, query: &str) -> (Option<Vec<String>>, &str) {
        // Check for type filter syntax: "type:query" or "type:"
        if let Some(colon_pos) = query.find(':') {
            let type_part = &query[..colon_pos];
            let rest = &query[colon_pos + 1..];

            // Check if type_part is a known category
            if let Some(extensions) = self.type_filters.get(type_part) {
                return (Some(extensions.clone()), rest);
            }

            // Check if it's a direct extension filter
            if type_part.len() <= 5 && type_part.chars().all(|c| c.is_ascii_alphabetic()) {
                return (Some(vec![type_part.to_lowercase()]), rest);
            }
        }

        (None, query)
    }

    /// Score a single file against query
    fn score_file(&self, file: &FileEntry, query: &str) -> Option<SearchResult> {
        let query_lower = query.to_lowercase();
        let name_lower = file.name.to_lowercase();

        let mut score = 0i64;
        let mut highlights = Vec::new();

        // Exact name match (highest priority)
        if name_lower == query_lower {
            score = 10000;
            highlights.push((0, file.name.len()));
        }
        // Name starts with query
        else if name_lower.starts_with(&query_lower) {
            score = 5000;
            highlights.push((0, query.len()));
        }
        // Name contains query as word
        else if name_lower.split(|c: char| !c.is_alphanumeric()).any(|word| word == query_lower) {
            score = 3000;
            // Find highlight position
            if let Some(pos) = name_lower.find(&query_lower) {
                highlights.push((pos, pos + query.len()));
            }
        }
        // Fuzzy match
        else if let Some((fuzzy_score, matched_indices)) = self.matcher.fuzzy_indices(&file.name, query) {
            score = fuzzy_score as i64;
            highlights = matched_indices
                .into_iter()
                .map(|i| (i, i + 1))
                .collect();
        }
        // Check path match
        else if file.path.to_lowercase().contains(&query_lower) {
            score = 100;
        }
        else {
            return None;
        }

        // Bonus for files vs directories
        if !file.is_dir {
            score += 50;
        }

        // Bonus for recent files
        if let Some(modified) = file.modified {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let age_days = (now - modified) / 86400;
            if age_days < 1 {
                score += 100;
            } else if age_days < 7 {
                score += 50;
            } else if age_days < 30 {
                score += 20;
            }
        }

        // Convert individual highlights to ranges
        let ranges = self.merge_highlights(&highlights);

        Some(SearchResult {
            file: file.clone(),
            score,
            highlights: ranges,
        })
    }

    /// Merge adjacent highlight indices into ranges
    fn merge_highlights(&self, highlights: &[(usize, usize)]) -> Vec<(usize, usize)> {
        if highlights.is_empty() {
            return Vec::new();
        }

        let mut sorted: Vec<_> = highlights.to_vec();
        sorted.sort();

        let mut merged = Vec::new();
        let mut current = sorted[0];

        for &(start, end) in sorted.iter().skip(1) {
            if start <= current.1 {
                current.1 = current.1.max(end);
            } else {
                merged.push(current);
                current = (start, end);
            }
        }
        merged.push(current);

        merged
    }

    /// Get supported file type categories
    pub fn get_type_categories(&self) -> &HashMap<String, Vec<String>> {
        &self.type_filters
    }

    /// Check if query matches pinyin initials
    #[cfg(feature = "pinyin")]
    fn pinyin_match(&self, name: &str, query: &str) -> Option<i64> {
        // Would integrate with pinyin crate if enabled
        None
    }
}

impl Default for SearchEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_basic() {
        let engine = SearchEngine::new();
        let files = vec![
            FileEntry {
                name: "test.txt".to_string(),
                path: "/home/test.txt".to_string(),
                is_dir: false,
                size: 100,
                modified: None,
                ext: Some("txt".to_string()),
            },
            FileEntry {
                name: "document.pdf".to_string(),
                path: "/home/document.pdf".to_string(),
                is_dir: false,
                size: 1000,
                modified: None,
                ext: Some("pdf".to_string()),
            },
        ];

        let results = engine.search("test", &files, 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].file.name, "test.txt");
    }

    #[test]
    fn test_type_filter() {
        let engine = SearchEngine::new();
        let files = vec![
            FileEntry {
                name: "video.mp4".to_string(),
                path: "/home/video.mp4".to_string(),
                is_dir: false,
                size: 1000000,
                modified: None,
                ext: Some("mp4".to_string()),
            },
            FileEntry {
                name: "document.pdf".to_string(),
                path: "/home/document.pdf".to_string(),
                is_dir: false,
                size: 1000,
                modified: None,
                ext: Some("pdf".to_string()),
            },
        ];

        let results = engine.search("video:", &files, 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].file.name, "video.mp4");
    }
}