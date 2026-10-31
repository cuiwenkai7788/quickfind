use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::fs;
use uuid::Uuid;

/// Plugin metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub main: String,
    pub commands: Vec<PluginCommand>,
    pub hooks: Vec<PluginHook>,
    pub enabled: bool,
}

/// Plugin command definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCommand {
    pub id: String,
    pub name: String,
    pub description: String,
    pub shortcut: Option<String>,
    pub icon: Option<String>,
}

/// Plugin hook definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginHook {
    pub event: String,
    pub action: String,
}

/// Plugin instance
pub struct Plugin {
    pub manifest: PluginManifest,
    pub path: PathBuf,
    pub state: PluginState,
}

/// Plugin state
#[derive(Debug, Clone, PartialEq)]
pub enum PluginState {
    Loaded,
    Running,
    Error(String),
    Disabled,
}

/// Plugin manager
pub struct PluginManager {
    plugins: HashMap<String, Plugin>,
    plugin_dir: PathBuf,
}

impl PluginManager {
    pub fn new() -> Self {
        let plugin_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("QuickFind")
            .join("plugins");

        Self {
            plugins: HashMap::new(),
            plugin_dir,
        }
    }

    /// Discover all plugins in plugin directory
    pub fn discover(&mut self) -> Result<Vec<String>, String> {
        if !self.plugin_dir.exists() {
            fs::create_dir_all(&self.plugin_dir)
                .map_err(|e| format!("Cannot create plugin dir: {}", e))?;
            return Ok(Vec::new());
        }

        let mut discovered = Vec::new();

        for entry in fs::read_dir(&self.plugin_dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();

            // Look for manifest.json
            let manifest_path = path.join("manifest.json");
            if !manifest_path.exists() {
                continue;
            }

            // Load manifest
            match self.load_manifest(&manifest_path) {
                Ok(manifest) => {
                    let plugin_id = manifest.id.clone();
                    self.plugins.insert(plugin_id.clone(), Plugin {
                        manifest,
                        path: path.clone(),
                        state: PluginState::Loaded,
                    });
                    discovered.push(plugin_id);
                }
                Err(e) => {
                    log::error!("Failed to load plugin at {:?}: {}", path, e);
                }
            }
        }

        Ok(discovered)
    }

    /// Load plugin manifest
    fn load_manifest(&self, path: &PathBuf) -> Result<PluginManifest, String> {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Cannot read manifest: {}", e))?;

        let manifest: PluginManifest = serde_json::from_str(&content)
            .map_err(|e| format!("Invalid manifest: {}", e))?;

        Ok(manifest)
    }

    /// Get all plugins
    pub fn get_all(&self) -> Vec<&Plugin> {
        self.plugins.values().collect()
    }

    /// Get plugin by ID
    pub fn get(&self, id: &str) -> Option<&Plugin> {
        self.plugins.get(id)
    }

    /// Enable/disable plugin
    pub fn toggle(&mut self, id: &str) -> Result<bool, String> {
        let plugin = self.plugins.get_mut(id)
            .ok_or_else(|| format!("Plugin not found: {}", id))?;

        plugin.manifest.enabled = !plugin.manifest.enabled;

        if plugin.manifest.enabled {
            plugin.state = PluginState::Loaded;
        } else {
            plugin.state = PluginState::Disabled;
        }

        Ok(plugin.manifest.enabled)
    }

    /// Install plugin from directory
    pub fn install(&mut self, source_path: &str) -> Result<String, String> {
        let source = PathBuf::from(source_path);

        if !source.exists() {
            return Err("Source directory does not exist".to_string());
        }

        let manifest_path = source.join("manifest.json");
        if !manifest_path.exists() {
            return Err("manifest.json not found".to_string());
        }

        let manifest = self.load_manifest(&manifest_path)?;
        let plugin_id = manifest.id.clone();

        // Create plugin directory
        let dest = self.plugin_dir.join(&plugin_id);
        if dest.exists() {
            return Err("Plugin already installed".to_string());
        }

        // Copy files
        fs::create_dir_all(&dest)
            .map_err(|e| format!("Cannot create plugin dir: {}", e))?;

        fn copy_dir(src: &Path, dst: &Path) -> Result<(), String> {
            for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let ty = entry.file_type().map_err(|e| e.to_string())?;

                if ty.is_dir() {
                    let new_dst = dst.join(entry.file_name());
                    fs::create_dir_all(&new_dst).map_err(|e| e.to_string())?;
                    copy_dir(&entry.path(), &new_dst)?;
                } else {
                    fs::copy(entry.path(), dst.join(entry.file_name()))
                        .map_err(|e| e.to_string())?;
                }
            }
            Ok(())
        }

