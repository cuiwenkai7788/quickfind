// QuickFind - Main Application Logic (v0.2.0)

const { invoke } = window.__TAURI__.core;
const { open } = window.__TAURI__.dialog;

// State
let results = [];
let selectedIndex = -1;
let currentFilter = 'all';
let searchMode = 'filename'; // 'filename' or 'content'
let searchHistory = [];
let config = {};
let previewVisible = false;
let everythingAvailable = false;

// DOM Elements
const searchInput = document.getElementById('search-input');
const resultsContainer = document.getElementById('results');
const statusEl = document.getElementById('status');
const resultCountEl = document.getElementById('result-count');
const filterButtons = document.querySelectorAll('.filter-btn');
const settingsPanel = document.getElementById('settings-panel');
const previewPanel = document.getElementById('preview-panel');
const contextMenu = document.getElementById('context-menu');
const modeFilename = document.getElementById('mode-filename');
const modeContent = document.getElementById('mode-content');
const everythingStatus = document.getElementById('everything-status');

// Debounce
let debounceTimer;
function debounce(fn, delay) {
  clearTimeout(debounceTimer);
  debounceTimer = setTimeout(fn, delay);
}

// Initialize
async function init() {
  try {
    // Load config
    config = await invoke('get_config');
    applyConfig(config);

    // Load search history
    searchHistory = await invoke('get_search_history');

    // Check Everything availability
    everythingAvailable = await invoke('check_everything');
    updateEverythingStatus();

    // Check index status
    const status = await invoke('get_index_status');
    updateIndexStatus(status);

    // Start file watching if enabled
    if (config.watch_changes) {
      await invoke('start_watching');
    }

    // Load plugins
    await loadPlugins();

    log('QuickFind initialized');
  } catch (error) {
    log('Init error: ' + error, 'error');
  }
}

// Apply config to UI
function applyConfig(cfg) {
  document.getElementById('auto-index').checked = cfg.auto_index;
  document.getElementById('watch-changes').checked = cfg.watch_changes;
  document.getElementById('always-on-top').checked = cfg.always_on_top;
  document.getElementById('hotkey').value = cfg.hotkey;
  document.getElementById('show-preview').checked = true;
  document.getElementById('enable-pinyin').checked = true;

  // Render index roots
  const rootsContainer = document.getElementById('index-roots');
  rootsContainer.innerHTML = cfg.index_roots.map(root => `
    <span class="tag">${escapeHtml(root)}<span class="remove" data-path="${escapeHtml(root)}">×</span></span>
  `).join('');
}

// Update Everything status
function updateEverythingStatus() {
  if (everythingAvailable) {
    everythingStatus.innerHTML = '<span class="badge success">Everything 已连接</span>';
  } else {
    everythingStatus.innerHTML = '<span class="badge info">使用内置索引</span>';
  }
}

// Load plugins
async function loadPlugins() {
  try {
    const plugins = await invoke('get_plugins');
    const pluginList = document.getElementById('plugin-list');

    if (plugins.length === 0) {
      pluginList.innerHTML = '<p class="hint">暂无插件</p>';
      return;
    }

    pluginList.innerHTML = plugins.map(p => `
      <div class="plugin-item">
        <span class="plugin-name">${escapeHtml(p.name)}</span>
        <span class="plugin-version">${escapeHtml(p.version)}</span>
        <label class="toggle">
          <input type="checkbox" data-plugin="${escapeHtml(p.id)}" ${p.enabled ? 'checked' : ''} />
        </label>
      </div>
    `).join('');

    // Add event listeners
    pluginList.querySelectorAll('input[type="checkbox"]').forEach(cb => {
      cb.addEventListener('change', async () => {
        const id = cb.dataset.plugin;
        await invoke('toggle_plugin', { id });
      });
    });
  } catch (error) {
    log('Failed to load plugins: ' + error, 'error');
  }
}

