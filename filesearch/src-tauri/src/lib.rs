mod config;
mod content;
mod everything;
mod indexer;
mod pinyin;
mod plugins;
mod preview;
mod search;

use config::{AppConfig, SearchHistoryManager};
use content::{ContentMatch, ContentSearcher, ContentSearchOptions};
use everything::EverythingSDK;
use indexer::{FileIndexer, FileWatcher, IndexStatus};
use pinyin::PinyinIndex;
use plugins::{PluginManager, PluginManifest};
use preview::{FilePreview, FilePreviewer, FileInfo};
use search::{SearchEngine, SearchResult};
use parking_lot::RwLock;
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};

/// File entry in the index
#[derive(Debug, Clone, serde::Serialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<u64>,
    pub ext: Option<String>,
}

/// Application state
pub struct AppState {
    indexer: Arc<FileIndexer>,
    searcher: Arc<SearchEngine>,
    pinyin_index: Arc<RwLock<PinyinIndex>>,
    config: Arc<RwLock<AppConfig>>,
    history: Arc<RwLock<SearchHistoryManager>>,
    watcher: Arc<RwLock<Option<FileWatcher>>>,
    content_searcher: Arc<RwLock<ContentSearcher>>,
    previewer: Arc<FilePreviewer>,
    everything: Arc<RwLock<Option<EverythingSDK>>>,
    plugin_manager: Arc<RwLock<PluginManager>>,
}

impl AppState {
    pub fn new() -> Self {
        let config = Arc::new(RwLock::new(AppConfig::load()));

        let indexer = Arc::new(FileIndexer::new());
        let searcher = Arc::new(SearchEngine::new());
        let pinyin_index = Arc::new(RwLock::new(PinyinIndex::new()));
        let history = Arc::new(RwLock::new(SearchHistoryManager::new(100)));
        let watcher = Arc::new(RwLock::new(None));
        let content_searcher = Arc::new(RwLock::new(ContentSearcher::default()));
        let previewer = Arc::new(FilePreviewer::default());
        let everything = Arc::new(RwLock::new(None));
        let plugin_manager = Arc::new(RwLock::new(PluginManager::new()));

        Self {
            indexer,
            searcher,
            pinyin_index,
            config,
            history,
            watcher,
            content_searcher,
            previewer,
            everything,
            plugin_manager,
        }
    }

    pub fn init(&self, app: AppHandle) {
        let config = self.config.read();

        // Initialize plugin manager
        {
            let mut pm = self.plugin_manager.write();
            if let Err(e) = pm.discover() {
                log::warn!("Plugin discovery failed: {}", e);
            }
        }

        // Try to initialize Everything SDK
        {
            let mut everything = self.everything.write();
            let sdk = EverythingSDK::new();
            if sdk.is_everything_running() {
                log::info!("Everything detected and available");
                *everything = Some(sdk);
            }
        }

        // Setup hotkey
        if !config.hotkey.is_empty() {
            let hotkey = config.normalize_hotkey();
            log::info!("Registering hotkey: {}", hotkey);

            #[cfg(not(target_os = "linux"))]
            {
                use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

                if let Ok(shortcut) = hotkey.parse::<Shortcut>() {
                    let app_clone = app.clone();
                    if let Err(e) = app.global_shortcut().on_shortcut(shortcut, move |_app, _shortcut, _event| {
                        if let Some(window) = app_clone.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }) {
                        log::error!("Failed to register hotkey: {}", e);
                    }
                }
            }
        }

        // Start indexing if auto_index enabled
        if config.auto_index {
            let indexer = self.indexer.clone();
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;

                match indexer.build_index() {
                    Ok(status) => {
                        log::info!("Initial indexing complete: {} files", status.file_count);
                    }
                    Err(e) => {
                        log::error!("Initial indexing failed: {}", e);
                    }
                }
            });
        }
    }
}

// ============================================================================
// Tauri Commands - Indexing
// ============================================================================

#[tauri::command]
async fn build_index(state: State<'_, AppState>) -> Result<IndexStatus, String> {
    log::info!("Building file index...");
    let indexer = state.indexer.clone();
    tokio::task::spawn_blocking(move || indexer.build_index())
        .await
        .map_err(|e| format!("Indexing task failed: {}", e))?
}

#[tauri::command]
async fn get_index_status(state: State<'_, AppState>) -> Result<IndexStatus, String> {
    Ok(state.indexer.get_status())
}

// ============================================================================
// Tauri Commands - Search
// ============================================================================

