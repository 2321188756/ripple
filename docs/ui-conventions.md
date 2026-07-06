# Ripple UI/UX 开发约定

> 踩坑记录与最佳实践，减少反复试错。

---

## 1. 布局与定位

### 1.1 `position: fixed` 与 `transform` 不兼容

**问题**：虚拟列表（`@tanstack/react-virtual`）使用 `transform: translateY()` 实现虚拟滚动。
任何 `position: fixed` 的子元素相对于该 `transform` 容器定位，而非视口。

**规则**：**`fixed` 定位的元素（弹窗/预览/右键菜单）必须在 `transform` 容器之外渲染。**

✅ 正确做法：在 App 组件层级管理全局浮动元素，通过事件机制触发：
```
MessageBubble → dispatch CustomEvent → App.tsx 监听 → 渲染 ImagePreview
```

❌ 错误做法：在 `MessageBubble` 内部直接渲染 `<ImagePreview>`（它在虚拟列表内部）。

**相关组件**：`ImagePreview`、`ContextMenu`、`Dialog`（shadcn Dialog 内部用 portal 到 body，不受影响，但自定义的 fixed 元素要注意）。

### 1.2 窗口缩放：Dialog 不支持 CSS `resize`

**问题**：shadcn `DialogContent` 使用 `translate(-50%, -50%)` 居中，CSS `resize: both` 与此冲突，
导致四个角同时缩放、卡顿。

**规则**：Dialog 缩放用固定宽高比（`w-[85vw] h-[80vh]`）+ 内容区内部滚动替代。

✅ 正确做法：
```tsx
<DialogContent className="w-[85vw] h-[80vh] flex flex-col overflow-hidden">
```

❌ 错误做法：
```tsx
<DialogContent className="..." style={{ resize: "both" }}>
```

**例外**：非 Dialog 的自定义浮动面板（如旧版设置面板）可用 `resize: both` + `position: fixed` +
`minWidth/minHeight`，但必须自行管理位置且没有 `translate` 居中。

### 1.3 独立窗口 vs 弹层

**规则**：功能完整的设置面板、长内容预览 → **独立 Tauri 窗口**。

- 设置页面 → `SettingsWindow`（加载 `index.html#settings`）
- 图片预览 → App 层全屏覆盖（不是独立窗口，因为没有复杂交互）
- 文档预览 → Dialog（内容适中，不需要独立窗口）

创建独立窗口的流程：
1. 在 `main.tsx` 中增加 hash 路由判断
2. 创建对应的独立组件
3. 用 `@tauri-apps/api/window` 或 `window.open()` 打开

---

## 2. 样式与色彩

### 2.1 设计 Token 优先，禁用硬编码颜色

使用 CSS 变量语义 token（`globals.css` 中定义）：

```tsx
✅ bg-background / text-foreground / bg-primary / text-muted-foreground
❌ bg-slate-50 / text-indigo-600 / bg-gray-100
```

### 2.2 图标统一

所有图标用 `lucide-react`，禁止 emoji 作为 UI 图标（头像/状态指示例外）。

```tsx
✅ import { Download, Bot, Settings } from "lucide-react";
❌ <span>📥</span> / <span>🤖</span>
```

### 2.3 颜色取色器

用户自定义颜色（边框/字体等）用原生 `<input type="color">`，不要用预设色块。
原生取色器跨平台一致，OS 自带调色盘体验好。

```tsx
✅ <input type="color" value={color} onChange={(e) => setColor(e.target.value)} />
❌ <button onClick={() => setColor("#6366f1")} style={{background:"#6366f1"}} />
```

---

## 3. 事件与交互

### 3.1 全局事件通信

**场景**：子组件需要触发父组件层级的 UI（如预览图片、右键菜单）。

**模式**：用 `window.dispatchEvent(new CustomEvent(...))` 而非 prop 层层传递。

```tsx
// 触发方（如在 MessageBubble 内）
window.dispatchEvent(new CustomEvent("ripple:preview-image", { detail: { url } }));

// 监听方（在 App.tsx 内）
useEffect(() => {
  const h = (e: Event) => setPreviewUrl((e as CustomEvent).detail.url);
  window.addEventListener("ripple:preview-image", h);
  return () => window.removeEventListener("ripple:preview-image", h);
}, []);
```

