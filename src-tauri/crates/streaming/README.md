# ripple-streaming

流式节流与消费器。接收 model-provider 的 `Stream<StreamChunk>`，用 `StreamBuffer` 合并高频文本增量。

## 已实现

- `StreamBuffer`：50ms / 500char 节流，天然反压
- `consume_stream`：消费 stream 并通过回调输出 `StreamEvent`
- `extract_signal`：检测工具调用/完成/用量等控制信号

## 测试（5 个）

- buffer_holds_until_interval_or_size：达到阈值才 flush
- flush_drains_remaining：强制 flush 残留
- consume_merges_text_and_passes_signals：文本合并 + 信号透传
- consume_propagates_error_after_flush：错误前先 flush