#[tauri::command]
async fn search_files(
    query: String,
    state: State<'_, AppState>,
) -> Result<Vec<SearchResult>, String> {
    let config = state.config.read();
    let max_results = config.max_results;
    drop(config);

    // Check if Everything is available and user prefers it
    let use_everything = {
        let everything = state.everything.read();
        everything.is_some()
    };

    if use_everything {
        // Try Everything search first
        let mut everything = state.everything.write();
        if let Some(ref sdk) = *everything {
            if let Ok(results) = sdk.search(&query, max_results as u32) {
                // Convert Everything results to SearchResult
                let converted: Vec<SearchResult> = results.into_iter().map(|e| {
                    SearchResult {
                        file: FileEntry {
                            name: e.name,
                            path: e.full_path,
                            is_dir: e.is_folder,
                            size: e.size,
                            modified: e.modified,
                            ext: e.extension,
                        },
                        score: 100,
                        highlights: vec![],
                    }
                }).collect();
                return Ok(converted);
            }
        }
    }

    // Fallback to built-in search
    let files = state.indexer.get_files();
    let searcher = state.searcher.clone();
    let mut pinyin_index = state.pinyin_index.write();

    let mut results = tokio::task::spawn_blocking(move || {
        let mut res = searcher.search(&query, &files, max_results);

        // Add pinyin matching
        for result in &mut res {
            if let Some(score) = pinyin_index.matches(&result.file.name, &query) {
                result.score += score;
            }
        }

        // Re-sort after pinyin boost
        res.sort_by(|a, b| b.score.cmp(&a.score));
        res
    })
    .await
    .map_err(|e| format!("Search task failed: {}", e))?;

    // Add to history
    state.history.write().add(query, results.len());

    Ok(results)
}

// ============================================================================
// Tauri Commands - Content Search
// ============================================================================

#[tauri::command]
async fn search_content(
    query: String,
    files: Vec<String>,
    state: State<'_, AppState>,
) -> Result<Vec<ContentMatch>, String> {
    let searcher = state.content_searcher.read();

    tokio::task::spawn_blocking(move || {
        searcher.search(&query, &files)
    })
    .await
    .map_err(|e| format!("Content search failed: {}", e))?
}

// ============================================================================
// Tauri Commands - Preview
// ============================================================================

#[tauri::command]
async fn preview_file(
    path: String,
    state: State<'_, AppState>,
) -> Result<FilePreview, String> {
    let previewer = state.previewer.clone();

    tokio::task::spawn_blocking(move || {
        previewer.preview(&path)
    })
    .await
    .map_err(|e| format!("Preview failed: {}", e))?
}

#[tauri::command]
async fn get_file_info(path: String) -> Result<FileInfo, String> {
    Ok(preview::get_file_info(&path))
}

// ============================================================================
// Tauri Commands - File Operations
// ============================================================================

#[tauri::command]
async fn open_file(path: String) -> Result<(), String> {
    log::info!("Opening file: {}", path);

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &path])
            .spawn()
            .map_err(|e| format!("无法打开文件: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("无法打开文件: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("无法打开文件: {}", e))?;
    }

    Ok(())
}

#[tauri::command]
async fn open_in_explorer(path: String) -> Result<(), String> {
    log::info!("Opening in explorer: {}", path);

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .args(["/select,", &path])
            .spawn()
            .map_err(|e| format!("无法打开资源管理器: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(["-R", &path])
            .spawn()
            .map_err(|e| format!("无法打开Finder: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(std::path::Path::new(&path).parent().unwrap_or(std::path::Path::new("/")))
            .spawn()
            .map_err(|e| format!("无法打开文件管理器: {}", e))?;
    }

    Ok(())
}

#[tauri::command]
async fn copy_path(path: String, app: AppHandle) -> Result<(), String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;

    app.clipboard()
        .write_text(&path)
        .map_err(|e| format!("复制失败: {}", e))?;

    log::info!("Copied path: {}", path);
    Ok(())
}

#[tauri::command]
async fn delete_file(path: String) -> Result<(), String> {
    // Use trash crate for safe deletion
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;

        Command::new("cmd")
            .args(["/C", "del", &path])
            .spawn()
            .map_err(|e| format!("删除失败: {}", e))?;
    }

    log::info!("Deleted file: {}", path);
    Ok(())
}

#[tauri::command]
async fn rename_file(old_path: String, new_name: String) -> Result<String, String> {
    let path = std::path::Path::new(&old_path);
    let new_path = path.parent()
        .map(|p| p.join(&new_name))
        .ok_or_else(|| "Invalid path".to_string())?;

    std::fs::rename(path, &new_path)
        .map_err(|e| format!("重命名失败: {}", e))?;

    Ok(new_path.to_string_lossy().to_string())
}

// ============================================================================
// Tauri Commands - History
// ============================================================================

#[tauri::command]
async fn get_search_history(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let history = state.history.read();
    Ok(history.get_recent(20).into_iter().map(String::from).collect())
}

#[tauri::command]
async fn clear_search_history(state: State<'_, AppState>) -> Result<(), String> {
    state.history.write().clear();
    Ok(())
}

// ============================================================================
// Tauri Commands - Configuration
// ============================================================================

#[tauri::command]
async fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    Ok(state.config.read().clone())
}

