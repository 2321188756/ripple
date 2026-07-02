# 开发路线图

## 已完成

### Phase 1：基础框架
- [x] Tauri v2 + React + Vite + TailwindCSS
- [x] Cargo workspace + 7 个 crate
- [x] SQLite Schema + 迁移系统（13 张表，v003）
- [x] 对话/消息 CRUD + FTS5 全文搜索
- [x] ModelProvider trait + OpenAI 兼容（SSE 流式）
- [x] 流式桥接 + StreamBuffer 节流
- [x] MarkdownRenderer（代码高亮 + KaTeX + Mermaid）

### Phase 2：多模型 + 对话管理
- [x] OpenAI 兼容 Provider（newapi）
- [x] Settings 面板（API Key / Base URL / 模型 / 对话置顶）
- [x] API Key AES-256-GCM 加密存储
- [x] 模型选择器 + 搜索/重命名/删除/置顶

### Phase 3：工具调用
- [x] 计算器工具（递归下降求值器）
- [x] 流式 ToolCall 检测 + 多轮工具调用循环
- [x] ToolCallCard 组件（嵌入流式气泡，永久保留）

### Phase 3.5：自定义 Agent
- [x] agents 表 + 后端 CRUD
- [x] 侧边栏三标签（Agent/会话/设置）
- [x] {key} 占位符 → Agents/*.txt + agent_map.json 映射
- [x] 每个 Agent 独立会话列表

### Phase 3.6：RAG 知识库
- [x] 文档分块/Embedding(newapi)/混合检索(RRF)
- [x] @kb_name 自动注入 + @ 补全
- [x] 知识库管理 UI（创建/导入/文档删除）

### Phase 3.7：插件系统
- [x] JSON 插件格式（manifest.json）
- [x] 三种模式：tool / transform / daemon
- [x] 多语言运行时：rhai / node / python / shell
- [x] 配置在线编辑（config_schema → config.json）
- [x] Settings Plugins 面板

### Phase 5：性能 + 打磨
- [x] 虚拟列表（@tanstack/react-virtual）
- [x] 上下文裁剪集成（context crate，配置可调）
- [x] Token 用量统计面板（每日柱状图 + 模型分布）
- [x] 对话导出 Markdown
- [x] 键盘快捷键（Ctrl+N/K/L）
- [x] 设置面板可拖拽/缩放/记忆
- [x] 日志实时刷新 + 滚动保留
- [x] @ 自动补全 + Mermaid 图表

---

## 待实现

### 近期
- [ ] PDF 解析（RAG 文档摄入）
- [ ] 对话导入（JSON）
- [ ] service/daemon 插件进程管理（spawn/health check）
- [ ] 多模型对比模式（并排回答）
- [ ] WASM 插件运行时（wasmtime）

### v2 规划
- [ ] 应用打包（MSI / DMG / AppImage）
- [ ] 语音输入/输出（Whisper / TTS）
- [ ] Agent 模式（多步自主任务）
- [ ] 对话分支/版本树
