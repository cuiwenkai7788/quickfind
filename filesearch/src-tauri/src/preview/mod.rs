use syntect::easy::HighlightLines;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::{as_24_bit_terminal_escaped, LinesWithEndings};
use std::path::Path;

/// File preview options
#[derive(Debug, Clone)]
pub struct PreviewOptions {
    pub max_lines: usize,
    pub max_size: u64,
    pub show_line_numbers: bool,
    pub highlight_syntax: bool,
    pub theme: String,
}

impl Default for PreviewOptions {
    fn default() -> Self {
        Self {
            max_lines: 100,
            max_size: 5 * 1024 * 1024, // 5MB
            show_line_numbers: true,
            highlight_syntax: true,
            theme: "base16-ocean.dark".to_string(),
        }
    }
}

/// File previewer
pub struct FilePreviewer {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
    options: PreviewOptions,
}

impl FilePreviewer {
    pub fn new(options: PreviewOptions) -> Self {
        Self {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
            options,
        }
    }

    /// Generate preview for a file
    pub fn preview(&self, path: &str) -> Result<FilePreview, String> {
        let file_path = Path::new(path);

        // Check file size
        let metadata = std::fs::metadata(file_path)
            .map_err(|e| format!("Cannot read file: {}", e))?;

        if metadata.len() > self.options.max_size {
            return Ok(FilePreview {
                path: path.to_string(),
                content_type: PreviewContentType::TooLarge {
                    size: metadata.len(),
                    limit: self.options.max_size,
                },
                language: None,
                line_count: None,
            });
        }

        // Detect file type
        let ext = file_path.extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        // Try to read as text
        let content = std::fs::read_to_string(file_path);

        match content {
            Ok(text) => {
                let lines: Vec<&str> = text.lines().collect();
                let line_count = lines.len();

                // Truncate if too many lines
                let display_lines: Vec<&str> = lines.into_iter()
                    .take(self.options.max_lines)
                    .collect();

                let truncated = line_count > self.options.max_lines;

                // Get syntax
                let syntax = self.syntax_set.find_syntax_for_file(file_path)
                    .map(|s| s.name.clone());

                // Highlight if enabled and syntax found
                if self.options.highlight_syntax {
                    if let Some(highlighted) = self.highlight(&display_lines.join("\n"), &ext) {
                        return Ok(FilePreview {
                            path: path.to_string(),
                            content_type: PreviewContentType::Highlighted {
                                html: highlighted,
                                truncated,
                            },
                            language: syntax,
                            line_count: Some(line_count),
                        });
                    }
                }

                // Plain text
                Ok(FilePreview {
                    path: path.to_string(),
                    content_type: PreviewContentType::Text {
                        content: display_lines.join("\n"),
                        truncated,
                    },
                    language: syntax,
                    line_count: Some(line_count),
                });
            }
            Err(_) => {
                // Binary file - try to detect type
                let bytes = std::fs::read(file_path)
                    .map_err(|e| format!("Cannot read file: {}", e))?;

                let kind = infer::get(&bytes);

                match kind {
                    Some(k) => {
                        Ok(FilePreview {
                            path: path.to_string(),
                            content_type: PreviewContentType::Binary {
                                mime_type: k.mime_type().to_string(),
                                extension: k.extension().to_string(),
                            },
                            language: None,
                            line_count: None,
                        })
                    }
                    None => {
                        Ok(FilePreview {
                            path: path.to_string(),
                            content_type: PreviewContentType::Unknown,
                            language: None,
                            line_count: None,
                        })
                    }
                }
            }
        }
    }

