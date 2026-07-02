# crate: core

共享基础类型、配置与错误定义。被所有业务 crate 依赖，本身不依赖任何业务 crate。

## 职责

- 统一数据类型：`Conversation` / `Message` / `ContentBlock` / `ProviderConfig` / `ModelInfo` / `ToolDefinition` / `PluginManifest`
- 应用配置：`AppConfig`（模型默认值、性能参数、路径）
- 统一错误类型：`RippleError`（thiserror），供各 crate 转换

## 依赖

`serde` / `serde_json` / `thiserror` / `uuid` / `chrono` / `tracing`

## 关键类型

详见 [docs/architecture.md](../../../docs/architecture.md) 与 [docs/database.md](../../../docs/database.md)。
