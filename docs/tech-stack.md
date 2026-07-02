# 技术栈

## 总览

| 层级 | 技术 | 版本 | 选型理由 |
|------|------|------|----------|
| 桌面框架 | Tauri | v2 | Rust 后端 + Web 前端，包体积仅 Electron 的 1/10，安全模型完善 |
| 后端语言 | Rust | stable | 性能、内存安全、与 Tauri 深度集成，async 生态成熟 |
| 异步运行时 | tokio | 1.x | Rust 事实标准，多线程调度、流式处理 |
| 前端框架 | React | 18.x | Chat/Markdown 组件生态最成熟，社区方案丰富 |
| 类型系统 | TypeScript | 5.x | 前端类型安全 |
| 构建工具 | Vite | 6.x | 极速 HMR，Tauri 官方推荐 |
| UI 样式 | TailwindCSS | 4.x | 原子化 CSS，暗色/亮色主题便捷 |
| 状态管理 | Zustand | 5.x | 轻量，与 Tauri 事件驱动架构契合 |
| 数据库 | SQLite | (via rusqlite) | 桌面标配，单文件，ACID，FTS5 全文搜索 |

---

## Rust 依赖

### 核心框架

| Crate | 版本 | 用途 |
|-------|------|------|
| `tauri` | 2.x | 桌面应用框架、IPC、打包 |
| `tauri-plugin-shell` | 2.x | Shell 调用能力 |
| `tauri-plugin-dialog` | 2.x | 文件选择对话框 |
| `tauri-plugin-fs` | 2.x | 文件系统访问 |
| `tauri-plugin-store` | 2.x | 加密 KV 存储（应用设置） |
| `tauri-plugin-notification` | 2.x | 系统通知 |

### 异步与序列化

| Crate | 版本 | 用途 |
|-------|------|------|
| `tokio` | 1.x | 异步运行时（full features） |
| `serde` / `serde_json` | 1.x | 序列化/反序列化 |
| `async-trait` | 0.1 | async trait 支持 |
| `futures` | 0.3 | Stream trait 处理 |

### HTTP 与流式

| Crate | 版本 | 用途 |
|-------|------|------|
| `reqwest` | 0.12 | HTTP 客户端（json + stream + rustls-tls） |
| `eventsource-stream` | 0.2 | SSE 流解析 |
| `tokio-stream` | 0.1 | 异步 Stream 工具 |

### 数据库

| Crate | 版本 | 用途 |
|-------|------|------|
| `rusqlite` | 0.32 | SQLite（bundled + FTS5） |
| `r2d2` / `r2d2_sqlite` | 0.25 | 连接池 |
| `sqlite-vec` | 0.1.x | 向量扩展（RAG KNN 搜索） |

### 插件引擎

| Crate | 版本 | 用途 |
|-------|------|------|
| `wasmtime` | 28.x | WASM 运行时 |
| `wasmtime-wasi` | 28.x | WASI 支持 |

### 安全

| Crate | 版本 | 用途 |
|-------|------|------|
| `aes-gcm` | 0.10 | AES-256-GCM 加密 |
| `argon2` | 0.5 | 密钥派生 |
| `zeroize` | 1.x | 敏感内存清零 |
| `keyring` | 3.x | OS keychain（可选） |

### 工具与杂项

| Crate | 版本 | 用途 |
|-------|------|------|
| `thiserror` | 2.x | 错误类型 |
| `tracing` / `tracing-subscriber` | 0.1 | 结构化日志 |
| `uuid` | 1.x | 唯一 ID（v4） |
| `chrono` | 0.4 | 时间处理 |
| `parking_lot` | 0.12 | 高性能锁 |
| `fuzzy-matcher` | 0.3 | 模糊搜索 |

### RAG 专用

| Crate | 版本 | 用途 |
|-------|------|------|
| `tiktoken-rs` | 0.5 | Token 计数 |
| `pdf-extract` | 0.8 | PDF 文本提取 |
| `text-splitter` | 0.18 | Markdown-aware 分块 |

---

## 前端依赖

### 框架与状态

| 库 | 版本 | 用途 |
|----|------|------|
| `react` / `react-dom` | 18.x | UI 框架 |
| `zustand` | 5.x | 状态管理 |
| `@tauri-apps/api` | 2.x | IPC（invoke / listen / emit） |
| `@tauri-apps/plugin-*` | 2.x | Tauri 插件前端绑定 |

### 渲染

| 库 | 版本 | 用途 |
|----|------|------|
| `react-markdown` | 9.x | Markdown 渲染 |
| `remark-gfm` | 4.x | GFM 扩展（表格、任务列表等） |
| `remark-math` | 6.x | 数学公式语法支持 |
| `rehype-katex` | 7.x | KaTeX 渲染 |
| `shiki` | 1.x | 代码语法高亮 |
| `mermaid` | 11.x | 图表渲染 |

### 交互与性能

| 库 | 版本 | 用途 |
|----|------|------|
| `@tanstack/react-virtual` | 3.x | 虚拟列表（消息/对话列表） |
| `@radix-ui/react-*` | latest | 无障碍对话框/下拉菜单 |
| `lucide-react` | latest | 图标库 |
| `cmdk` | 1.x | 命令面板 |

### 构建与样式

| 库 | 版本 | 用途 |
|----|------|------|
| `vite` | 6.x | 构建工具 |
| `tailwindcss` | 4.x | 原子化 CSS |
| `typescript` | 5.x | 类型检查 |

---

## 版本约定

- Rust：stable channel，edition 2021
- Node.js：18 LTS+
- 所有依赖锁定在 `Cargo.lock` / `package-lock.json`，避免隐式升级
