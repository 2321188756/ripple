# src-tauri (Rust Backend)

Ripple 的 Rust 后端，基于 Tauri v2。

## 结构

```
src-tauri/
├── Cargo.toml              # workspace + app crate
├── tauri.conf.json         # Tauri 配置
├── capabilities/
│   └── default.json        # v2 权限
├── build.rs                # Tauri 构建脚本
├── crates/                 # 核心 crate
│   ├── core/               # 共享类型 + 错误
│   ├── model-provider/     # 模型抽象 + OpenAI/SSE
│   ├── streaming/          # 流式节流
│   ├── context/            # 上下文裁剪
│   ├── security/           # API Key 加密
│   └── conversation-store/ # SQLite 持久化
├── icons/                  # 应用图标
└── src/
    ├── main.rs             # 入口
    ├── lib.rs              # App 构建 + setup
    ├── state.rs            # AppState
    └── commands/           # IPC 命令
        ├── chat.rs         # 发送消息 + 工具调用循环
        ├── conversation.rs # 对话 CRUD
        ├── message.rs      # 消息查询 + FTS5
        ├── settings.rs     # 设置读写
        ├── log.rs          # 日志管理
        ├── tools.rs        # 计算器工具
        └── test_chat.rs    # API 连通性测试
```

## 依赖关系

```
commands ──→ model-provider ──→ core
           ─→ conversation-store ──→ core
           ─→ streaming ──→ core
           ─→ security ──→ core
           ─→ context ──→ core
           ─→ tools (内联)
```

`core` 是最底层，不依赖任何业务 crate。commands 为应用层，编排所有业务 crate。

## 开发

```bash
cargo test --workspace          # 运行全部测试（27+ 个）
cargo build                     # 编译
./target/debug/ripple-app.exe   # 直接运行（需先起 Vite）
```

## AppState

```rust
pub struct AppState {
    pub db: DbPool,
    pub providers: Arc<ProviderRegistry>,
    pub key_manager: Arc<KeyManager>,
    pub active_streams: Arc<Mutex<HashMap<String, ActiveStream>>>,
    pub interrupted: Arc<Notify>,
}
```

## 测试覆盖

| Crate | 测试数 | 测试内容 |
|-------|--------|----------|
| streaming | 5 | StreamBuffer 节流、consume 流、错误传播 |
| context | 8 | 滑动窗口、摘要裁剪、预算、工具链保护 |
| security | 7 | AES 加密回环、错误密码/篡改/Unicode |
| conversation-store | 7 | 对话 CRUD、FTS5 搜索、分页、级联删除 |
| tools | 6 | 计算器四则运算、函数、精度、错误处理 |
