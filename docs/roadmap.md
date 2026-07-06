# 开发路线图

## 已完成

### Phase 1：基础框架
- [x] Tauri v2 + React + Vite + TailwindCSS
- [x] Cargo workspace + 7 个 crate（core/model-provider/streaming/context/conversation-store/rag/security）
- [x] SQLite Schema + 版本化迁移（v005，事务化）
- [x] 对话/消息 CRUD + FTS5 全文搜索
- [x] ModelProvider trait + OpenAI 兼容（SSE 流式）
- [x] 流式桥接 + StreamBuffer 节流（50ms/500 字符）
- [x] MarkdownRenderer（代码高亮 PrismLight + KaTeX + Mermaid）

### Phase 2：多模型 + 对话管理
- [x] OpenAI 兼容 Provider（newapi）
- [x] Settings（API Key / Base URL / 模型 / 对话置顶）
- [x] 模型选择器 + 搜索/重命名/删除/置顶

### Phase 3：工具调用
- [x] 计算器工具（递归下降求值器）
- [x] 流式 ToolCall 检测 + 多轮工具调用循环（MAX_TOOL_ROUNDS=8）
- [x] ToolCallCard 组件（按轮次嵌入，永久保留）

### Phase 3.5：自定义 Agent
- [x] agents 表 + 后端 CRUD
- [x] 侧边栏三标签（Agent/会话/编辑）
- [x] {key} 占位符 → Agents/*.txt + agent_map.json 映射
- [x] 每个 Agent 独立会话列表（metadata.agent_id 过滤）
- [x] 无会话 Agent 清空右侧 + 发消息自动建 Agent 会话

### Phase 3.6：RAG 知识库
- [x] 文档分块/Embedding(newapi Qwen3-Embedding-8B)/混合检索(向量+FTS5+RRF)
- [x] chunks_fts 触发器维护（MIGRATION_005）
- [x] @kb_name 自动注入 + @ 补全
- [x] 知识库管理 UI（创建/导入/文档删除/批量导入/重命名/在线编辑）

### Phase 3.7：插件系统
- [x] JSON 插件格式（manifest.json）
- [x] 三种模式：tool / transform / daemon
- [x] 多语言运行时：rhai / node / python / shell
- [x] plugin_ 前缀剥离（工具执行修复）
- [x] 配置在线编辑（config_schema → config.json）
- [x] Settings Plugins 面板

### Phase 5：性能 + 打磨
- [x] 虚拟列表（@tanstack/react-virtual）
- [x] 上下文裁剪集成（context crate，配置可调）
- [x] Token 用量统计面板（每日柱状图 + 模型分布）
- [x] 对话导出 Markdown
- [x] 键盘快捷键（Ctrl+N/K/逗号）
- [x] 日志实时刷新 + 滚动保留
- [x] @ 自动补全 + Mermaid 图表

### Phase 6：全面审计与优化（2026-07）
- [x] 后端严重 bug 修复：delete_from 删全、stop_generation 不取消、插件前缀、chunks_fts 无触发器、PRAGMA per-connection、迁移非原子、import_folder 错配、RRF 饱和、DB 跨 await 持锁、非流式 tool_calls 丢失、token 计数漏块
- [x] 前端严重 bug 修复：regenerate 不截断、switchConversation 缓存不刷新/流中不停止、首块竞态丢字、finalize 落错对话、stop 丢半截回复、流式 Enter 清空输入、mermaid DOM 突变、toolEvents 堆错轮次、XSS、ContextMenu 泄漏
- [x] 前端性能：原子 selector（消除每 token 全树重渲染）、MessageBubble/MarkdownRenderer memo、Vite manualChunks（主 chunk 1.5MB→161KB）、PrismLight 按需语言、设置面板懒加载、移除 framer-motion 死依赖、toolEvents 类型化
- [x] 独立设置窗口（OS 原生窗口 + hash 路由 + 跨窗口状态同步 + App 懒加载提速）

### Phase 7：打磨与功能补全（2026-07）
- [x] 清理预存编译警告（cargo check 零警告）
- [x] 非 ASCII API Key 返回明确错误（不再静默 401）
- [x] KnowledgePanel 拆分（488 行 → 主面板 ~290 + 4 子组件），顺带修「打开编辑」菜单 bug
- [x] reqwest::Client 复用（存 AppState，避免每请求 TLS 握手）
- [x] 插件 exec_process 改 tokio::process（不阻塞 tokio 线程）
- [x] ErrorBanner 重试按钮 + EmptyChatPlaceholder 显示当前 Agent
- [x] PDF 解析（RAG 文档摄入，pdf-extract）
- [x] 对话 JSON 导入导出（ChatHeader 导出下拉 + GeneralSettings 导入按钮 + 跨窗口刷新）

---

## 待实现

### 近期
- [ ] **KeyManager 安全硬化**（推荐独立专项）：持久化随机 machine_id（独立文件）、save_api_key/has_api_key 命令、send_message 空时后端解密、前端不再缓存明文、旧版明文 key 迁移。涉及 send_message 契约变更，需谨慎。
- [ ] service/daemon 插件进程管理（spawn/health check）

### v2 规划
- [ ] 应用打包（MSI / DMG / AppImage）
- [ ] 语音输入/输出（Whisper / TTS）
- [ ] Agent 模式（多步自主任务）
- [ ] 对话分支/版本树
- [ ] 多模型对比模式（并排回答）