#[tauri::command]
async fn update_config(
    config: AppConfig,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    config.save().map_err(|e| format!("保存配置失败: {}", e))?;

    {
        let mut current = state.config.write();
        *current = config.clone();
    }

    log::info!("Configuration updated");
    Ok(config)
}

// ============================================================================
// Tauri Commands - Plugins
// ============================================================================

#[tauri::command]
async fn get_plugins(state: State<'_, AppState>) -> Result<Vec<PluginManifest>, String> {
    let pm = state.plugin_manager.read();
    Ok(pm.get_all().iter().map(|p| p.manifest.clone()).collect())
}

#[tauri::command]
async fn toggle_plugin(id: String, state: State<'_, AppState>) -> Result<bool, String> {
    let mut pm = state.plugin_manager.write();
    pm.toggle(&id)
}

#[tauri::command]
async fn create_plugin(name: String, state: State<'_, AppState>) -> Result<String, String> {
    let pm = state.plugin_manager.read();
    let path = pm.create_template(&name)?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
async fn install_plugin(source: String, state: State<'_, AppState>) -> Result<String, String> {
    let mut pm = state.plugin_manager.write();
    pm.install(&source)
}

#[tauri::command]
async fn uninstall_plugin(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut pm = state.plugin_manager.write();
    pm.uninstall(&id)
}

// ============================================================================
// Tauri Commands - Everything
// ============================================================================

#[tauri::command]
async fn check_everything(state: State<'_, AppState>) -> Result<bool, String> {
    let everything = state.everything.read();
    Ok(everything.as_ref().map(|e| e.is_everything_running()).unwrap_or(false))
}

#[tauri::command]
async fn get_everything_version(state: State<'_, AppState>) -> Result<String, String> {
    let everything = state.everything.read();
    if let Some(ref sdk) = *everything {
        sdk.get_version()
    } else {
        Err("Everything not available".to_string())
    }
}

// ============================================================================
// Tauri Commands - Watching
// ============================================================================

#[tauri::command]
async fn start_watching(state: State<'_, AppState>) -> Result<(), String> {
    let config = state.config.read();
    if !config.watch_changes {
        return Err("实时监听未启用".to_string());
    }

    let roots = config.index_roots.clone();
    drop(config);

    let indexer = state.indexer.clone();
    let mut watcher = FileWatcher::new();

    watcher.start(&roots, move |change| {
        use indexer::FileChange;

        match change {
            FileChange::Created(path) | FileChange::Modified(path) => {
                indexer.update_file(std::path::Path::new(&path));
            }
            FileChange::Removed(path) => {
                indexer.remove_file(&path);
            }
            FileChange::Renamed { old_path, new_path } => {
                indexer.remove_file(&old_path);
                indexer.update_file(std::path::Path::new(&new_path));
            }
        }
    }).map_err(|e| e.to_string())?;

    *state.watcher.write() = Some(watcher);

    log::info!("Started file watching");
    Ok(())
}

#[tauri::command]
async fn stop_watching(state: State<'_, AppState>) -> Result<(), String> {
    if let Some(mut watcher) = state.watcher.write().take() {
        watcher.stop();
        log::info!("Stopped file watching");
    }
    Ok(())
}

// ============================================================================
// Entry Point
// ============================================================================

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    let state = AppState::new();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            build_index,
            get_index_status,
            search_files,
            search_content,
            preview_file,
            get_file_info,
            open_file,
            open_in_explorer,
            copy_path,
            delete_file,
            rename_file,
            get_search_history,
            clear_search_history,
            get_config,
            update_config,
            get_plugins,
            toggle_plugin,
            create_plugin,
            install_plugin,
            uninstall_plugin,
            check_everything,
            get_everything_version,
            start_watching,
            stop_watching,
        ])
        .setup(|app| {
            let state = app.state::<AppState>();
            let app_handle = app.handle().clone();
            state.init(app_handle);
            log::info!("QuickFind started successfully");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}