// Search files
async function searchFiles(query) {
  if (!query.trim()) {
    results = [];
    renderResults();
    updateResultCount(0);
    updateStatus('就绪');
    return;
  }

  updateStatus('搜索中...');

  try {
    if (searchMode === 'content') {
      // Content search
      const contentResults = await invoke('search_content', {
        query: query,
        files: [] // Will search all indexed files
      });
      renderContentResults(contentResults);
    } else {
      // Filename search
      let searchQuery = query;
      if (currentFilter !== 'all') {
        searchQuery = `${currentFilter}:${query}`;
      }

      const searchResults = await invoke('search_files', { query: searchQuery });
      results = searchResults;
      selectedIndex = -1;
      renderResults();
      updateResultCount(results.length);
      updateStatus(`找到 ${results.length} 个结果`);
    }
  } catch (error) {
    log('Search error: ' + error, 'error');
    updateStatus('搜索出错');
  }
}

// Render filename results
function renderResults() {
  if (results.length === 0) {
    if (searchInput.value.trim()) {
      resultsContainer.innerHTML = `
        <div class="empty-state fade-in">
          <h3>未找到匹配的文件</h3>
          <p>尝试其他关键词或检查索引状态</p>
        </div>
      `;
    } else {
      resultsContainer.innerHTML = `
        <div class="empty-state">
          <h3>开始搜索</h3>
          <p>输入文件名或关键词搜索</p>
          <p class="hint">支持拼音搜索：输入 "xt" 可匹配 "系统"</p>
          <p class="hint">内容搜索：切换到"内容"模式</p>
        </div>
      `;
    }
    return;
  }

  resultsContainer.innerHTML = results.map((result, index) => {
    const file = result.file;
    const icon = getFileIcon(file.ext, file.is_dir);
    const size = formatSize(file.size);
    const modified = file.modified ? formatDate(file.modified) : '';

    let name = escapeHtml(file.name);
    if (result.highlights && result.highlights.length > 0) {
      const highlights = [...result.highlights].reverse();
      for (const [start, end] of highlights) {
        const before = name.substring(0, start);
        const match = name.substring(start, end);
        const after = name.substring(end);
        name = before + '<mark>' + match + '</mark>' + after;
      }
    }

    return `
      <div class="result-item ${index === selectedIndex ? 'selected' : ''} fade-in"
           data-index="${index}" data-path="${escapeHtml(file.path)}">
        <div class="result-icon">${icon}</div>
        <div class="result-info">
          <div class="result-name">${name}</div>
          <div class="result-path">${escapeHtml(file.path)}</div>
          <div class="result-meta">
            <span>${size}</span>
            ${modified ? `<span>${modified}</span>` : ''}
            ${result.score > 50 ? `<span class="score">相关度: ${result.score}</span>` : ''}
          </div>
        </div>
        <div class="result-actions">
          <button class="action-btn" data-action="open-folder">位置</button>
          <button class="action-btn" data-action="copy-path">复制</button>
          <button class="action-btn" data-action="preview">预览</button>
        </div>
      </div>
    `;
  }).join('');

  // Event listeners
  document.querySelectorAll('.result-item').forEach(item => {
    item.addEventListener('click', (e) => {
      const action = e.target.dataset.action;
      if (action) {
        e.stopPropagation();
        handleAction(action, item.dataset.path);
        return;
      }

      const index = parseInt(item.dataset.index);
      selectItem(index);
      openFile(item.dataset.path);
    });

    item.addEventListener('contextmenu', (e) => {
      e.preventDefault();
      showContextMenu(e, item.dataset.path);
    });
  });
}

// Render content search results
function renderContentResults(contentResults) {
  if (contentResults.length === 0) {
    resultsContainer.innerHTML = `
      <div class="empty-state fade-in">
        <h3>未找到匹配的内容</h3>
        <p>尝试其他关键词</p>
      </div>
    `;
    return;
  }

  resultsContainer.innerHTML = contentResults.map((match, index) => `
    <div class="content-result fade-in" data-path="${escapeHtml(match.file_path)}" data-line="${match.line_number}">
      <div class="content-header">
        <span class="file-name">${escapeHtml(getFileName(match.file_path))}</span>
        <span class="line-number">第 ${match.line_number} 行</span>
      </div>
      <div class="content-line">
        ${match.context_before ? `<div class="context-line">${escapeHtml(match.context_before)}</div>` : ''}
        <div class="matched-line">
          ${escapeHtml(match.line_content.substring(0, match.match_start))}
          <mark>${escapeHtml(match.line_content.substring(match.match_start, match.match_end))}</mark>
          ${escapeHtml(match.line_content.substring(match.match_end))}
        </div>
        ${match.context_after ? `<div class="context-line">${escapeHtml(match.context_after)}</div>` : ''}
      </div>
      <div class="content-path">${escapeHtml(match.file_path)}</div>
    </div>
  `).join('');

  updateResultCount(contentResults.length);
  updateStatus(`找到 ${contentResults.length} 处匹配`);

  // Event listeners
  document.querySelectorAll('.content-result').forEach(item => {
    item.addEventListener('click', () => {
      openFile(item.dataset.path);
    });
  });
}

