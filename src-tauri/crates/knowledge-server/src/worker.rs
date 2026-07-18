use ripple_knowledge_ingest::{chunk_text, extract_text, ObjectStore};
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
        if store
            .ingestion_job_cancel_requested(job.id, worker_id)
            .await
            .map_err(|_| ("job_state_failed", true))?
        {
            store
                .cancel_leased_ingestion_job(&job, worker_id)
                .await
                .map_err(|_| ("job_state_failed", true))?;
            return Ok::<(), (&str, bool)>(());
        }
        let bytes = object_store
            .read_bytes(&job.original_object_key, 10 * 1024 * 1024)
            .await
            .map_err(|_| ("object_read_failed", true))?;
        if store
            .ingestion_job_cancel_requested(job.id, worker_id)
            .await
            .map_err(|_| ("job_state_failed", true))?
        {
            store
                .cancel_leased_ingestion_job(&job, worker_id)
                .await
                .map_err(|_| ("job_state_failed", true))?;
            return Ok::<(), (&str, bool)>(());
        }
        let extracted = extract_text(&bytes, &job.mime_type, &job.display_name, 16 * 1024 * 1024)
            .map_err(|error| match error {
            ripple_knowledge_ingest::ExtractionError::UnsupportedType => {
                ("unsupported_document_type", false)
            }
            ripple_knowledge_ingest::ExtractionError::InvalidEncoding => {
                ("invalid_text_encoding", false)
            }
            ripple_knowledge_ingest::ExtractionError::Empty => ("empty_document", false),
            ripple_knowledge_ingest::ExtractionError::TooLarge => {
                ("extracted_text_too_large", false)
            }
        })?;
        let chunks = chunk_text(&extracted.normalized_text, 400, 550);
        if chunks.is_empty() {
            return Err(("empty_document", false));
        }
        store
            .renew_ingestion_lease(job.id, worker_id)
            .await
            .map_err(|_| ("job_state_failed", true))?;
        if store
            .ingestion_job_cancel_requested(job.id, worker_id)
            .await
            .map_err(|_| ("job_state_failed", true))?
        {
            store
                .cancel_leased_ingestion_job(&job, worker_id)
                .await
                .map_err(|_| ("job_state_failed", true))?;
            return Ok::<(), (&str, bool)>(());
        }
        store
            .complete_ingestion_job(
                &job,
                worker_id,
                &extracted.title,
                &extracted.normalized_text,
                extracted.extractor_id,
                extracted.extractor_version,
                &extracted.warnings,
                &extracted.segments,
                &chunks,
            )
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
