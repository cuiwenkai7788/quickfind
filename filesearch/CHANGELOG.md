# Changelog

All notable changes to this project will be documented in this file.

## [0.2.0] - 2024-XX-XX

### Added
- **Pinyin Search** - Support Chinese pinyin search (initials and full pinyin)
- **Content Search** - Search within file contents with context preview
- **File Preview** - Syntax highlighted code preview with line numbers
- **Plugin System** - Custom commands and extensions support
- **Everything Integration** - Optional integration with Everything search engine
- Preview panel with file info
- Content search mode toggle
- Context menu for file actions
- Plugin management UI
- Everything status indicator

### Changed
- Improved search algorithm with pinyin matching
- Enhanced UI with preview panel
- Better keyboard navigation (Tab for preview)
- Optimized index building performance

### Technical
- Added pinyin crate for Chinese character conversion
- Added grep crates for content search
- Added syntect for syntax highlighting
- Added plugin infrastructure
- Added Everything SDK integration module

## [0.1.0] - 2024-XX-XX

### Added
- Initial release
- Fast file search with fuzzy matching
- Real-time file indexing across all drives
- File change monitoring (watcher)
- Global hotkey support (Ctrl+Shift+F)
- File type filtering (document, image, video, audio, archive, code)
- Search syntax with type filters (e.g., `video:mp4`)
- Keyboard navigation (arrow keys, Enter, Escape)
- Open file and open in explorer actions
- Copy file path to clipboard
- Search history
- Configuration persistence
- Auto-index on startup
- Dark theme UI
- MSI installer for Windows

### Technical
- Built with Tauri v2 and Rust
- Parallel search with rayon
- Fuzzy matching with fuzzy-matcher
- File watching with notify crate
- Configuration stored in `%APPDATA%/QuickFind/config.json`

---

[0.2.0]: https://github.com/your-username/quickfind/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/your-username/quickfind/releases/tag/v0.1.0