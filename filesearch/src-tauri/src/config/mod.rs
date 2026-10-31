use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Search index roots
    pub index_roots: Vec<String>,
    /// Ignored directories
    pub ignored_dirs: Vec<String>,
    /// Ignored file patterns
    pub ignored_patterns: Vec<String>,
    /// Global hotkey to show/hide window
    pub hotkey: String,
    /// Maximum results to show
    pub max_results: usize,
    /// Auto-index on startup
    pub auto_index: bool,
    /// Index update interval (seconds)
    pub index_interval: u64,
    /// Enable real-time watching
    pub watch_changes: bool,
    /// Window always on top
    pub always_on_top: bool,
    /// Theme: "dark" or "light"
    pub theme: String,
    /// Custom file type associations
    pub file_types: HashMap<String, Vec<String>>,
}

impl Default for AppConfig {
    fn default() -> Self {
        let mut file_types = HashMap::new();

        // Default file type categories
        file_types.insert("document".to_string(), vec![
            "pdf".to_string(), "doc".to_string(), "docx".to_string(),
            "xls".to_string(), "xlsx".to_string(), "txt".to_string(),
        ]);
        file_types.insert("image".to_string(), vec![
            "jpg".to_string(), "png".to_string(), "gif".to_string(), "svg".to_string(),
        ]);
        file_types.insert("video".to_string(), vec![
            "mp4".to_string(), "avi".to_string(), "mkv".to_string(), "mov".to_string(),
        ]);
        file_types.insert("audio".to_string(), vec![
            "mp3".to_string(), "wav".to_string(), "flac".to_string(),
        ]);

        Self {
            index_roots: get_default_roots(),
            ignored_dirs: vec![
                "Windows".to_string(),
                "Program Files".to_string(),
                "Program Files (x86)".to_string(),
                "ProgramData".to_string(),
                "$Recycle.Bin".to_string(),
                "System Volume Information".to_string(),
            ],
            ignored_patterns: vec![
                "*.tmp".to_string(),
                "*.temp".to_string(),
                "*.log".to_string(),
                "*.bak".to_string(),
            ],
            hotkey: "Ctrl+Shift+F".to_string(),
            max_results: 100,
            auto_index: true,
            index_interval: 300, // 5 minutes
            watch_changes: true,
            always_on_top: true,
            theme: "dark".to_string(),
            file_types,
        }
    }
}

impl AppConfig {
    /// Load config from file
    pub fn load() -> Self {
        let config_path = Self::get_config_path();

        if let Ok(content) = fs::read_to_string(&config_path) {
            match serde_json::from_str(&content) {
                Ok(config) => {
                    log::info!("Loaded config from {:?}", config_path);
                    return config;
                }
                Err(e) => {
                    log::warn!("Failed to parse config: {}, using defaults", e);
                }
            }
        }

        Self::default()
    }

    /// Save config to file
    pub fn save(&self) -> Result<(), String> {
        let config_path = Self::get_config_path();

        // Ensure parent directory exists
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config dir: {}", e))?;
        }

        let content = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;

        fs::write(&config_path, content)
            .map_err(|e| format!("Failed to write config: {}", e))?;

        log::info!("Saved config to {:?}", config_path);
        Ok(())
    }

    /// Get config file path
    fn get_config_path() -> PathBuf {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("QuickFind");

        config_dir.join("config.json")
    }

    /// Validate and normalize hotkey string
    pub fn normalize_hotkey(&self) -> String {
        // Convert to Tauri format
        self.hotkey
            .replace("+", "+")
            .replace("Ctrl", "CommandOrControl")
            .replace("cmd", "CommandOrControl")
    }

    /// Check if path should be ignored
    pub fn should_ignore(&self, path: &str) -> bool {
        let path_lower = path.to_lowercase();

        // Check ignored directories
        for dir in &self.ignored_dirs {
            if path_lower.contains(&dir.to_lowercase()) {
                return true;
            }
        }

        // Check ignored patterns (simple glob matching)
        for pattern in &self.ignored_patterns {
            if let Some(ext) = pattern.strip_prefix("*.") {
                if path_lower.ends_with(&format!(".{}", ext.to_lowercase())) {
                    return true;
                }
            }
        }

        false
    }
}

/// Get default index roots based on OS
fn get_default_roots() -> Vec<String> {
    #[cfg(target_os = "windows")]
    {
        let mut drives = Vec::new();
        for letter in b'A'..=b'Z' {
            let path = format!("{}:\\", letter as char);
            if std::path::Path::new(&path).exists() {
                drives.push(path);
            }
        }
        drives
    }

    #[cfg(not(target_os = "windows"))]
    {
        vec!["/home".to_string()]
    }
}

/// Search history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHistory {
    pub query: String,
    pub timestamp: u64,
    pub result_count: usize,
}

impl SearchHistory {
    pub fn new(query: String, result_count: usize) -> Self {
        Self {
            query,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            result_count,
        }
    }
}

/// Search history manager
pub struct SearchHistoryManager {
    history: Vec<SearchHistory>,
    max_entries: usize,
}

impl SearchHistoryManager {
    pub fn new(max_entries: usize) -> Self {
        Self {
            history: Vec::new(),
            max_entries,
        }
    }

    /// Add entry to history
    pub fn add(&mut self, query: String, result_count: usize) {
        // Remove duplicate
        self.history.retain(|h| h.query != query);

        // Add new entry
        self.history.push(SearchHistory::new(query, result_count));

        // Limit size
        if self.history.len() > self.max_entries {
            self.history.remove(0);
        }
    }

    /// Get all history
    pub fn get_all(&self) -> &[SearchHistory] {
        &self.history
    }

    /// Get recent queries
    pub fn get_recent(&self, limit: usize) -> Vec<&str> {
        self.history
            .iter()
            .rev()
            .take(limit)
            .map(|h| h.query.as_str())
            .collect()
    }

    /// Clear history
    pub fn clear(&mut self) {
        self.history.clear();
    }
}