// Preview file
async function previewFile(path) {
  if (!previewVisible) {
    previewPanel.classList.remove('hidden');
    previewVisible = true;
  }

  const titleEl = document.getElementById('preview-title');
  const contentEl = document.getElementById('preview-content');
  const infoEl = document.getElementById('preview-info');

  titleEl.textContent = getFileName(path);
  contentEl.innerHTML = '<div class="loading">加载中...</div>';

  try {
    const preview = await invoke('preview_file', { path });
    const info = await invoke('get_file_info', { path });

    // Render preview content
    switch (preview.content_type.type) {
      case 'Text':
        contentEl.innerHTML = `<pre class="preview-text">${escapeHtml(preview.content_type.content)}</pre>`;
        if (preview.content_type.truncated) {
          contentEl.innerHTML += '<p class="truncated">... 已截断</p>';
        }
        break;

      case 'Highlighted':
        contentEl.innerHTML = preview.content_type.html;
        if (preview.content_type.truncated) {
          contentEl.innerHTML += '<p class="truncated">... 已截断</p>';
        }
        break;

      case 'Binary':
        contentEl.innerHTML = `
          <div class="binary-preview">
            <span class="mime">${escapeHtml(preview.content_type.mime_type)}</span>
            <span class="ext">.${escapeHtml(preview.content_type.extension)}</span>
          </div>
        `;
        break;

      case 'TooLarge':
        contentEl.innerHTML = `
          <div class="too-large">
            <p>文件过大 (${formatSize(preview.content_type.size)})</p>
            <p>限制: ${formatSize(preview.content_type.limit)}</p>
          </div>
        `;
        break;

      default:
        contentEl.innerHTML = '<p class="hint">无法预览</p>';
    }

    // File info
    infoEl.innerHTML = `
      <div class="info-item"><span>大小:</span> ${formatSize(info.size)}</div>
      <div class="info-item"><span>类型:</span> ${info.mime_type || '未知'}</div>
      <div class="info-item"><span>修改:</span> ${info.modified ? formatDate(info.modified) : '未知'}</div>
      <div class="info-item"><span>路径:</span> ${escapeHtml(info.parent || '')}</div>
    `;

  } catch (error) {
    contentEl.innerHTML = `<p class="error">预览失败: ${escapeHtml(error)}</p>`;
  }
}

// Get file name from path
function getFileName(path) {
  return path.split(/[/\\]/).pop() || path;
}

// Get file icon
function getFileIcon(ext, isDir) {
  if (isDir) return '📁';

  const icons = {
    pdf: '📄', doc: '📄', docx: '📄', xls: '📊', xlsx: '📊',
    ppt: '📽️', pptx: '📽️', txt: '📝', md: '📝',
    jpg: '🖼️', jpeg: '🖼️', png: '🖼️', gif: '🖼️', svg: '🎨', webp: '🖼️',
    mp4: '🎬', avi: '🎬', mkv: '🎬', mov: '🎬', wmv: '🎬',
    mp3: '🎵', wav: '🎵', flac: '🎵', aac: '🎵',
    zip: '📦', rar: '📦', '7z': '📦', tar: '📦',
    js: '📜', ts: '📜', py: '🐍', rs: '🦀', java: '☕', cpp: '⚙️', go: '🔷',
    exe: '⚡', msi: '⚡',
  };

  return icons[ext?.toLowerCase()] || '📄';
}