    /// Highlight code with syntax
    fn highlight(&self, code: &str, ext: &str) -> Option<String> {
        let syntax = self.syntax_set.find_syntax_by_extension(ext)?;

        let theme = self.theme_set.themes.get(&self.options.theme)
            .or_else(|| self.theme_set.themes.get("base16-ocean.dark"))?;

        let mut highlighter = HighlightLines::new(syntax, theme);
        let mut html = String::new();

        html.push_str("<pre class=\"code-preview\">");

        for (i, line) in LinesWithEndings::from(code).enumerate() {
            if self.options.show_line_numbers {
                html.push_str(&format!(
                    "<span class=\"line-number\">{}</span>",
                    i + 1
                ));
            }

            let ranges: Vec<_> = highlighter.highlight_line(line, &self.syntax_set).ok()?;

            // Convert to HTML spans
            html.push_str("<span class=\"line-content\">");
            for (style, text) in ranges {
                let color = format!(
                    "#{:02x}{:02x}{:02x}",
                    (style.foreground.r),
                    (style.foreground.g),
                    (style.foreground.b)
                );
                html.push_str(&format!(
                    "<span style=\"color:{}\">{}</span>",
                    color,
                    html_escape(text)
                ));
            }
            html.push_str("</span>\n");
        }

        html.push_str("</pre>");

        Some(html)
    }

    /// Generate thumbnail for image
    #[cfg(feature = "image_preview")]
    pub fn image_thumbnail(&self, path: &str, max_width: u32, max_height: u32) -> Result<String, String> {
        // Read image - using image crate (optional feature)
        use image::imageops::FilterType;
        
        let img = image::open(path)
            .map_err(|e| format!("Cannot open image: {}", e))?;

        // Resize
        let thumbnail = img.resize(max_width, max_height, FilterType::Lanczos3);

        // Encode as base64
        let mut buffer = Vec::new();
        thumbnail.write_to(&mut std::io::Cursor::new(&mut buffer), image::ImageFormat::Png)
            .map_err(|e| format!("Cannot encode image: {}", e))?;

        Ok(format!(
            "data:image/png;base64,{}",
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &buffer)
        ))
    }

    /// Get image info (without thumbnail generation)
    #[cfg(not(feature = "image_preview"))]
    pub fn image_thumbnail(&self, _path: &str, _max_width: u32, _max_height: u32) -> Result<String, String> {
        Err("Image preview disabled".to_string())
    }
}

impl Default for FilePreviewer {
    fn default() -> Self {
        Self::new(PreviewOptions::default())
    }
}

/// Preview content types
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type")]
pub enum PreviewContentType {
    Text {
        content: String,
        truncated: bool,
    },
    Highlighted {
        html: String,
        truncated: bool,
    },
    Binary {
        mime_type: String,
        extension: String,
    },
    TooLarge {
        size: u64,
        limit: u64,
    },
    Unknown,
}

/// File preview result
#[derive(Debug, Clone, serde::Serialize)]
pub struct FilePreview {
    pub path: String,
    pub content_type: PreviewContentType,
    pub language: Option<String>,
    pub line_count: Option<usize>,
}

/// HTML escape helper
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Get common file info
pub fn get_file_info(path: &str) -> FileInfo {
    let file_path = Path::new(path);

    let name = file_path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let extension = file_path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    let parent = file_path.parent()
        .map(|p| p.to_string_lossy().to_string());

    let metadata = std::fs::metadata(file_path).ok();

    let (size, modified, is_dir) = metadata
        .map(|m| {
            (
                m.len(),
                m.modified().ok().and_then(|t| {
                    t.duration_since(std::time::UNIX_EPOCH).ok()
                }).map(|d| d.as_secs()),
                m.is_dir(),
            )
        })
        .unwrap_or((0, None, false));

    let mime = mime_guess::from_path(file_path)
        .first()
        .map(|m| m.to_string());

    FileInfo {
        name,
        path: path.to_string(),
        extension,
        parent,
        size,
        modified,
        is_dir,
        mime_type: mime,
    }
}

/// File information
#[derive(Debug, Clone, serde::Serialize)]
pub struct FileInfo {
    pub name: String,
    pub path: String,
    pub extension: String,
    pub parent: Option<String>,
    pub size: u64,
    pub modified: Option<u64>,
    pub is_dir: bool,
    pub mime_type: Option<String>,
}