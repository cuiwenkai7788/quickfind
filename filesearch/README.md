# QuickFind

⚡ 极速文件搜索工具 - 类似 Listary 的开源替代方案

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Platform](https://img.shields.io/badge/platform-Windows-lightgrey.svg)](https://www.microsoft.com/windows)
[![Tauri](https://img.shields.io/badge/Tauri-v2-24c8db.svg)](https://tauri.app)

## ✨ 特性

### 核心功能
- **🚀 极速搜索** - 毫秒级响应，模糊匹配，智能排序
- **🔍 实时索引** - 全盘扫描 + 文件变化监听
- **⌨️ 全键盘操作** - 纯键盘流，效率至上
- **🎯 智能筛选** - 按文件类型快速过滤

### 高级功能
- **🔤 拼音搜索** - 支持中文拼音首字母和全拼
- **📄 内容搜索** - 在文件内容中搜索关键词
- **👁️ 文件预览** - 代码高亮、语法着色
- **🔌 插件系统** - 自定义命令和扩展
- **⚡ Everything 集成** - 可选连接 Everything 加速搜索

### 用户体验
- **⚡ 全局快捷键** - 一键呼出，随时搜索
- **📦 轻量级** - Rust 构建，内存占用低
- **🎨 极简设计** - 暗色主题，专注搜索

## 📸 截图

```
┌──────────────────────────────────────────────────────────┐
│ 🔍 搜索文件...                        [文件|内容] 128个结果│
├──────────────────────────────────────────────────────────┤
│ [全部] [文档] [图片] [视频] [音频] [压缩] [代码]  🟢Everything│
├──────────────────────────────────────────────────────────┤
│ 📄 report_2024.docx                           │ 预览面板 │
│    D:/Documents/report_2024.docx              │          │
│    2.3 MB · 今天 14:30                        │ 文件信息 │
├──────────────────────────────────────────────────────────┤
│ 🖼️ screenshot.png                             │ 大小:2.3M│
│    C:/Users/User/Desktop/screenshot.png       │ 类型:docx│
│    1.2 MB · 昨天                              │ 修改:今天│
├──────────────────────────────────────────────────────────┤
│ 📄 就绪                                       │          │
│    ↑↓ 选择 · Enter 打开 · Tab 预览 · ESC 清空 │          │
└──────────────────────────────────────────────────────────┘
```

## 🚀 快速开始

### 下载安装

前往 [Releases](../../releases) 页面下载最新版本。

### 从源码编译

**前置要求：**

| 工具 | 版本要求 | 下载链接 |
|------|---------|----------|
| Rust | 1.70+ | [rustup.rs](https://rustup.rs) |
| Node.js | 18+ | [nodejs.org](https://nodejs.org) |
| Visual Studio Build Tools | 2022 | [Visual Studio](https://visualstudio.microsoft.com) |

**编译步骤：**

```powershell
# 1. 克隆仓库
git clone https://github.com/your-username/quickfind.git
cd quickfind

# 2. 安装依赖
npm install

# 3. 开发模式运行
npm run tauri dev

# 4. 构建发布版本
npm run tauri build
```

编译完成后，安装包位于 `src-tauri/target/release/bundle/msi/`。

## 📖 使用指南

### 基本操作

| 操作 | 快捷键 |
|------|--------|
| 呼出/隐藏窗口 | `Ctrl+Shift+F` |
| 向下选择 | `↓` |
| 向上选择 | `↑` |
| 打开文件 | `Enter` |
| 打开文件位置 | `Ctrl+Enter` |
| 预览文件 | `Tab` |
| 清空搜索 | `Esc` |
| 重建索引 | `F5` |
| 打开设置 | `Ctrl+,` |

### 搜索语法

支持模糊搜索和类型筛选：

```
# 普通搜索
report

# 拼音搜索（中文）
xts      →  系统设置
xitong   →  系统

# 按类型筛选
video:mp4
image:png
doc:pdf

# 内容搜索（切换到"内容"模式）
关键词
```

### 文件类型筛选

点击顶部按钮快速筛选：
- **全部** - 显示所有文件
- **文档** - pdf, doc, docx, xls, xlsx, txt...
- **图片** - jpg, png, gif, svg, webp...
- **视频** - mp4, avi, mkv, mov...
- **音频** - mp3, wav, flac, aac...
- **压缩包** - zip, rar, 7z, tar...
- **代码** - js, py, rs, go, java...

## 🔌 插件系统

QuickFind 支持自定义插件扩展功能。

### 创建插件

在设置面板中点击"创建插件"，或在插件目录创建：

```
%APPDATA%/QuickFind/plugins/my-plugin/
├── manifest.json    # 插件配置
└── main.js          # 插件代码
```

### 插件示例

```javascript
// manifest.json
{
  "id": "my-plugin",
  "name": "我的插件",
  "version": "0.1.0",
  "description": "自定义功能",
  "main": "main.js",
  "commands": [
    {
      "id": "my-plugin.hello",
      "name": "打招呼",
      "shortcut": "Ctrl+H"
    }
  ]
}

// main.js
function onSearch(query, results) {
  // 修改搜索结果
  return results;
}

function onFileSelect(file) {
  // 文件选中事件
}

function onCommand(commandId, context) {
  if (commandId === 'my-plugin.hello') {
    return { message: 'Hello!' };
  }
}

module.exports = { onSearch, onFileSelect, onCommand };
```

## ⚡ Everything 集成

QuickFind 可选集成 Everything 搜索引擎：

1. 安装 [Everything](https://voidtools.com)
2. 确保 Everything 正在运行
3. QuickFind 自动检测并使用 Everything 加速搜索

### 优势
- 更快的索引速度（秒级）
- 更低的资源占用
- 支持网络驱动器

## ⚙️ 配置

配置文件位于：`%APPDATA%/QuickFind/config.json`

```json
{
  "index_roots": ["C:\\", "D:\\"],
  "hotkey": "Ctrl+Shift+F",
  "auto_index": true,
  "watch_changes": true,
  "always_on_top": true,
  "max_results": 100,
  "theme": "dark"
}
```

## 🏗️ 技术架构

```
QuickFind/
├── src/                    # 前端 (HTML + CSS + JS)
│   ├── index.html          # 主页面
│   ├── main.js             # 应用逻辑
│   └── styles.css          # 样式
├── src-tauri/              # 后端 (Rust)
│   ├── src/
│   │   ├── lib.rs          # 主入口
│   │   ├── indexer/        # 文件索引
│   │   ├── search/         # 搜索引擎
│   │   ├── pinyin/         # 拼音搜索
│   │   ├── content/        # 内容搜索
│   │   ├── preview/        # 文件预览
│   │   ├── plugins/        # 插件系统
│   │   ├── everything/     # Everything集成
│   │   └── config/         # 配置管理
│   └── Cargo.toml          # Rust 依赖
└── .github/workflows/      # CI/CD
```

### 核心技术

| 层级 | 技术 | 用途 |
|------|------|------|
| 前端 | Vanilla JS | 轻量、无框架依赖 |
| 后端 | Rust + Tauri v2 | 性能、安全、小体积 |
| 索引 | walkdir + notify | 递归遍历 + 实时监听 |
| 搜索 | fuzzy-matcher + rayon | 模糊匹配 + 并行计算 |
| 拼音 | pinyin crate | 中文拼音支持 |
| 预览 | syntect | 语法高亮 |

## 🗺️ 开发路线

### v0.2.0 (当前)
- [x] 拼音搜索
- [x] 内容搜索
- [x] 文件预览
- [x] 插件系统
- [x] Everything 集成

### v0.3.0 (计划中)
- [ ] 网络驱动器支持
- [ ] 文件标签系统
- [ ] 搜索建议
- [ ] 云同步配置

### v0.4.0 (未来)
- [ ] AI 智能搜索
- [ ] 文件关系图
- [ ] 自定义主题
- [ ] 跨平台支持

## 🤝 贡献

欢迎贡献！

### 开发

```bash
# 运行开发模式
npm run tauri dev

# 运行测试
cd src-tauri && cargo test

# 代码格式化
cargo fmt
```

## 📝 License

MIT License - 详见 [LICENSE](LICENSE) 文件

## 🙏 致谢

- [Tauri](https://tauri.app) - 跨平台桌面应用框架
- [Listary](https://listary.com) - 灵感来源
- [Everything](https://voidtools.com) - 搜索引擎集成

---

**QuickFind** - 让文件搜索变得简单快速 ⚡