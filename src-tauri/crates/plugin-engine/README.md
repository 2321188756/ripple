# crate: plugin-engine

WASM 插件运行时。加载、运行、隔离用户安装的插件。

## 职责

- wasmtime 引擎管理
- 加载 `.wasm` 模块，校验 manifest
- 提供 Host Functions（http-request / file-read / log），受能力令牌限制
- 插件生命周期：load → init → register tools → execute → shutdown → unload
- 能力令牌权限强制（域名/路径/命令/流量/时间）

## 模块

```
src/
├── engine.rs          # 插件宿主：加载/卸载/调用
├── wasm_runtime.rs    # wasmtime 引擎 + 实例管理
├── manifest.rs        # manifest 解析与校验
├── capability.rs      # 能力令牌系统
├── host.rs            # Host Functions 实现
└── builtin/           # 原生内置工具（非 WASM）
    ├── web_search.rs
    ├── file_read.rs
    ├── shell_exec.rs
    ├── calculator.rs
    └── rag_search.rs  # 委托 rag crate
```

## 安全

- 插件代码运行在独立 WASM 实例，内存隔离
- Host Functions 每次调用校验能力令牌，越权拒绝
- 危险能力（shell_exec）即便插件声明，仍需用户运行时审批
- 可选：WASM 模块签名校验，防篡改

插件开发详见 [docs/plugin-development.md](../../../docs/plugin-development.md)。