        copy_dir(&source, &dest)?;

        // Add to plugins
        self.plugins.insert(plugin_id.clone(), Plugin {
            manifest,
            path: dest,
            state: PluginState::Loaded,
        });

        Ok(plugin_id)
    }

    /// Uninstall plugin
    pub fn uninstall(&mut self, id: &str) -> Result<(), String> {
        let plugin = self.plugins.get(id)
            .ok_or_else(|| format!("Plugin not found: {}", id))?;

        // Remove directory
        if plugin.path.exists() {
            fs::remove_dir_all(&plugin.path)
                .map_err(|e| format!("Cannot remove plugin: {}", e))?;
        }

        // Remove from map
        self.plugins.remove(id);

        Ok(())
    }

    /// Create a new plugin template
    pub fn create_template(&self, name: &str) -> Result<PathBuf, String> {
        let plugin_id = name.to_lowercase()
            .replace(' ', "-")
            .replace(|c: char| !c.is_alphanumeric() && c != '-', "");

        let plugin_dir = self.plugin_dir.join(&plugin_id);
        fs::create_dir_all(&plugin_dir)
            .map_err(|e| format!("Cannot create plugin dir: {}", e))?;

        // Create manifest
        let manifest = PluginManifest {
            id: plugin_id.clone(),
            name: name.to_string(),
            version: "0.1.0".to_string(),
            description: format!("{} plugin for QuickFind", name),
            author: "Anonymous".to_string(),
            main: "main.js".to_string(),
            commands: vec![
                PluginCommand {
                    id: format!("{}.hello", plugin_id),
                    name: "Hello".to_string(),
                    description: "Say hello".to_string(),
                    shortcut: None,
                    icon: Some("👋".to_string()),
                }
            ],
            hooks: vec![],
            enabled: true,
        };

        let manifest_json = serde_json::to_string_pretty(&manifest)
            .map_err(|e| format!("Cannot serialize manifest: {}", e))?;

        fs::write(plugin_dir.join("manifest.json"), manifest_json)
            .map_err(|e| format!("Cannot write manifest: {}", e))?;

        // Create main.js template
        let main_js = r#"// QuickFind Plugin: {{name}}

/**
 * Called when a search is performed
 * @param {string} query - Search query
 * @param {Array} results - Search results
 * @returns {Array} Modified results
 */
function onSearch(query, results) {
    return results;
}

/**
 * Called when a file is selected
 * @param {object} file - Selected file
 */
function onFileSelect(file) {
    console.log('File selected:', file.name);
}

/**
 * Called when a command is triggered
 * @param {string} commandId - Command ID
 * @param {object} context - Execution context
 */
function onCommand(commandId, context) {
    if (commandId === '{{id}}.hello') {
        return { message: 'Hello from {{name}}!' };
    }
}

// Export plugin functions
module.exports = {
    onSearch,
    onFileSelect,
    onCommand
};
"#.replace("{{name}}", name)
   .replace("{{id}}", &plugin_id);

        fs::write(plugin_dir.join("main.js"), main_js)
            .map_err(|e| format!("Cannot write main.js: {}", e))?;

        Ok(plugin_dir)
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Built-in commands
pub fn get_builtin_commands() -> Vec<PluginCommand> {
    vec![
        PluginCommand {
            id: "core.open".to_string(),
            name: "Open File".to_string(),
            description: "Open selected file".to_string(),
            shortcut: Some("Enter".to_string()),
            icon: Some("📂".to_string()),
        },
        PluginCommand {
            id: "core.open-folder".to_string(),
            name: "Open in Folder".to_string(),
            description: "Open file location".to_string(),
            shortcut: Some("Ctrl+Enter".to_string()),
            icon: Some("📁".to_string()),
        },
        PluginCommand {
            id: "core.copy-path".to_string(),
            name: "Copy Path".to_string(),
            description: "Copy file path to clipboard".to_string(),
            shortcut: Some("Ctrl+C".to_string()),
            icon: Some("📋".to_string()),
        },
        PluginCommand {
            id: "core.delete".to_string(),
            name: "Delete".to_string(),
            description: "Move file to trash".to_string(),
            shortcut: Some("Delete".to_string()),
            icon: Some("🗑️".to_string()),
        },
        PluginCommand {
            id: "core.rename".to_string(),
            name: "Rename".to_string(),
            description: "Rename file".to_string(),
            shortcut: Some("F2".to_string()),
            icon: Some("✏️".to_string()),
        },
        PluginCommand {
            id: "core.properties".to_string(),
            name: "Properties".to_string(),
            description: "Show file properties".to_string(),
            shortcut: Some("Alt+Enter".to_string()),
            icon: Some("ℹ️".to_string()),
        },
    ]
}