# ripple-model-provider

模型抽象层 + OpenAI 兼容 Provider。

## 已实现

- `ModelProvider` trait（chat / chat_stream / list_models / validate_api_key）
- `OpenAiProvider`：OpenAI 兼容格式（newapi / DeepSeek / OpenRouter 等）
- SSE 流式解析（eventsource-stream），含 DeepSeek null content 兼容
- `ProviderRegistry`：注册与查找

## 模块

```
src/
├── lib.rs              # 导出 + reqwest 错误映射
├── traits.rs           # ModelProvider trait
├── registry.rs         # ProviderRegistry
└── providers/
    ├── mod.rs
    └── openai.rs       # OpenAI DTO + 请求构建 + SSE 解析 + 消息序列化
```
