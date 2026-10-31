mod watcher;

use crate::FileEntry;
use parking_lot::RwLock;
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Instant, SystemTime};
use walkdir::WalkDir;

pub use watcher::FileWatcher;

/// Index status
#[derive(Debug, Clone, serde::Serialize)]
pub struct IndexStatus {
    pub file_count: usize,
    pub dir_count: usize,
    pub last_indexed: Option<u64>,
    pub indexing: bool,
    pub progress: f32,
    pub current_path: String,
}

/// File indexer
pub struct FileIndexer {
    files: Arc<RwLock<Vec<FileEntry>>>,
    name_index: Arc<RwLock<HashMap<String, Vec<usize>>>>,
    status: Arc<RwLock<IndexStatus>>,
    roots: Vec<String>,
}

impl FileIndexer {
    pub fn new() -> Self {
        Self {
            files: Arc::new(RwLock::new(Vec::new())),
            name_index: Arc::new(RwLock::new(HashMap::new())),
            status: Arc::new(RwLock::new(IndexStatus {
                file_count: 0,
                dir_count: 0,
                last_indexed: None,
                indexing: false,
                progress: 0.0,
                current_path: String::new(),
            })),
            roots: Self::get_default_roots(),
        }
    }

    /// Get default root directories based on OS
    fn get_default_roots() -> Vec<String> {
        #[cfg(target_os = "windows")]
        {
            let mut drives = Vec::new();
            for letter in b'A'..=b'Z' {
                let path = format!("{}:\\", letter as char);
                if Path::new(&path).exists() {
                    drives.push(path);
                }
            }
            drives
        }

        #[cfg(not(target_os = "windows"))]
        {
            vec!["/home".to_string(), "/mnt".to_string()]
        }
    }

    /// Set custom root directories
    pub fn set_roots(&mut self, roots: Vec<String>) {
        self.roots = roots.into_iter().filter(|r| Path::new(r).exists()).collect();
    }

    /// Get current roots
    pub fn get_roots(&self) -> &[String] {
        &self.roots
    }

    /// Get index status
    pub fn get_status(&self) -> IndexStatus {
        self.status.read().clone()
    }

    /// Check if indexing is in progress
    pub fn is_indexing(&self) -> bool {
        self.status.read().indexing
    }

    /// Build full file index
    pub fn build_index(&self) -> Result<IndexStatus, String> {
        // Check if already indexing
        {
            let status = self.status.read();
            if status.indexing {
                return Err("索引正在进行中".to_string());
            }
        }

        // Set indexing flag
        {
            let mut status = self.status.write();
            status.indexing = true;
            status.progress = 0.0;
            status.current_path = String::new();
        }

        log::info!("Starting file indexing for {} roots", self.roots.len());

        let start = Instant::now();
        let mut all_files = Vec::new();
        let mut file_count = 0usize;
        let mut dir_count = 0usize;

        // Estimate total items for progress
        let estimated_total: u64 = self.roots.iter().map(|r| {
            WalkDir::new(r).into_iter().count() as u64
        }).sum();

        for root in &self.roots {
            log::info!("Indexing {}...", root);

            let walker = WalkDir::new(root)
                .follow_links(false)
                .into_iter()
                .filter_entry(|e| {
                    let name = e.file_name().to_string_lossy();
                    // Skip hidden and system directories
                    !name.starts_with('.')
                        && !name.starts_with('$')
                        && name != "System Volume Information"
                        && name != "Recycle.Bin"
                        && name != "Windows"
                        && name != "Program Files"
                        && name != "Program Files (x86)"
                        && name != "ProgramData"
                });

            for entry in walker {
                match entry {
                    Ok(entry) => {
                        let path = entry.path();

                        // Update progress
                        {
                            let mut status = self.status.write();
                            status.current_path = path.to_string_lossy().to_string();
                        }

                        let file_name = entry.file_name().to_string_lossy().to_string();
                        let metadata = entry.metadata().ok();

                        let (size, modified, is_dir) = metadata
                            .map(|m| {
                                (
                                    m.len(),
                                    m.modified()
                                        .ok()
                                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                        .map(|d| d.as_secs()),
                                    m.is_dir(),
                                )
                            })
                            .unwrap_or((0, None, false));

                        let entry = FileEntry {
                            name: file_name,
                            path: normalize_path(path),
                            is_dir,
                            size,
                            modified,
                            ext: Path::new(&path)
                                .extension()
                                .map(|e| e.to_string_lossy().to_lowercase()),
                        };

                        all_files.push(entry);

                        if is_dir {
                            dir_count += 1;
                        } else {
                            file_count += 1;
                        }

                        // Update progress periodically
                        if all_files.len() % 1000 == 0 {
                            let mut status = self.status.write();
                            status.file_count = file_count;
                            status.dir_count = dir_count;
                            status.progress = (all_files.len() as f64 / estimated_total as f64).min(0.99) as f32;
                        }
                    }
                    Err(e) => {
                        log::debug!("Skip error: {}", e);
                    }
                }
            }
        }

        // Build name index for faster search
        let name_index = build_name_index(&all_files);

        // Update files and index
        {
            let mut files = self.files.write();
            *files = all_files;
        }

        {
            let mut index = self.name_index.write();
            *index = name_index;
        }

        // Update final status
        let elapsed = start.elapsed();
        log::info!("Indexed {} files and {} dirs in {:?}", file_count, dir_count, elapsed);

        let mut status = self.status.write();
        status.file_count = file_count;
        status.dir_count = dir_count;
        status.last_indexed = Some(
            SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        );
        status.indexing = false;
        status.progress = 1.0;
        status.current_path = String::new();

        Ok(status.clone())
    }

