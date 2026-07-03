# 技术栈

## 总览

| 层级 | 技术 | 版本 | 选型理由 |
|------|------|------|----------|
| 桌面框架 | Tauri | v2 | Rust 后端 + Web 前端，包体积仅 Electron 的 1/10，安全模型完善 |
| 后端语言 | Rust | stable (edition 2021) | 性能、内存安全、与 Tauri 深度集成，async 生态成熟 |
| 异步运行时 | tokio | 1.x (full features) | Rust 事实标准，多线程调度、流式处理 |
| 前端框架 | React | 18.x | Chat/Markdown 组件生态最成熟 |
| 类型系统 | TypeScript | 5.x (strict) | 前端类型安全 |
| 构建工具 | Vite | 6.x | 极速 HMR，Tauri 官方推荐，manualChunks 分包 |
| UI 样式 | TailwindCSS | 3.4 | 原子化 CSS，`darkMode: "class"` + CSS 变量 token |
| 状态管理 | Zustand | 5.x | 轻量，原子 selector 精确订阅，与 Tauri 事件驱动架构契合 |
| 数据库 | SQLite | via rusqlite 0.32 (bundled) | 桌面标配，单文件，ACID，FTS5 全文搜索 |

> **注意**：不使用 sqlite-vec。嵌入向量以 JSON 存 `chunks.embedding_json` 列，检索时 Rust 端 `cosine_similarity` 计算。不使用 wasmtime/WASM 插件（插件走 rhai/node/python/shell 子进程）。不使用 shiki（代码高亮用 react-syntax-highlighter 的 PrismLight）。

---

## Rust 依赖

### 核心框架（src-tauri/Cargo.toml）

| Crate | 版本 | 用途 |
|-------|------|------|
| `tauri` | 2.x | 桌面应用框架、IPC、打包、多窗口 |
| `tauri-plugin-shell` | 2.x | Shell 调用能力 |
| `tauri-plugin-dialog` | 2.x | 文件选择对话框 |
| `tauri-plugin-fs` | 2.x | 文件系统访问 |
| `tauri-plugin-store` | 2.x | KV 存储 |

### 异步与序列化

| Crate | 版本 | 用途 |
|-------|------|------|
| `tokio` | 1.x | 异步运行时（full features：rt-multi-thread, sync, process, ...） |
| `serde` / `serde_json` | 1.x | 序列化/反序列化 |
| `async-trait` | 0.1 | async trait 支持 |
| `futures` | 0.3 | Stream trait 处理 |

### HTTP 与流式

| Crate | 版本 | 用途 |
|-------|------|------|
| `reqwest` | 0.12 (json + stream + rustls-tls) | HTTP 客户端 |
| `eventsource-stream` | 0.2 | SSE 流解析 |
| `tokio-stream` | 0.1 | 异步 Stream 工具 |

### 数据库

| Crate | 版本 | 用途 |
|-------|------|------|
| `rusqlite` | 0.32 (bundled) | SQLite（含 FTS5） |
| `r2d2` / `r2d2_sqlite` | 0.8 / 0.25 | 连接池（max 8，`with_init` 设 PRAGMA） |

### 安全

| Crate | 版本 | 用途 |
|-------|------|------|
| `aes-gcm` | 0.10 | AES-256-GCM 加密 |
| `argon2` | 0.5 | 密钥派生 |
| `zeroize` | 1.x | 敏感内存清零 |

### 工具与杂项

| Crate | 版本 | 用途 |
|-------|------|------|
| `thiserror` | 2.x | 错误类型 |
| `tracing` / `tracing-subscriber` / `tracing-appender` | 0.1 / 0.3 / 0.2 | 结构化日志（控台 + 滚动文件） |
| `uuid` | 1.x (v4, serde) | 唯一 ID |
| `chrono` | 0.4 (serde) | 时间处理 |
| `once_cell` | 1.x | 全局静态（插件注册表） |
| `rand` | 0.8 | 随机数 |
| `dirs` | 6.x | 系统目录 |

> Token 计数用自研 `CharApproxCounter`（`chars().count() / 4`），不依赖 tiktoken-rs。

---

## 前端依赖（package.json）

### 框架与状态

| 库 | 版本 | 用途 |
|----|------|------|
| `react` / `react-dom` | 18.x | UI 框架 |
| `zustand` | 5.x | 状态管理（原子 selector） |
| `@tauri-apps/api` | 2.x | IPC（invoke / listen / emit / WebviewWindow） |
| `@tauri-apps/plugin-*` | 2.x | dialog / fs / shell / store 插件前端绑定 |

### 渲染

| 库 | 版本 | 用途 |
|----|------|------|
| `react-markdown` | 9.x | Markdown 渲染 |
| `remark-gfm` | 4.x | GFM 扩展（表格、任务列表等） |
| `remark-math` | 6.x | 数学公式语法支持 |
| `rehype-katex` | 7.x | KaTeX 渲染 |
| `katex` | 0.17 | KaTeX 样式 |
| `react-syntax-highlighter` | 16.x | 代码高亮（**PrismLight** 按需注册语言，非全量 Prism） |
| `mermaid` | 11.x | 图表渲染（动态 import 懒加载） |

### 交互与性能

| 库 | 版本 | 用途 |
|----|------|------|
| `@tanstack/react-virtual` | 3.x | 虚拟列表（消息/对话列表） |
| `@radix-ui/react-*` | latest | 无障碍原语（dialog/dropdown/popover/select/tooltip/scroll-area/tabs/...） |
| `lucide-react` | 1.x | 图标库 |
| `sonner` | 2.x | Toast 通知 |
| `class-variance-authority` + `clsx` + `tailwind-merge` | — | className 合并（`cn()`） |
| `tailwindcss-animate` | 1.x | 动画工具类 |

### 构建与样式

| 库 | 版本 | 用途 |
|----|------|------|
| `vite` | 6.x | 构建工具（manualChunks: react-vendor / radix-vendor / markdown / syntax-highlight） |
| `tailwindcss` | 3.4 | 原子化 CSS |
| `@tailwindcss/typography` | 0.5 | prose 排版 |
| `typescript` | 5.x | 类型检查 |

> 已移除 `framer-motion`（全仓未使用）、`next-themes`（主题用自研 `useTheme`）。

---

## 版本约定

- Rust：stable channel，edition 2021
- Node.js：18 LTS+
- 所有依赖锁定在 `Cargo.lock` / `package-lock.json`，避免隐式升级