// Format size
function formatSize(bytes) {
  if (bytes === 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  return `${(bytes / Math.pow(1024, i)).toFixed(i > 0 ? 1 : 0)} ${units[i]}`;
}

// Format date
function formatDate(timestamp) {
  const date = new Date(timestamp * 1000);
  const now = new Date();
  const diff = now - date;

  if (diff < 86400000 && date.getDate() === now.getDate()) {
    return `今天 ${date.getHours().toString().padStart(2, '0')}:${date.getMinutes().toString().padStart(2, '0')}`;
  }

  const yesterday = new Date(now);
  yesterday.setDate(yesterday.getDate() - 1);
  if (date.getDate() === yesterday.getDate()) {
    return '昨天';
  }

  if (date.getFullYear() === now.getFullYear()) {
    return `${date.getMonth() + 1}月${date.getDate()}日`;
  }

  return `${date.getFullYear()}/${date.getMonth() + 1}/${date.getDate()}`;
}

// Escape HTML
function escapeHtml(text) {
  if (!text) return '';
  const div = document.createElement('div');
  div.textContent = text;
  return div.innerHTML;
}

// Select item
function selectItem(index) {
  selectedIndex = index;
  renderResults();

  const item = document.querySelector(`.result-item[data-index="${index}"]`);
  if (item) {
    item.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
  }
}

// Update status
function updateStatus(text, type = '') {
  statusEl.textContent = text;
  statusEl.className = 'status ' + type;
}

// Update result count
function updateResultCount(count) {
  resultCountEl.textContent = count > 0 ? `${count} 个结果` : '';
}

// Update index status
function updateIndexStatus(status) {
  if (status.indexing) {
    updateStatus(`索引中... ${Math.round(status.progress * 100)}%`, 'loading');
  } else if (status.file_count > 0) {
    updateStatus(`已索引 ${status.file_count} 个文件`, 'success');
  } else {
    updateStatus('请先建立索引', 'loading');
  }
}

// Open file
async function openFile(path) {
  try {
    await invoke('open_file', { path });
  } catch (error) {
    log('Open error: ' + error, 'error');
  }
}

// Open in explorer
async function openInExplorer(path) {
  try {
    await invoke('open_in_explorer', { path });
  } catch (error) {
    log('Open in explorer error: ' + error, 'error');
  }
}

// Copy path
async function copyPath(path) {
  try {
    await invoke('copy_path', { path });
    updateStatus('已复制路径', 'success');
  } catch (error) {
    log('Copy error: ' + error, 'error');
  }
}

// Handle action
function handleAction(action, path) {
  switch (action) {
    case 'open-folder':
      openInExplorer(path);
      break;
    case 'copy-path':
      copyPath(path);
      break;
    case 'preview':
      previewFile(path);
      break;
  }
}

// Show context menu
function showContextMenu(e, path) {
  contextMenu.style.left = e.clientX + 'px';
  contextMenu.style.top = e.clientY + 'px';
  contextMenu.classList.remove('hidden');
  contextMenu.dataset.path = path;
}

// Hide context menu
function hideContextMenu() {
  contextMenu.classList.add('hidden');
}

// Rebuild index
async function rebuildIndex() {
  updateStatus('重建索引中...', 'loading');

  try {
    const status = await invoke('build_index');
    updateIndexStatus(status);
  } catch (error) {
    log('Build index error: ' + error, 'error');
    updateStatus('索引失败: ' + error, 'error');
  }
}

// Save settings
async function saveSettings() {
  const newConfig = {
    ...config,
    auto_index: document.getElementById('auto-index').checked,
    watch_changes: document.getElementById('watch-changes').checked,
    always_on_top: document.getElementById('always-on-top').checked,
    hotkey: document.getElementById('hotkey').value,
  };

  try {
    config = await invoke('update_config', { config: newConfig });
    updateStatus('设置已保存', 'success');
    hideSettings();
  } catch (error) {
    log('Save settings error: ' + error, 'error');
  }
}

// Show/hide settings
function showSettings() {
  settingsPanel.classList.remove('hidden');
  settingsPanel.classList.add('visible');
}

function hideSettings() {
  settingsPanel.classList.remove('visible');
  settingsPanel.classList.add('hidden');
}

// Log
function log(message, level = 'info') {
  console[level](`[QuickFind] ${message}`);
}

// Event Listeners

// Search input
searchInput.addEventListener('input', (e) => {
  debounce(() => searchFiles(e.target.value), 150);
});

// Search mode toggle
modeFilename.addEventListener('click', () => {
  searchMode = 'filename';
  modeFilename.classList.add('active');
  modeContent.classList.remove('active');
  searchInput.placeholder = '搜索文件名 (支持拼音)';
  searchFiles(searchInput.value);
});

modeContent.addEventListener('click', () => {
  searchMode = 'content';
  modeContent.classList.add('active');
  modeFilename.classList.remove('active');
  searchInput.placeholder = '搜索文件内容...';
  searchFiles(searchInput.value);
});

// Keyboard navigation
searchInput.addEventListener('keydown', (e) => {
  switch (e.key) {
    case 'ArrowDown':
      e.preventDefault();
      if (selectedIndex < results.length - 1) {
        selectItem(selectedIndex + 1);
      }
      break;

    case 'ArrowUp':
      e.preventDefault();
      if (selectedIndex > 0) {
        selectItem(selectedIndex - 1);
      } else if (selectedIndex === 0) {
        selectedIndex = -1;
        searchInput.focus();
      }
      break;

    case 'Enter':
      e.preventDefault();
      if (e.ctrlKey && selectedIndex >= 0) {
        openInExplorer(results[selectedIndex].file.path);
      } else if (selectedIndex >= 0 && results[selectedIndex]) {
        openFile(results[selectedIndex].file.path);
      }
      break;

    case 'Tab':
      e.preventDefault();
      if (selectedIndex >= 0) {
        previewFile(results[selectedIndex].file.path);
      }
      break;

    case 'Escape':
      searchInput.value = '';
      results = [];
      renderResults();
      updateResultCount(0);
      updateStatus('就绪');
      hideContextMenu();
      if (previewVisible) {
        previewPanel.classList.add('hidden');
        previewVisible = false;
      }
      break;

    case 'F5':
      e.preventDefault();
      rebuildIndex();
      break;

    case ',':
      if (e.ctrlKey) {
        e.preventDefault();
        if (settingsPanel.classList.contains('visible')) {
          hideSettings();
        } else {
          showSettings();
        }
      }
      break;
  }
});

// Type filter buttons
filterButtons.forEach(btn => {
  btn.addEventListener('click', () => {
    filterButtons.forEach(b => b.classList.remove('active'));
    btn.classList.add('active');
    currentFilter = btn.dataset.type;
    if (searchInput.value.trim()) {
      searchFiles(searchInput.value);
    }
  });
});

// Context menu
document.addEventListener('click', hideContextMenu);

contextMenu.querySelectorAll('.menu-item').forEach(item => {
  item.addEventListener('click', () => {
    const action = item.dataset.action;
    const path = contextMenu.dataset.path;

    switch (action) {
      case 'open':
        openFile(path);
        break;
      case 'open-folder':
        openInExplorer(path);
        break;
      case 'copy-path':
        copyPath(path);
        break;
      case 'delete':
        if (confirm('确定要删除此文件吗？')) {
          invoke('delete_file', { path });
        }
        break;
    }

    hideContextMenu();
  });
});

// Settings panel
document.getElementById('close-settings')?.addEventListener('click', hideSettings);
document.getElementById('rebuild-index')?.addEventListener('click', rebuildIndex);
document.getElementById('save-settings')?.addEventListener('click', saveSettings);

// Preview panel
document.getElementById('close-preview')?.addEventListener('click', () => {
  previewPanel.classList.add('hidden');
  previewVisible = false;
});

// Add root directory
document.getElementById('add-root')?.addEventListener('click', async () => {
  const selected = await open({
    directory: true,
    multiple: false,
  });

  if (selected) {
    config.index_roots.push(selected);
    applyConfig(config);
  }
});

// Create plugin
document.getElementById('create-plugin')?.addEventListener('click', async () => {
  const name = prompt('输入插件名称:');
  if (name) {
    try {
      const path = await invoke('create_plugin', { name });
      alert(`插件已创建: ${path}`);
      loadPlugins();
    } catch (error) {
      alert('创建失败: ' + error);
    }
  }
});

// Initialize on load
init();

// Global error handler
window.onerror = (msg, url, line) => {
  log(`Error: ${msg} at ${url}:${line}`, 'error');
};