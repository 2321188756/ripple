# VCPToolBox RAG 设计文档

**文档版本：** 1.0.0  
**基准提交：** `c1ea9b5`（`origin/main`）  
**更新日期：** 2026-07-17  
**范围：** 基于当前仓库的静态源码审查；未启动服务、未调用外部 Embedding 或 Rerank 服务，运行时状态需要另行验证。

---

## 目录

1. [目标、范围与证据约定](#1-目标范围与证据约定)
2. [系统总览](#2-系统总览)
3. [核心模块与职责](#3-核心模块与职责)
4. [共用 Embedding 层](#4-共用-embedding-层)
5. [热记忆 RAG：摄取与存储](#5-热记忆-rag摄取与存储)
6. [热记忆 RAG：检索与增强](#6-热记忆-rag检索与增强)
7. [请求期编排与混合召回](#7-请求期编排与混合召回)
8. [冷知识库 RAG：TriviumDB 链路](#8-冷知识库-ragtriviumdb-链路)
9. [结果注入与溯源格式](#9-结果注入与溯源格式)
10. [生命周期、一致性与恢复](#10-生命周期一致性与恢复)
11. [可复用边界与 VCP 专有扩展](#11-可复用边界与-vcp-专有扩展)
12. [限制、风险与部署建议](#12-限制风险与部署建议)
13. [核心文件索引与验证清单](#13-核心文件索引与验证清单)

---

## 1. 目标、范围与证据约定

本文档描述 VCPToolBox 当前实现的检索增强生成（RAG）系统，目标是让开发者能够：

- 理解从文件变更到模型上下文注入的完整数据流；
- 区分“热记忆（日记）”与“冷知识库”两条检索产品线；
- 在不重复通读大型插件代码的前提下，定位可修改的边界；
- 复用稳定的 RAG 原语，同时避免把 VCP 的实验性增强误当成通用必需组件。

### 1.1 证据标签

- **已确认：**直接由当前源码、配置样例或已跟踪文档证明。
- **推断：**依据控制流和接口语义做出的工程判断。
- **运行时待验证：**依赖外部 API、真实索引数据、环境配置或 native 模块状态的结论。

关键事实按下列格式定位：

```text
📁 文件路径：相对仓库根目录
📍 位置：行号范围或函数名
💡 说明：该证据说明的实现语义或边界
```

### 1.2 设计结论

**已确认：**系统不是一个单一“向量库加 Top-K”实现，而是由两个独立数据面和一个请求期编排层构成：

1. **热记忆 RAG**：面向 `dailynote/`；以 SQLite 保存文件、chunk、标签与向量，以 Rust Vexus 索引做按日记本隔离的 ANN 加速；支持 TagMemo、时间、BM25、关联发现等扩展。
2. **冷知识库 RAG**：面向 `knowledge/`；使用 TriviumDB 保存文档和 chunk 节点及关系，并用元数据 SQLite 维护可靠摄取队列。
3. **请求期编排层**：`RAGDiaryPlugin` 解析消息中的占位符，构造查询向量，调用热或冷检索，再把结果写回将发送给上游模型的 messages。

---

## 2. 系统总览

### 2.1 端到端主路径

```text
聊天请求
  → POST /v1/chat/completions 或 /v1/chatvcp/completions
  → ChatCompletionHandler 与 PluginManager
  → RAGDiaryPlugin.processMessages()
  → 解析 system 或虚拟 system 消息中的占位符
  → 从最后真实 user 与最近 assistant 上下文构建 query vector
  → 热记忆：Vexus ANN 加 SQLite hydrate
      或 冷知识：TriviumDB hybrid search
  → 可选：TagMemo、Time、BM25、TimeDecay、Rerank、RRF、Associate、Expand
  → RAGResultFormatter 打包带溯源的检索上下文
  → 将占位符替换为 RAG block
  → 处理后的 messages 发送给上游模型
```

📁 `server.js`  
📍 聊天路由约 1205-1231；`initialize()` 约 1507-1572  
💡 主服务先初始化热记忆与冷知识库管理器，再把它们注入插件与消息处理链路。

### 2.2 两条数据路径

```text
热记忆
  dailynote 文件
  → watcher
  → chunk 与 Tag 提取
  → Embedding
  → SQLite files、chunks、tags、file_tags
  → 每个日记本一个 Vexus index，加一个全局 tag index
  → RAGDiaryPlugin 检索与注入

冷知识库
  knowledge 下的 library 文件
  → watcher 与 SQLite ingest_queue
  → chunk 与 Embedding
  → TriviumDB document 和 chunk nodes、contains、next、prev 关系
  → 混合搜索与可选全文展开
  → RAGDiaryPlugin 检索与注入
```

**推断：**热路径优化“频繁编辑的个人记忆与日记”，冷路径优化“更大规模、文档化、可按库隔离的事实知识”。两者共用 Embedding 基础设施，但不共享同一持久化或索引模型。

---

## 3. 核心模块与职责

| 层次 | 实现 | 职责 |
|---|---|---|
| 服务启动 | `server.js` | 初始化两个知识库管理器，向插件链路提供依赖，注册聊天入口。 |
| 热库门面 | `KnowledgeBaseManager.js` | 管理 SQLite、日记本索引、watcher、摄取、检索、恢复和关闭。 |
| 热库摄取 | `modules/knowledgeBase/ingestionPipeline.js` | 稳定读取、增量判断、切块/标签向量化、SQLite 写入和索引同步。 |
| 热库检索 | `modules/knowledgeBase/searchService.js` | 日记索引加载、ANN、TagMemo 增强、测地线重排、SQLite 回填。 |
| 文本与嵌入 | `TextChunker.js`、`EmbeddingUtils.js` | token 感知分块；OpenAI 形状兼容的 Embedding 请求、批处理与回退。 |
| 标签增强 | `TagMemoEngine.js` | 标签资产、查询向量增强、能量场和可选 Geodesic rerank。 |
| 请求编排 | `Plugin/RAGDiaryPlugin/RAGDiaryPlugin.js` | 占位符、查询构造、热/冷调用、BM25、时间、Rerank、去重和结果组合。 |
| 注入渲染 | `Plugin/RAGDiaryPlugin/RAGResultFormatter.js` | 结果文本、来源路径、RAG block 元数据。 |
| 冷库 | `TDBKnowledge.js` | 可靠队列、TriviumDB 分库、节点关系、混合检索与全文展开。 |
| 管理面 | `routes/admin/rag.js`、`AdminPanel-Vue/src/views/RagTuning.vue` | RAG 参数、主题、标签和 TagMemo 主动训练。 |

---

## 4. 共用 Embedding 层

### 4.1 API 契约、批处理与降级

**已确认：**Embedding 层使用 OpenAI 形状兼容的 HTTP 请求：`POST {apiUrl}/v1/embeddings`，body 是 `{ model, input }`，认证采用 Bearer API Key。该描述仅指请求格式兼容，并不意味着依赖某一家厂商或 SDK。

📁 `EmbeddingUtils.js`  
📍 `_sendBatch()`，51-127  
💡 逐个候选模型尝试；429 时等待后切换候选模型；成功响应按服务返回的 `index` 恢复为输入顺序。

```js
const requestUrl = `${config.apiUrl}/v1/embeddings`;
const requestBody = { model, input: batchTexts };
const requestHeaders = {
    'Content-Type': 'application/json',
    'Authorization': `Bearer ${config.apiKey}`
};

const response = await fetch(requestUrl, {
    method: 'POST',
    headers: requestHeaders,
    body: JSON.stringify(requestBody)
});

return data.data
    .sort((a, b) => a.index - b.index)
    .map(item => item.embedding);
```

### 4.2 关键不变量

- `getEmbeddingsBatch()` 会保持返回数组与输入文本列表**位置对齐**；失败或超长文本以 `null` 占位，而非移除项目。
- 调用方必须跳过 `null`，不能把它写成零向量或错误 BLOB。
- 向量长度必须等于 `VECTORDB_DIMENSION`/热库配置 dimension；检索层会在搜索前校验。
- 代码还按 token 和 item 数拆分 batch、控制并发，并支持配置与环境变量中的备援模型。

📁 `EmbeddingUtils.js`  
📍 `getEmbeddingsBatch()`，141-248  
💡 该函数实现并发控制、token 上限、失败位置保留和整批结果对齐。

**运行时待验证：**部署配置中的向量维度是否与当前实际模型输出完全一致；代码不会从远程服务自动协商维度。

---

## 5. 热记忆 RAG：摄取与存储

### 5.1 数据模型与事实源

热库默认源目录为 `dailynote/`，持久化目录为 `VectorStore/`，SQLite 文件为 `VectorStore/knowledge_base.sqlite`。核心表如下：

| 表 | 关键字段 | 职责 |
|---|---|---|
| `files` | `path`、`diary_name`、`checksum`、`mtime`、`size` | 文件事实表。 |
| `chunks` | `file_id`、`chunk_index`、`content`、`vector` | 文本切块及 Float32 向量 BLOB。 |
| `tags` | `name`、`vector` | 标签与标签向量。 |
| `file_tags` | `file_id`、`tag_id`、`position` | 文件标签关系及标签顺序。 |
| `migration_deleted_*` | 文件与 chunk 墓碑 | 移动/复制时短期复用已有向量。 |

📁 `modules/knowledgeBase/schemaManager.js`  
📍 schema 初始化，3-195  
💡 SQLite 保存可恢复的真实数据；Vexus 索引是派生加速层，而不是唯一真相源。

### 5.2 监听、稳定快照与文本处理

**已确认：**热库只处理 `.md`、`.txt`，排除 `.git`、`node_modules`、`dist`、隐藏目录等。优先使用 Rust watcher，失败后回退 Chokidar；回退路径会对文件做两次 `stat` 并等待稳定，避免把编辑中的中间态写入索引。

📁 `modules/knowledgeBase/fileWatcher.js`  
📍 文件过滤、Rust watcher 与 Chokidar 回退，53-320  
💡 watcher 事件进入待处理集合，而不是立即同步写库。

切块采用 `cl100k_base` tokenizer。安全上限默认是 Embedding 最大 token 的 85%，并按句子边界优先切分，超长句再按 token 强制分段。标签从 `Tag:` 行提取并经黑名单、日期和长度过滤。

📁 `TextChunker.js`  
📍 分块与 overlap，1-133  
📁 `modules/knowledgeBase/textPreprocessor.js`  
📍 Embedding 清洗与标签提取，3-75

### 5.3 摄取事务与派生索引更新

摄取批次的核心顺序是：收集有效 chunks 与标签 → 批量 Embedding → SQLite transaction 写入文件/切块/标签关系 → 在事务成功后更新 Vexus 派生索引。

📁 `modules/knowledgeBase/ingestionPipeline.js`  
📍 `_flushBatch()`，161-343、345-461  
💡 同内容移动或复制时，先尝试复用 SQLite/墓碑中的 chunk vectors，降低重新向量化成本。

```js
const allChunksWithMeta = [];
for (const [dName, docs] of docsByDiary) {
    docs.forEach((doc) => {
        const validChunks = doc.chunks
            .map(c => this._prepareTextForEmbedding(c))
            .filter(c => c !== '[EMPTY_CONTENT]');
        doc.chunks = validChunks;

        validChunks.forEach((txt, cIdx) => {
            allChunksWithMeta.push({ text: txt, diaryName: dName, doc, chunkIdx: cIdx });
        });
    });
}

const texts = allChunksWithMeta.map(i => i.text);
const chunkVectors = await getEmbeddingsBatch(texts, embeddingConfig);
// 返回长度与 texts 对齐；失败或超长的位置是 null。

const transaction = this.db.transaction(() => {
    // 写 files、chunks、tags、file_tags，并收集索引更新和删除信息。
});
transaction();
```

**边界：**为便于阅读，片段省略了 transaction 内部逐项 SQL 写入与事务后 Vexus 更新循环；二者都在同一个 `_flushBatch()` 中。SQLite 事务保证关系数据写入的一致性；Vexus 更新发生在事务成功之后。因此这不是跨 SQLite 与 native ANN 索引的全局原子事务。系统通过启动恢复、Ghost Index 清理、数量自检和重新构建来修复派生索引偏差。

### 5.4 删除、迁移与持久化

- 删除前可保存 chunk 向量墓碑；默认墓碑 TTL 为 2 分钟。
- 大批量删除超过阈值时，会丢弃对应日记本的派生索引，并在下次搜索时从 SQLite 恢复，而不是逐个删除大量 ANN ID。
- 每个日记本可有 `.usearch` 持久化文件；是否持久化由默认开关、白名单目录及名称规则控制。
- 空闲日记索引会在默认两小时后卸载，释放内存。

📁 `modules/knowledgeBase/migrationVectorCache.js`  
📍 向量复用与墓碑写入，55-182  
📁 `modules/knowledgeBase/indexRepository.js`  
📍 持久化、懒加载、从 SQLite 恢复、idle unload，46-167、271-338

---

## 6. 热记忆 RAG：检索与增强

### 6.1 基线：ANN 后回填关系数据

日记本检索不是把 ANN 返回的向量直接交给模型。Vexus 首先返回 chunk ID 与 score；服务随后用批量 SQLite 查询回填正文、来源文件、更新时间和标签。

📁 `modules/knowledgeBase/searchService.js`  
📍 `_searchSpecificIndex()`，115-359  
💡 每个 diary 有独立索引；多 diary 搜索会并行各自检索，再做全局排序与截断。

### 6.2 TagMemo、候选放大与 Ghost Index 清理

TagMemo 是可选增强层。开启 `tagBoost` 后，系统可在请求内复用已经准备好的增强结果；否则调用 `applyTagBoost()` 改写 query vector 并生成能量场。开启 Geodesic 时，先扩大候选池，在 hydration 前重排，再截回原始 K。

```js
const preparedBoostResult = options?.preparedBoostResult || options?.boostResult || null;
if (preparedBoostResult?.vector) {
    searchVecFloat = preparedBoostResult.vector instanceof Float32Array
        ? preparedBoostResult.vector
        : new Float32Array(preparedBoostResult.vector);
    energyField = preparedBoostResult.energyField || null;
} else {
    const artifactResolution = preparedBoostResult?.artifactBundle
        ? null
        : this._resolveTagMemoRequest(options);
    const boostResult = this.tagMemoEngine.applyTagBoost(
        new Float32Array(vector),
        tagBoost,
        coreTags,
        coreBoostFactor,
        {
            artifactBundle: artifactResolution?.bundle,
            version: artifactResolution?.requestedVersion,
            strictVersion: options?.strictVersion === true
        }
    );
    searchVecFloat = boostResult.vector;
    energyField = boostResult.energyField || null;
}

const geoCandidatePlan = this._resolveGeodesicCandidateK(k, options);
let results = idx.search(searchVecFloat, geoCandidatePlan.candidateK);
if (options?.geodesicRerank && energyField) {
    results = this.tagMemoEngine.geodesicRerank(results, { energyField });
}
results = results.slice(0, geoCandidatePlan.requestedK);
```

📁 `modules/knowledgeBase/searchService.js`  
📍 `_searchSpecificIndex()`，132-220  
💡 上述片段保留真实控制顺序；`applyTagBoost` 的完整 options 在源码中含 artifact/version 控制，示例为突出设计边界而省略其余同函数参数。

回填和幽灵索引修复：

```js
const rows = this._queryByChunks(`
    SELECT c.id, c.content as text, f.path as sourceFile, f.updated_at, f.id as file_id
    FROM chunks c
    JOIN files f ON c.file_id = f.id
    WHERE c.id`, resultChunkIds);

for (const res of results) {
    const row = rowByChunkId.get(Number(res.id));
    if (!row) {
        if (idx.remove) idx.remove(res.id);
        continue;
    }
    hydratedResults.push({ text: row.text, score: res.score, sourceFile: row.sourceFile });
}
```

📁 `modules/knowledgeBase/searchService.js`  
📍 `_searchSpecificIndex()`，229-260  
💡 Vexus 存在、SQLite 已不存在的 ID 被识别为 Ghost Index 并删除，防止脏命中反复出现。

### 6.3 TagMemo 的定位

**已确认：**TagMemo 包含标签图资产、传播核、残差/锚点、相似度和 generation 等派生数据；查询增强与 Geodesic rerank 使用同一请求级资产快照，避免并发请求跨代混用。

📁 `TagMemoEngine.js`  
📍 资产解析与发布约 181-468；`applyTagBoost()` 约 679-1120；Geodesic rerank 约 1141-1809  
💡 它是 VCP 的可选强化层，不是部署一个基础 RAG 所必需的算法。

---

## 7. 请求期编排与混合召回

### 7.1 占位符和查询构造

RAG 由 system 或受限制的“虚拟 system user”消息中的占位符触发，常见形式包括：

| 形式 | 含义 |
|---|---|
| `[[xx日记本]]` | 直接热 RAG。 |
| `《《xx日记本》》` | 相似度门控的热 RAG。 |
| `{{xx日记本}}` | 直接文本读取，不走主 Embedding 链。 |
| `[[xx知识库]]` | 直接冷知识库 RAG。 |
| `《《xx知识库》》` | 相似度门控的冷知识库 RAG。 |

📁 `Plugin/RAGDiaryPlugin/RAGDiaryPlugin.js`  
📍 `processMessages()`，1135-1506；占位符处理，2543-3081  
💡 插件先尝试直接文本快速路径；只有需要语义检索时才计算 query embedding。

查询向量来自最后真实用户内容和最近 assistant 内容的加权组合，避免把承载系统占位符的虚拟 user 消息误认为用户查询。

```js
const lastUserMessage = findLastRealUserMessage(messages, {
    sanitize: this.sanitizeForEmbedding.bind(this)
});
const userContent = lastUserMessage.sanitizedContent || '';
const aiContent = lastAiMessageIndex > -1
    ? this.sanitizeForEmbedding(
        this._extractTextFromContent(messages[lastAiMessageIndex].content),
        'assistant'
    )
    : null;

const [userVector, aiVector] = await Promise.all([
    userContent ? this.getSingleEmbeddingCached(userContent) : null,
    aiContent ? this.getSingleEmbeddingCached(aiContent) : null
]);
const queryVector = this._getWeightedAverageVector(
    [userVector, aiVector],
    this.ragParams?.RAGDiaryPlugin?.mainSearchWeights || [0.7, 0.3]
);
```

📁 `Plugin/RAGDiaryPlugin/RAGDiaryPlugin.js`  
📍 `processMessages()`，约 1222-1313  
💡 assistant 上下文是本项目默认策略；一个通用 RAG 可只使用最后用户轮次。

### 7.2 稠密、稀疏与时间候选

插件可组合多个候选来源：普通 dense 搜索、BM25、按时间范围筛选、历史 shotgun 查询与关联发现。动态 K 和 tag 权重会参考用户长度、AI 文本广度、EPA 逻辑深度及语义宽度，并被限制在安全范围。

`::BM25` 使用文本候选与 vector cosine 的线性融合：

```text
hybridScore = normalizedBM25Score × bm25Weight
            + cosineSimilarity(queryVector, chunk.vector) × (1 - bm25Weight)
```

对应核心代码：

```js
const vectorScore = queryVector && chunk.vector
    ? this.cosineSimilarity(queryVector, chunk.vector)
    : 0;

return {
    ...chunk,
    score: (bm25Info.normalizedBM25Score * sparseWeight)
        + (vectorScore * (1 - sparseWeight)),
    bm25Score: bm25Info.bm25Score,
    source: mode === 'body' ? 'bm25_body' : 'bm25_tag'
};
```

📁 `Plugin/RAGDiaryPlugin/RAGDiaryPlugin.js`  
📍 `_getBM25RagCandidates()`，约 641-768  
💡 BM25 先在文件文本侧命中，再回取对应 chunks，并以来源/文本键去重。

### 7.3 后处理和外部 Rerank

候选生成后可按配置执行 TimeDecay、外部 Rerank、RRF、截断、时间连续性补充和关联发现。Rerank 是可选的 HTTP 兼容服务；未配置、断路器打开、请求异常或结果无效都会保留原候选排序，而非让聊天请求整体失败。

```js
if (!this.rerankConfig.url || !this.rerankConfig.apiKey || !this.rerankConfig.model) {
    return documents.slice(0, originalK);
}

const response = await axios.post(rerankUrl, {
    model: this.rerankConfig.model,
    query: truncatedQuery,
    documents: batch.map(doc => doc.text),
    top_n: batch.length
}, {
    headers,
    timeout: 30000,
    maxRedirects: 0
});

return response.data.results.map(result => ({
    ...batch[result.index],
    rerank_score: result.relevance_score
}));
```

📁 `Plugin/RAGDiaryPlugin/RAGDiaryPlugin.js`  
📍 `_rerankDocuments()`，约 3809-4041  
💡 完整实现还限制每批文档数与 token、控制并发，并对失败做 circuit breaker；RRF 使用检索排名和 rerank 排名进行融合。

---

## 8. 冷知识库 RAG：TriviumDB 链路

### 8.1 可靠队列与入库模型

冷库由 `TDBKnowledgeManager` 管理。每个 `knowledge/` 顶层目录作为一个 library；元数据 SQLite 记录 `files`、`chunks` 和 `ingest_queue`。队列有 lease 恢复、事务领取、指数退避和最多五次重试，超过上限会标记为 `failed`。

📁 `TDBKnowledge.js`  
📍 schema 与队列，127-188、416-485  
💡 这条路径与热库不同：其目标是可靠地向独立 TriviumDB library 入库，而非直接更新 Vexus。

单文件 upsert 会先做 checksum 去重，删除旧节点，再创建 document/chunk nodes、关系与文本索引：

```js
const content = await fs.readFile(normalizedPath, 'utf-8');
const checksum = crypto.createHash('sha256').update(content).digest('hex');

if (old && old.checksum === checksum && old.size === stats.size) {
    // 仅同步 mtime，跳过昂贵的重新 Embedding。
    return;
}

await this._deleteExistingFileNodes(handle, library, relPath);
const chunks = chunkText(content).filter(Boolean);
const [docVector] = await getEmbeddingsBatch([path.basename(relPath)], {
    apiKey: this.config.apiKey,
    apiUrl: this.config.apiUrl,
    model: this.config.model
});
```

📁 `TDBKnowledge.js`  
📍 `_upsertFileUnlocked()`，566-699  
💡 后续代码会对 chunks 批量 Embedding，写 document/chunk nodes，建立 `contains`、`next`、`prev` 关系并更新元数据。

### 8.2 Hybrid search 与全文展开

冷库先对 query Embedding；若有已有向量则调用 `searchWithVector()` 避免重复 Embedding。每个 library 优先调用 TriviumDB 的 `searchHybrid`，若接口不可用则回退 dense `search`。

```js
const topK = options.topK || 10;
const expandDepth = options.expandDepth ?? 1;
const minScore = options.minScore ?? 0.1;
const hybridAlpha = options.hybridAlpha ?? 0.7;

try {
    hits = this._callDb(handle.db, ['searchHybrid', 'search_hybrid'], [
        Array.from(queryVector), queryText, topK, expandDepth, minScore, hybridAlpha
    ]);
} catch (e) {
    hits = this._callDb(handle.db, ['search'], [
        Array.from(queryVector), topK, expandDepth, minScore], []);
}
```

📁 `TDBKnowledge.js`  
📍 `searchWithVector()`，803-825；`_searchLibraryUnlocked()`，868-900  
💡 TriviumDB 的精确 sparse/dense/graph 评分公式不在本仓库中，不能仅据这里断言其内部算法。

`expand=true` 时会按 `sourceFile` 读取完整文件并按文件去重，读取失败时回退原 chunk。

📁 `TDBKnowledge.js`  
📍 `_expandHits()`，828-860  
💡 全文展开有利于阅读完整材料，但会显著扩大注入模型上下文的体积。

---

## 9. 结果注入与溯源格式

热 RAG 使用带元数据的 HTML 注释边界打包结果。该协议可保留可读正文和机器可识别的 block 标记，后续刷新或替换时能定位 RAG 内容。

```js
function buildRagBlock(innerContent, metadata) {
    const metadataString = JSON.stringify(metadata).replace(/-->/g, '--\\>');
    return `<!-- VCP_RAG_BLOCK_START ${metadataString} -->${innerContent}<!-- VCP_RAG_BLOCK_END -->`;
}

function formatStandardResults(searchResults, displayName, metadata) {
    let innerContent = `\n[--- 从"${displayName}"中检索到的相关记忆片段 ---]\n`;
    innerContent += searchResults.length > 0
        ? searchResults.map(r => formatMemoryEntry(r).trimEnd()).join('\n')
        : '没有找到直接相关的记忆片段。';
    return buildRagBlock(innerContent, metadata);
}
```

📁 `Plugin/RAGDiaryPlugin/RAGResultFormatter.js`  
📍 `buildRagBlock()` 与 `formatStandardResults()`，20-44  
💡 `file:///` 本地来源路径由 `formatResultPathLine()` 生成；`VCP_RAG_BLOCK` 的 HTML 注释语法是 VCP 专有协议，而“携带溯源元数据的上下文封装”是可复用原则。

**安全边界：**检索内容并非仅展示给用户，而是会成为模型输入的一部分。因此文档正文、路径和标签都可能出现在上游模型请求、调试日志或客户端可见的上下文中。

---

## 10. 生命周期、一致性与恢复

### 10.1 热库启动与健康恢复

热库初始化顺序包括创建存储目录、打开并检查 SQLite、初始化 schema、清理孤儿、恢复全局 tag index、加载参数、初始化 TagMemo、启动 watcher/idle sweep/watchdog，并延后安排 TagMemo 派生刷新。

📁 `KnowledgeBaseManager.js`  
📍 `initialize()`，205-278  
💡 每个日记本索引采取惰性加载；SQLite 被视为恢复来源。

运行时/启动期 SQLite 损坏检测使用 `PRAGMA quick_check`；损坏时数据库、WAL、SHM 会改名备份并通过重新扫描源文件重建。

📁 `modules/knowledgeBase/sqliteHealthManager.js`  
📍 SQLite 配置与损坏恢复，19-71、151-175

### 10.2 JS 与 Rust 写入协调

系统显式协调 `better-sqlite3` 的 JS 摄取/删除与 Rust 派生计算对同一 WAL 的访问。Rust 取得 lease 前会检查数据库健康、启动冷却、JS mutation、pending 文件数及最近写入间隔；lease 有 TTL，并可在 grant 前执行 WAL checkpoint 与 quick check。

📁 `modules/knowledgeBase/databaseCoordinator.js`  
📍 协调器、lease 与 grant 条件，17-63、219-547  
💡 这降低多写入方造成 SQLite 锁竞争或派生资产跨代的问题，但真实并发效果仍需压测验证。

### 10.3 热更新与优雅关闭

- `KnowledgeBaseManager` 监听 `rag_params.json` 并保留最后一个健康配置。
- `RAGDiaryPlugin` 还监听 `rag_tags.json`，更新时重建相关缓存并清除 query cache。
- 关闭时主服务先停止入口和活跃请求，再关闭插件、TDB、热库与日志；热库会停止 watcher、等待任务/恢复尾队列、flush 索引，最后关闭 SQLite。

📁 `KnowledgeBaseManager.js`  
📍 参数监听，280-329；关闭，1046-1088  
📁 `server.js`  
📍 graceful shutdown，1721-1848

---

## 11. 可复用边界与 VCP 专有扩展

### 11.1 可直接复用的 RAG 原语

| 原语 | 本项目实现位置 | 复用说明 |
|---|---|---|
| 稳定文件摄取 | `fileWatcher.js`、`ingestionPipeline.js` | 读取前后状态校验、批处理与重试。 |
| Token 感知切块 | `TextChunker.js` | 句子优先、token 强制切分、overlap。 |
| 批量 Embedding | `EmbeddingUtils.js` | 限流、备援、位置对齐、失败 `null`。 |
| 持久化事实源 | SQLite schema 与 transaction | 用关系数据保存文本、元数据、向量和关联。 |
| ANN + hydrate | `searchService.js` | 以 ANN ID 命中，再回填正文和元数据。 |
| 稀疏/稠密融合 | `_getBM25RagCandidates()` | 归一化后做可配置分数融合。 |
| 最终 Rerank | `_rerankDocuments()` | 候选扩展后的精排、超时与降级。 |
| 溯源上下文包装 | `RAGResultFormatter.js` | 保留来源、可机器定位的注入边界。 |

### 11.2 应作为可选插件的 VCP 扩展

| 扩展 | 理由 |
|---|---|
| 日记本/知识库占位符 DSL | 与 VCP 消息协议和虚拟 system message 绑定。 |
| TagMemo、EPA、残差金字塔、Geodesic | 是 VCP 的标签图与语义动力学增强，不是通用 RAG 基础依赖。 |
| `::Time`、`::Associate`、`::Base64Memo` | 面向日记时间线、关联记忆和多模态附件的产品功能。 |
| `VCP_RAG_BLOCK` HTML 注释 | 是 VCP 用于识别/替换上下文块的具体载体。 |
| “簇”目录持久化规则 | 与本项目的日记组织方式绑定。 |

推荐的可移植最小管线是：

```text
ingest → chunk → embed → persist → ANN/hybrid retrieve
→ optional rerank → hydrate/provenance → context injection
```

---

## 12. 限制、风险与部署建议

### 12.1 不可信文档进入模型上下文

**已确认：**热/冷 RAG 的结果会插入 messages，而不是只在 UI 展示。  
**风险：**不可信文档若包含“忽略指令”“调用工具”等提示，可能影响模型行为。  
**建议：**将检索文本标注为不可信引用资料；在系统指令中明确其不能覆盖系统、开发者或用户指令；对外部导入材料按来源建立信任边界。

### 12.2 `::Expand` 的上下文膨胀

**已确认：**热/冷的 Expand 路径都可读取完整文件。  
**风险：**多个长文件可能造成模型上下文溢出、成本和延迟异常。  
**建议：**增加全局 `ragMaxInjectedTokens` 与单来源上限；默认返回命中窗口而非全文；按 score/recency 截断并标记已截断。

### 12.3 Embedding 维度与模型变更

**已确认：**热检索会校验向量长度，但模型维度由配置人工指定。  
**建议：**启动时对探针文本 Embedding，并强制校验 `actualDimension === VECTORDB_DIMENSION`；不匹配时拒绝摄取与检索，而不是静默写入不完整索引。

### 12.4 冷库 failed 队列任务

**已确认：**冷库任务重试到上限后为 `failed`，未在当前主路径中看到自动重置。  
**建议：**管理面展示 failed jobs，并提供单文件/批量 retry、错误分类和在配置修复后的重新入队能力。

### 12.5 路径与管理端暴露面

**已确认：**热 RAG formatter 会输出 `file:///` 本地路径；主服务和管理服务均配置了全开放 CORS，body parser 上限为 300 MB。  
**建议：**默认仅输出逻辑来源名，路径仅在可信本地管理模式展示；管理端绑定 localhost/内网并通过反向代理、TLS、IP allowlist 隔离；将 body limit 降至实际需求。

📁 `server.js`  
📍 CORS 与 body parser，546-553  
📁 `adminServer.js`  
📍 CORS 与 body parser，45-50

---

## 13. 核心文件索引与验证清单

### 13.1 文件索引

| 主题 | 优先阅读文件 |
|---|---|
| 主服务与注入顺序 | `server.js` |
| 热库门面与生命周期 | `KnowledgeBaseManager.js` |
| Schema、摄取、检索 | `modules/knowledgeBase/schemaManager.js`、`ingestionPipeline.js`、`searchService.js` |
| watcher、索引和数据库协调 | `fileWatcher.js`、`indexRepository.js`、`databaseCoordinator.js` |
| 分块与 Embedding | `TextChunker.js`、`EmbeddingUtils.js` |
| TagMemo 深度算法 | `TagMemoEngine.js`、`docs/MEMORY_SYSTEM.md` |
| 消息编排和后处理 | `Plugin/RAGDiaryPlugin/RAGDiaryPlugin.js` |
| 注入格式 | `Plugin/RAGDiaryPlugin/RAGResultFormatter.js` |
| 冷库 | `TDBKnowledge.js`、`docs/TDB_COLD_KNOWLEDGE_BASE.md` |
| 调参接口 | `routes/admin/rag.js`、`AdminPanel-Vue/src/views/RagTuning.vue` |

### 13.2 修改或迁移 RAG 后的验证清单

1. **Embedding 合约：**确认 API URL、认证、模型存在、输入输出顺序、失败 `null` 语义和向量维度一致。
2. **摄取：**修改、新增、重命名、复制、删除一份日记，检查 SQLite 的 files/chunks/tags 关系和 Vexus 检索是否同步。
3. **恢复：**删除派生 `.usearch` 后重启或触发搜索，确认可从 SQLite 恢复；确认不会出现持续 Ghost Index。
4. **检索：**分别验证单 diary、多 diary、全局搜索；关闭/开启 TagMemo、BM25、Time、Rerank 的降级行为。
5. **冷库：**验证新文件、改文件、删文件、失败任务重试、`searchHybrid` 回退及 `expand` 上下文大小。
6. **上下文安全：**用带冲突指令的测试文档验证模型系统指令优先级；检查客户端和日志是否意外暴露绝对路径或密钥。
7. **并发与关闭：**文件批量写入、TagMemo 派生、查询并发和 graceful shutdown 下观察 SQLite 锁、pending 队列和索引一致性。

---

## 相关文档

- [MEMORY_SYSTEM.md](./MEMORY_SYSTEM.md)：TagMemo、EPA、残差金字塔与热记忆算法细节。
- [TDB_COLD_KNOWLEDGE_BASE.md](./TDB_COLD_KNOWLEDGE_BASE.md)：冷知识库的独立设计说明。
- [RUST_VECTOR_ENGINE.md](./RUST_VECTOR_ENGINE.md)：Vexus N-API 与向量索引接口。
- [CONFIGURATION.md](./CONFIGURATION.md)：运行时配置语义。
- [OPERATIONS.md](./OPERATIONS.md)：部署与故障排查。

