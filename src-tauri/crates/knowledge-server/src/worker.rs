use ripple_knowledge_ingest::{chunk_text, ObjectStore};
use ripple_knowledge_store::KnowledgeStore;
use std::{sync::Arc, time::Duration};

pub async fn run_once(
    store: &KnowledgeStore,
    object_store: &impl ObjectStore,
    worker_id: &str,
) -> Result<bool, ripple_knowledge_domain::KnowledgeError> {
    let Some(job) = store.lease_next_ingestion_job(worker_id).await? else {
        return Ok(false);
    };
    let result = async {
        let bytes = object_store
            .read_bytes(&job.original_object_key, 10 * 1024 * 1024)
            .await
            .map_err(|_| ("object_read_failed", true))?;
        let text = String::from_utf8(bytes).map_err(|_| ("invalid_utf8", false))?;
        let normalized = text.replace("\r\n", "\n").trim().to_owned();
        if normalized.is_empty() {
            return Err(("empty_document", false));
        }
        let chunks = chunk_text(&normalized, 400, 550);
        if chunks.is_empty() {
            return Err(("empty_document", false));
        }
        let title = job
            .original_object_key
            .rsplit('/')
            .next()
            .unwrap_or("Untitled");
        store
            .complete_ingestion_job(&job, worker_id, title, &normalized, &chunks)
            .await
            .map_err(|_| ("activation_failed", true))?;
        Ok::<(), (&str, bool)>(())
    }
    .await;
    if let Err((code, retryable)) = result {
        store
            .fail_ingestion_job(&job, worker_id, code, retryable)
            .await?;
    }
    Ok(true)
}

pub fn spawn_worker(
    store: KnowledgeStore,
    object_store: impl ObjectStore + 'static,
    worker_id: String,
) {
    let object_store = Arc::new(object_store);
    tokio::spawn(async move {
        loop {
            match run_once(&store, object_store.as_ref(), &worker_id).await {
                Ok(true) => continue,
                Ok(false) => tokio::time::sleep(Duration::from_millis(500)).await,
                Err(error) => {
                    tracing::warn!(
                        code = error.code(),
                        "knowledge ingestion worker deferred a job"
                    );
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }
    });
}