**事件命名约定**：`ripple:<事件名>` 避免和系统事件冲突。

### 3.2 右键菜单

浏览器默认右键菜单（检查/刷新/另存为）在桌面客户端中无用，**全局禁用**：

```tsx
useEffect(() => {
  const h = (e: MouseEvent) => e.preventDefault();
  document.addEventListener("contextmenu", h);
  return () => document.removeEventListener("contextmenu", h);
}, []);
```

自定义右键菜单用 `ContextMenu` 组件（基于自定义事件 + `position: fixed`，
注意避开 `transform` 容器）。

### 3.3 双击 vs 单击

不要在同一元素上混用 `onClick` 和 `onDoubleClick`，它们会冲突且延迟感严重。
用专用按钮（hover 显示图标）替代双击操作。

```tsx
✅ <Pencil className="opacity-0 group-hover:opacity-100" onClick={startRename} />
❌ <div onClick={preview} onDoubleClick={rename} />
```

---

## 4. 数据与持久化

### 4.1 参数命名：Tauri invoke 用 camelCase

Tauri v2 的 `invoke("command_name", params)`，**前端参数名必须用 camelCase**。
Rust 端 `#[tauri::command]` 自动转换 camelCase → snake_case。

```typescript
// frontend (camelCase)
invoke("update_agent", { iconColor: "#fff", borderWidth: 3 });

// Rust (snake_case — Tauri 自动转换)
pub async fn update_agent(icon_color: String, border_width: f64) { }
```

如果传 snake_case（如 `icon_color`），参数会被静默丢弃，保存不生效。

### 4.2 Agent 级 vs 对话级配置

Agent 的模型参数（temperature/max_tokens/top_p）是**默认值**，保存在 agents 表。
对话级应覆盖这些值（存在 conversation.metadata 中）。

### 4.3 迁移优先于直接改表

新增字段必须用 schema 迁移（`MIGRATIONS` 数组 + 版本号），不要直接 ALTER TABLE。
已有迁移：v1(初始)、v2(RAG)、v3(Agent)、v4(Agent样式/参数)。

---

## 5. 性能

### 5.1 代码分割

大依赖懒加载，避免主包过大：

```typescript
// 好：设置面板懒加载
const GeneralSettings = lazy(() => import("./GeneralSettings"));
const KnowledgePanel = lazy(() => import("./KnowledgePanel"));

// 好：build 拆 chunk
rollupOptions.output.manualChunks = {
  "react-vendor": ["react", "react-dom"],
  markdown: ["react-markdown", "remark-gfm", ...],
};
```

### 5.2 PrismLight 按需注册

语法高亮用 `PrismLight` 而非 `Prism`，只注册项目用到的语言：

```typescript
import { PrismLight } from "react-syntax-highlighter";
import js from "react-syntax-highlighter/dist/esm/languages/prism/javascript";
PrismLight.registerLanguage("javascript", js);
```

全量 Prism 含 300+ 语言 ~600KB，不要引入。

### 5.3 搜索请求防竞态

快速连续搜索时，旧请求晚到不应覆盖新结果：

```typescript
const reqIdRef = useRef(0);
const execute = async () => {
  const reqId = ++reqIdRef.current;
  const result = await search(query);
  if (reqId !== reqIdRef.current) return; // 丢弃过期结果
  setResults(result);
};
```

---

## 6. 常见陷阱速查

| 问题 | 原因 | 修复 |
|------|------|------|
| `fixed` 定位错乱 | 父级有 `transform` | 移到 App 层渲染 |
| Dialog 缩放卡顿 | `resize` + `translate` 冲突 | 用固定宽高比 |
| invoke 参数不生效 | camelCase vs snake_case 不匹配 | 前端用 camelCase |
| 双击和单击冲突 | 混用 onClick/onDoubleClick | 用 hover 按钮代替 |
| Mermaid 渲染异常 | 内联 HTML 导致 React reconcilation 冲突 | 用 state 组件 |
| 保存按钮无响应 | `dirty` 状态未追踪 | 加 `dirty` 控制 disabled |
| 日志滚动整个面板 | 外层 overflow-y-auto 抢占滚动 | 每个 TabsContent 各自控制 |