    /// Add or update a single file
    pub fn update_file(&self, path: &Path) {
        let file_name = path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        if file_name.is_empty() || file_name.starts_with('.') || file_name.starts_with('$') {
            return;
        }

        let metadata = fs::metadata(path).ok();
        let (size, modified, is_dir) = metadata
            .map(|m| {
                (
                    m.len(),
                    m.modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs()),
                    m.is_dir(),
                )
            })
            .unwrap_or((0, None, false));

        let entry = FileEntry {
            name: file_name,
            path: normalize_path(path),
            is_dir,
            size,
            modified,
            ext: path.extension().map(|e| e.to_string_lossy().to_lowercase()),
        };

        let mut files = self.files.write();

        // Find and update or add
        if let Some(pos) = files.iter().position(|f| f.path == entry.path) {
            files[pos] = entry;
        } else {
            files.push(entry);
        }
    }

    /// Remove a file from index
    pub fn remove_file(&self, path: &str) {
        let mut files = self.files.write();
        files.retain(|f| f.path != path);
    }

    /// Get all files (for search)
    pub fn get_files(&self) -> Vec<FileEntry> {
        self.files.read().clone()
    }

    /// Get files by name prefix (fast lookup)
    pub fn get_files_by_prefix(&self, prefix: &str) -> Vec<FileEntry> {
        let prefix = prefix.to_lowercase();
        let index = self.name_index.read();
        let files = self.files.read();

        let mut results = Vec::new();

        // Find all entries where key starts with prefix
        for (key, indices) in index.iter() {
            if key.starts_with(&prefix) {
                for &idx in indices {
                    if let Some(file) = files.get(idx) {
                        results.push(file.clone());
                    }
                }
            }
        }

        results
    }
}

/// Normalize path for display
fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Build name index for faster prefix search
fn build_name_index(files: &[FileEntry]) -> HashMap<String, Vec<usize>> {
    let mut index: HashMap<String, Vec<usize>> = HashMap::new();

    for (idx, file) in files.iter().enumerate() {
        // Index by first character of filename
        if let Some(first_char) = file.name.chars().next() {
            let key = first_char.to_lowercase().to_string();
            index.entry(key).or_default().push(idx);
        }

        // Index by extension
        if let Some(ref ext) = file.ext {
            let key = format!(".{}", ext);
            index.entry(key).or_default().push(idx);
        }
    }

    index
}