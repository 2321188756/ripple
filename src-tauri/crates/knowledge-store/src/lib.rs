//! PostgreSQL persistence and authorization boundary for the Knowledge Service.
//!
//! Repository methods accept validated scopes and never return passwords, raw
//! access credentials, refresh credentials, SQL details, or connection data.

use argon2::{password_hash::SaltString, Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use rand::{rngs::OsRng, RngCore};
use ripple_knowledge_domain::{
    AccessScope, AuditAction, AuditOutcome, CollectionId, CollectionMemberResponse,
    CollectionResponse, CollectionRole, DependencyHealth, DependencyState, GlobalRole,
    IdentityResponse, KnowledgeError, OrganizationId, SessionId, UserId,
};
use sha2::{Digest, Sha256};
use sqlx::{postgres::PgPoolOptions, PgPool, Postgres, Row, Transaction};
use std::time::Duration;
use subtle::ConstantTimeEq;
use uuid::Uuid;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const ACCESS_TOKEN_BYTES: usize = 32;
const REFRESH_TOKEN_BYTES: usize = 32;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[derive(Clone)]
pub struct KnowledgeStore {
    pool: PgPool,
}

#[derive(Clone)]
pub struct AuthConfig {
    pub access_ttl: ChronoDuration,
    pub refresh_ttl: ChronoDuration,
}

pub struct IssuedSession {
    pub access_token: String,
    pub refresh_token: String,
    pub access_expires_at: DateTime<Utc>,
    pub identity: IdentityResponse,
}

#[derive(Debug, Clone)]
pub struct LeasedIngestionJob {
    pub id: Uuid,
    pub source_id: Uuid,
    pub revision_id: Uuid,
    pub collection_id: CollectionId,
    pub original_object_key: String,
    pub mime_type: String,
    pub display_name: String,
    pub correlation_id: Uuid,
}

impl KnowledgeStore {
    pub async fn connect(database_url: &str, max_connections: u32) -> Result<Self, KnowledgeError> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .acquire_timeout(CONNECT_TIMEOUT)
            .connect(database_url)
            .await
            .map_err(|_| KnowledgeError::DatabaseUnavailable)?;
        Ok(Self { pool })
    }

    pub async fn initialize(&self) -> Result<(), KnowledgeError> {
        self.verify_postgres().await?;
        MIGRATOR
            .run(&self.pool)
            .await
            .map_err(|_| KnowledgeError::MigrationIntegrity)
    }

    pub async fn readiness(&self) -> Result<Vec<DependencyHealth>, KnowledgeError> {
        self.verify_postgres().await?;
        self.verify_migration_ledger().await?;
        Ok(vec![
            DependencyHealth {
                name: "postgresql".into(),
                state: DependencyState::Ready,
            },
            DependencyHealth {
                name: "pgvector".into(),
                state: DependencyState::NotConfigured,
            },
            DependencyHealth {
                name: "migration_audit".into(),
                state: DependencyState::Ready,
            },
        ])
    }

    pub async fn bootstrap(
        &self,
        expected_bootstrap_digest: &[u8],
        submitted_bootstrap_digest: &[u8],
        username: &str,
        password: &str,
        device_name: &str,
        config: &AuthConfig,
        request_id: Uuid,
    ) -> Result<IssuedSession, KnowledgeError> {
        validate_username(username)?;
        validate_password(password)?;
        validate_device_name(device_name)?;
        if expected_bootstrap_digest
            .ct_eq(submitted_bootstrap_digest)
            .unwrap_u8()
            != 1
        {
            return Err(KnowledgeError::Unauthenticated);
        }

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|_| KnowledgeError::DatabaseUnavailable)?;
        let existing_admin: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM users WHERE global_role = 'server_admin')",
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| KnowledgeError::Internal)?;
        if existing_admin {
            return Err(KnowledgeError::Conflict);
        }

        let stored = sqlx::query("SELECT bootstrap_token_digest, consumed_at FROM service_bootstrap WHERE singleton = TRUE FOR UPDATE")
            .fetch_optional(&mut *tx).await.map_err(|_| KnowledgeError::Internal)?;
        match stored {
            Some(row) => {
                let stored_digest: Vec<u8> = row
                    .try_get("bootstrap_token_digest")
                    .map_err(|_| KnowledgeError::Internal)?;
                let consumed_at: Option<DateTime<Utc>> = row
                    .try_get("consumed_at")
                    .map_err(|_| KnowledgeError::Internal)?;
                if consumed_at.is_some()
                    || stored_digest.ct_eq(expected_bootstrap_digest).unwrap_u8() != 1
                {
                    return Err(KnowledgeError::Conflict);
                }
            }
            None => {
                sqlx::query("INSERT INTO service_bootstrap (singleton, bootstrap_token_digest) VALUES (TRUE, $1)")
                    .bind(expected_bootstrap_digest).execute(&mut *tx).await.map_err(|_| KnowledgeError::Internal)?;
            }
        }

        let organization_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let password_hash = hash_password(password)?;
        sqlx::query("INSERT INTO organizations (id, name) VALUES ($1, $2)")
            .bind(organization_id)
            .bind("Ripple Knowledge")
            .execute(&mut *tx)
            .await
            .map_err(|_| KnowledgeError::Internal)?;
        sqlx::query("INSERT INTO users (id, organization_id, username_normalized, password_hash, global_role) VALUES ($1, $2, $3, $4, 'server_admin')")
            .bind(user_id).bind(organization_id).bind(normalize_username(username)).bind(password_hash).execute(&mut *tx).await.map_err(|_| KnowledgeError::Conflict)?;
        sqlx::query(
            "UPDATE service_bootstrap SET consumed_at = CURRENT_TIMESTAMP WHERE singleton = TRUE",
        )
        .execute(&mut *tx)
        .await
        .map_err(|_| KnowledgeError::Internal)?;
        let issued = issue_session(
            &mut tx,
            user_id,
            organization_id,
            GlobalRole::ServerAdmin,
            normalize_username(username),
            device_name,
            config,
        )
        .await?;
        audit(
            &mut tx,
            organization_id,
            Some(user_id),
            AuditAction::ServiceBootstrap,
            AuditOutcome::Succeeded,
            "service",
            None,
            request_id,
        )
        .await?;
        tx.commit().await.map_err(|_| KnowledgeError::Internal)?;
        Ok(issued)
    }

    pub async fn login(
        &self,
        username: &str,
        password: &str,
        device_name: &str,
        config: &AuthConfig,
        request_id: Uuid,
    ) -> Result<IssuedSession, KnowledgeError> {
        validate_username(username)?;
        validate_password(password)?;
        validate_device_name(device_name)?;
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|_| KnowledgeError::DatabaseUnavailable)?;
        let row = sqlx::query("SELECT id, organization_id, username_normalized, password_hash, global_role, enabled FROM users WHERE username_normalized = $1")
            .bind(normalize_username(username)).fetch_optional(&mut *tx).await.map_err(|_| KnowledgeError::Internal)?
            .ok_or(KnowledgeError::Unauthenticated)?;
        let enabled: bool = row
            .try_get("enabled")
            .map_err(|_| KnowledgeError::Internal)?;
        let password_hash: String = row
            .try_get("password_hash")
            .map_err(|_| KnowledgeError::Internal)?;
        if !enabled || !verify_password(password, &password_hash) {
            return Err(KnowledgeError::Unauthenticated);
        }
        let user_id: UserId = row.try_get("id").map_err(|_| KnowledgeError::Internal)?;
        let organization_id: OrganizationId = row
            .try_get("organization_id")
            .map_err(|_| KnowledgeError::Internal)?;
        let role = parse_global_role(
            &row.try_get::<String, _>("global_role")
                .map_err(|_| KnowledgeError::Internal)?,
        )?;
        let canonical_username: String = row
            .try_get("username_normalized")
            .map_err(|_| KnowledgeError::Internal)?;
        sqlx::query("UPDATE users SET last_login_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP WHERE id = $1").bind(user_id).execute(&mut *tx).await.map_err(|_| KnowledgeError::Internal)?;
        let issued = issue_session(
            &mut tx,
            user_id,
            organization_id,
            role,
            canonical_username,
            device_name,
            config,
        )
        .await?;
        audit(
            &mut tx,
            organization_id,
            Some(user_id),
            AuditAction::LoginSucceeded,
            AuditOutcome::Succeeded,
            "session",
            Some(issued.identity.session_id),
            request_id,
        )
        .await?;
        tx.commit().await.map_err(|_| KnowledgeError::Internal)?;
        Ok(issued)
    }

    pub async fn refresh(
        &self,
        refresh_token: &str,
        config: &AuthConfig,
        request_id: Uuid,
    ) -> Result<IssuedSession, KnowledgeError> {
        let digest = token_digest(refresh_token);
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|_| KnowledgeError::DatabaseUnavailable)?;
        let row = sqlx::query("SELECT s.id, s.user_id, s.device_name, u.organization_id, u.username_normalized, u.global_role, u.enabled FROM sessions s JOIN users u ON u.id = s.user_id WHERE s.refresh_token_digest = $1 AND s.revoked_at IS NULL AND s.refresh_expires_at > CURRENT_TIMESTAMP FOR UPDATE")
            .bind(digest).fetch_optional(&mut *tx).await.map_err(|_| KnowledgeError::Internal)?.ok_or(KnowledgeError::Unauthenticated)?;
        let enabled: bool = row
            .try_get("enabled")
            .map_err(|_| KnowledgeError::Internal)?;
        if !enabled {
            return Err(KnowledgeError::Unauthenticated);
        }
        let old_session_id: SessionId = row.try_get("id").map_err(|_| KnowledgeError::Internal)?;
        let user_id: UserId = row
            .try_get("user_id")
            .map_err(|_| KnowledgeError::Internal)?;
        let organization_id: OrganizationId = row
            .try_get("organization_id")
            .map_err(|_| KnowledgeError::Internal)?;
        let role = parse_global_role(
            &row.try_get::<String, _>("global_role")
                .map_err(|_| KnowledgeError::Internal)?,
        )?;
        let username: String = row
            .try_get("username_normalized")
            .map_err(|_| KnowledgeError::Internal)?;
        let device_name: String = row
            .try_get("device_name")
            .map_err(|_| KnowledgeError::Internal)?;
        sqlx::query("UPDATE sessions SET revoked_at = CURRENT_TIMESTAMP WHERE id = $1")
            .bind(old_session_id)
            .execute(&mut *tx)
            .await
            .map_err(|_| KnowledgeError::Internal)?;
        let issued = issue_session(
            &mut tx,
            user_id,
            organization_id,
            role,
            username,
            &device_name,
            config,
        )
        .await?;
        audit(
            &mut tx,
            organization_id,
            Some(user_id),
            AuditAction::SessionRefreshed,
            AuditOutcome::Succeeded,
            "session",
            Some(issued.identity.session_id),
            request_id,
        )
        .await?;
        tx.commit().await.map_err(|_| KnowledgeError::Internal)?;
        Ok(issued)
    }

    pub async fn resolve_access_scope(
        &self,
        access_token: &str,
    ) -> Result<(AccessScope, IdentityResponse), KnowledgeError> {
        let row = sqlx::query("SELECT s.id, s.access_expires_at, s.user_id, u.organization_id, u.username_normalized, u.global_role FROM sessions s JOIN users u ON u.id = s.user_id WHERE s.access_token_digest = $1 AND s.revoked_at IS NULL AND s.access_expires_at > CURRENT_TIMESTAMP AND u.enabled = TRUE")
            .bind(token_digest(access_token)).fetch_optional(&self.pool).await.map_err(|_| KnowledgeError::DatabaseUnavailable)?.ok_or(KnowledgeError::Unauthenticated)?;
        let session_id: SessionId = row.try_get("id").map_err(|_| KnowledgeError::Internal)?;
        let user_id: UserId = row
            .try_get("user_id")
            .map_err(|_| KnowledgeError::Internal)?;
        let organization_id: OrganizationId = row
            .try_get("organization_id")
            .map_err(|_| KnowledgeError::Internal)?;
        let role = parse_global_role(
            &row.try_get::<String, _>("global_role")
                .map_err(|_| KnowledgeError::Internal)?,
        )?;
        let identity = IdentityResponse {
            user_id,
            organization_id,
            username: row
                .try_get("username_normalized")
                .map_err(|_| KnowledgeError::Internal)?,
            global_role: role,
            session_id,
            expires_at: row
                .try_get("access_expires_at")
                .map_err(|_| KnowledgeError::Internal)?,
        };
        Ok((
            AccessScope {
                user_id,
                organization_id,
                global_role: role,
                session_id,
            },
            identity,
        ))
    }

    pub async fn resolve_access_scope_from_scope(
        &self,
        scope: &AccessScope,
    ) -> Result<IdentityResponse, KnowledgeError> {
        let row = sqlx::query("SELECT s.access_expires_at, u.username_normalized, u.global_role FROM sessions s JOIN users u ON u.id = s.user_id WHERE s.id = $1 AND s.user_id = $2 AND s.revoked_at IS NULL AND s.access_expires_at > CURRENT_TIMESTAMP AND u.enabled = TRUE")
            .bind(scope.session_id)
            .bind(scope.user_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|_| KnowledgeError::DatabaseUnavailable)?
            .ok_or(KnowledgeError::Unauthenticated)?;
        Ok(IdentityResponse {
            user_id: scope.user_id,
            organization_id: scope.organization_id,
            username: row
                .try_get("username_normalized")
                .map_err(|_| KnowledgeError::Internal)?,
            global_role: parse_global_role(
                &row.try_get::<String, _>("global_role")
                    .map_err(|_| KnowledgeError::Internal)?,
            )?,
            session_id: scope.session_id,
            expires_at: row
                .try_get("access_expires_at")
                .map_err(|_| KnowledgeError::Internal)?,
        })
    }

    pub async fn revoke_session(
        &self,
        scope: &AccessScope,
        request_id: Uuid,
    ) -> Result<(), KnowledgeError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|_| KnowledgeError::DatabaseUnavailable)?;
        sqlx::query("UPDATE sessions SET revoked_at = CURRENT_TIMESTAMP WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL").bind(scope.session_id).bind(scope.user_id).execute(&mut *tx).await.map_err(|_| KnowledgeError::Internal)?;
        audit(
            &mut tx,
            scope.organization_id,
            Some(scope.user_id),
            AuditAction::SessionRevoked,
            AuditOutcome::Succeeded,
            "session",
            Some(scope.session_id),
            request_id,
        )
        .await?;
        tx.commit().await.map_err(|_| KnowledgeError::Internal)
    }

    pub async fn lexical_search(
        &self,
        scope: &AccessScope,
        query: &str,
        collection_ids: &[CollectionId],
        max_results: u32,
        mode: &str,
    ) -> Result<ripple_knowledge_domain::RetrievalResponse, KnowledgeError> {
        if query.trim().is_empty() || query.len() > 4096 || collection_ids.len() > 64 {
            return Err(KnowledgeError::Validation);
        }
        let max_results = max_results.clamp(1, 50) as i32;
        let query_hash = Sha256::digest(query.as_bytes()).to_vec();
        let run_id = Uuid::new_v4();
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|_| KnowledgeError::DatabaseUnavailable)?;
        sqlx::query("INSERT INTO retrieval_runs (id, organization_id, user_id, query_sha256, mode) VALUES ($1,$2,$3,$4,$5)")
            .bind(run_id).bind(scope.organization_id).bind(scope.user_id).bind(query_hash).bind(mode).execute(&mut *tx).await.map_err(|_| KnowledgeError::Internal)?;
        let rows = sqlx::query("SELECT c.id AS chunk_id, c.source_revision_id, c.ordinal, c.content, c.char_start, c.char_end, s.id AS source_id, s.display_name, ts_rank_cd(c.search_vector, websearch_to_tsquery('simple', $1)) AS score FROM chunks c JOIN source_revisions r ON r.id = c.source_revision_id AND r.state = 'ready' JOIN sources s ON s.active_revision_id = r.id AND s.state = 'active' JOIN collection_memberships m ON m.collection_id = s.collection_id AND m.user_id = $2 WHERE s.collection_id = ANY($3) AND s.collection_id IN (SELECT collection_id FROM collection_memberships WHERE user_id = $2) AND c.search_vector @@ websearch_to_tsquery('simple', $1) ORDER BY score DESC, c.id LIMIT $4")
            .bind(query).bind(scope.user_id).bind(collection_ids).bind(max_results).fetch_all(&mut *tx).await.map_err(|_| KnowledgeError::Internal)?;
        let mut hits = Vec::new();
        for (index, row) in rows.iter().enumerate() {
            let chunk_id: Uuid = row
                .try_get("chunk_id")
                .map_err(|_| KnowledgeError::Internal)?;
            let source_id: Uuid = row
                .try_get("source_id")
                .map_err(|_| KnowledgeError::Internal)?;
            let revision_id: Uuid = row
                .try_get("source_revision_id")
                .map_err(|_| KnowledgeError::Internal)?;
            let score: f64 = row.try_get("score").map_err(|_| KnowledgeError::Internal)?;
            sqlx::query("INSERT INTO retrieval_hits (retrieval_run_id, chunk_id, rank, lexical_score, fused_score) VALUES ($1,$2,$3,$4,$4)").bind(run_id).bind(chunk_id).bind(index as i32 + 1).bind(score).execute(&mut *tx).await.map_err(|_| KnowledgeError::Internal)?;
            hits.push(ripple_knowledge_domain::RetrievalHitResponse {
                content: row
                    .try_get("content")
                    .map_err(|_| KnowledgeError::Internal)?,
                score,
                citation: ripple_knowledge_domain::RetrievalCitation {
                    key: format!("cite-{}", index + 1),
                    source_id,
                    revision_id,
                    chunk_id,
                    source_name: row
                        .try_get("display_name")
                        .map_err(|_| KnowledgeError::Internal)?,
                    ordinal: row
                        .try_get("ordinal")
                        .map_err(|_| KnowledgeError::Internal)?,
                    char_start: row
                        .try_get("char_start")
                        .map_err(|_| KnowledgeError::Internal)?,
                    char_end: row
                        .try_get("char_end")
                        .map_err(|_| KnowledgeError::Internal)?,
                },
            });
        }
        sqlx::query("UPDATE retrieval_runs SET result_count = $1 WHERE id = $2")
            .bind(hits.len() as i32)
            .bind(run_id)
            .execute(&mut *tx)
            .await
            .map_err(|_| KnowledgeError::Internal)?;
        tx.commit().await.map_err(|_| KnowledgeError::Internal)?;
        Ok(ripple_knowledge_domain::RetrievalResponse {
            run_id,
            hits,
            degraded: true,
        })
    }

    pub async fn lease_next_ingestion_job(
        &self,
        worker_id: &str,
    ) -> Result<Option<LeasedIngestionJob>, KnowledgeError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|_| KnowledgeError::DatabaseUnavailable)?;
        let row = sqlx::query("SELECT j.id, j.source_id, j.revision_id, j.collection_id, j.correlation_id, s.display_name, r.original_object_key, r.mime_type FROM ingestion_jobs j JOIN source_revisions r ON r.id = j.revision_id JOIN sources s ON s.id = j.source_id WHERE j.kind = 'ingest_revision' AND j.state IN ('queued','retry_scheduled') AND j.next_run_at <= CURRENT_TIMESTAMP AND j.cancel_requested_at IS NULL ORDER BY j.created_at FOR UPDATE SKIP LOCKED LIMIT 1")
            .fetch_optional(&mut *tx).await.map_err(|_| KnowledgeError::Internal)?;
        let Some(row) = row else {
            tx.commit().await.map_err(|_| KnowledgeError::Internal)?;
            return Ok(None);
        };
        let id: Uuid = row.try_get("id").map_err(|_| KnowledgeError::Internal)?;
        sqlx::query("UPDATE ingestion_jobs SET state = 'leased', attempt = attempt + 1, lease_owner = $1, lease_expires_at = CURRENT_TIMESTAMP + INTERVAL '60 seconds', updated_at = CURRENT_TIMESTAMP WHERE id = $2")
            .bind(worker_id).bind(id).execute(&mut *tx).await.map_err(|_| KnowledgeError::Internal)?;
        sqlx::query(
            "UPDATE source_revisions SET state = 'processing' WHERE id = $1 AND state = 'pending'",
        )
        .bind(
            row.try_get::<Uuid, _>("revision_id")
                .map_err(|_| KnowledgeError::Internal)?,
        )
        .execute(&mut *tx)
        .await
        .map_err(|_| KnowledgeError::Internal)?;
        tx.commit().await.map_err(|_| KnowledgeError::Internal)?;
        Ok(Some(LeasedIngestionJob {
            id,
            source_id: row
                .try_get("source_id")
                .map_err(|_| KnowledgeError::Internal)?,
            revision_id: row
                .try_get("revision_id")
                .map_err(|_| KnowledgeError::Internal)?,
            collection_id: row
                .try_get("collection_id")
                .map_err(|_| KnowledgeError::Internal)?,
            original_object_key: row
                .try_get("original_object_key")
                .map_err(|_| KnowledgeError::Internal)?,
            mime_type: row
                .try_get("mime_type")
                .map_err(|_| KnowledgeError::Internal)?,
            display_name: row
                .try_get("display_name")
                .map_err(|_| KnowledgeError::Internal)?,
            correlation_id: row
                .try_get("correlation_id")
                .map_err(|_| KnowledgeError::Internal)?,
        }))
    }

    pub async fn renew_ingestion_lease(
        &self,
        job_id: Uuid,
        worker_id: &str,
    ) -> Result<(), KnowledgeError> {
        let result = sqlx::query("UPDATE ingestion_jobs SET lease_expires_at = CURRENT_TIMESTAMP + INTERVAL '60 seconds', updated_at = CURRENT_TIMESTAMP WHERE id = $1 AND state IN ('leased','running') AND lease_owner = $2 AND lease_expires_at > CURRENT_TIMESTAMP AND cancel_requested_at IS NULL")
            .bind(job_id)
            .bind(worker_id)
            .execute(&self.pool)
            .await
            .map_err(|_| KnowledgeError::DatabaseUnavailable)?;
        if result.rows_affected() != 1 {
            return Err(KnowledgeError::Conflict);
        }
        Ok(())
    }

    pub async fn ingestion_job_cancel_requested(
        &self,
        job_id: Uuid,
        worker_id: &str,
    ) -> Result<bool, KnowledgeError> {
        sqlx::query_scalar("SELECT cancel_requested_at IS NOT NULL FROM ingestion_jobs WHERE id = $1 AND state IN ('leased','running') AND lease_owner = $2")
            .bind(job_id)
            .bind(worker_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|_| KnowledgeError::DatabaseUnavailable)?
            .ok_or(KnowledgeError::Conflict)
    }

    pub async fn cancel_leased_ingestion_job(
        &self,
        job: &LeasedIngestionJob,
        worker_id: &str,
    ) -> Result<(), KnowledgeError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|_| KnowledgeError::DatabaseUnavailable)?;
        let result = sqlx::query("UPDATE ingestion_jobs SET state = 'cancelled', lease_owner = NULL, lease_expires_at = NULL, updated_at = CURRENT_TIMESTAMP WHERE id = $1 AND state IN ('leased','running') AND lease_owner = $2 AND cancel_requested_at IS NOT NULL")
            .bind(job.id)
            .bind(worker_id)
            .execute(&mut *tx)
            .await
            .map_err(|_| KnowledgeError::Internal)?;
        if result.rows_affected() != 1 {
            return Err(KnowledgeError::Conflict);
        }
        sqlx::query("UPDATE source_revisions SET state = 'cancelled', completed_at = CURRENT_TIMESTAMP WHERE id = $1 AND state = 'processing'")
            .bind(job.revision_id)
            .execute(&mut *tx)
            .await
            .map_err(|_| KnowledgeError::Internal)?;
        sqlx::query("INSERT INTO ingestion_job_events (id, job_id, state, correlation_id) VALUES ($1,$2,'cancelled',$3)")
            .bind(Uuid::new_v4())
            .bind(job.id)
            .bind(job.correlation_id)
            .execute(&mut *tx)
            .await
            .map_err(|_| KnowledgeError::Internal)?;
        tx.commit().await.map_err(|_| KnowledgeError::Internal)
    }

    pub async fn fail_ingestion_job(
        &self,
        job: &LeasedIngestionJob,
        worker_id: &str,
        error_code: &str,
        retryable: bool,
    ) -> Result<(), KnowledgeError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|_| KnowledgeError::DatabaseUnavailable)?;
        let row = sqlx::query("SELECT attempt, max_attempts, correlation_id FROM ingestion_jobs WHERE id = $1 AND state = 'leased' AND lease_owner = $2 FOR UPDATE")
            .bind(job.id).bind(worker_id).fetch_optional(&mut *tx).await.map_err(|_| KnowledgeError::Internal)?.ok_or(KnowledgeError::Conflict)?;
        let attempt: i32 = row
            .try_get("attempt")
            .map_err(|_| KnowledgeError::Internal)?;
        let max_attempts: i32 = row
            .try_get("max_attempts")
            .map_err(|_| KnowledgeError::Internal)?;
        let correlation_id: Uuid = row
            .try_get("correlation_id")
            .map_err(|_| KnowledgeError::Internal)?;
        let next_state = if retryable && attempt < max_attempts {
            "retry_scheduled"
        } else {
            "failed"
        };
        sqlx::query("UPDATE ingestion_jobs SET state = $1, error_code = $2, lease_owner = NULL, lease_expires_at = NULL, next_run_at = CURRENT_TIMESTAMP + (INTERVAL '2 seconds' * POWER(2, LEAST(attempt, 8))), updated_at = CURRENT_TIMESTAMP WHERE id = $3")
            .bind(next_state).bind(error_code).bind(job.id).execute(&mut *tx).await.map_err(|_| KnowledgeError::Internal)?;
        if next_state == "failed" {
            sqlx::query("UPDATE source_revisions SET state = 'failed' WHERE id = $1")
                .bind(job.revision_id)
                .execute(&mut *tx)
                .await
                .map_err(|_| KnowledgeError::Internal)?;
        }
        sqlx::query("INSERT INTO ingestion_job_events (id, job_id, state, error_code, correlation_id) VALUES ($1,$2,$3,$4,$5)")
            .bind(Uuid::new_v4()).bind(job.id).bind(next_state).bind(error_code).bind(correlation_id).execute(&mut *tx).await.map_err(|_| KnowledgeError::Internal)?;
        tx.commit().await.map_err(|_| KnowledgeError::Internal)
    }

    pub async fn recover_expired_ingestion_leases(&self) -> Result<u64, KnowledgeError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|_| KnowledgeError::DatabaseUnavailable)?;
        let rows = sqlx::query("SELECT id, revision_id, attempt, max_attempts, correlation_id, cancel_requested_at IS NOT NULL AS cancelled FROM ingestion_jobs WHERE state IN ('leased','running') AND lease_expires_at < CURRENT_TIMESTAMP FOR UPDATE")
            .fetch_all(&mut *tx)
            .await
            .map_err(|_| KnowledgeError::Internal)?;
        for row in &rows {
            let job_id: Uuid = row.try_get("id").map_err(|_| KnowledgeError::Internal)?;
            let revision_id: Option<Uuid> = row
                .try_get("revision_id")
                .map_err(|_| KnowledgeError::Internal)?;
            let attempt: i32 = row
                .try_get("attempt")
                .map_err(|_| KnowledgeError::Internal)?;
            let max_attempts: i32 = row
                .try_get("max_attempts")
                .map_err(|_| KnowledgeError::Internal)?;
            let cancelled: bool = row
                .try_get("cancelled")
                .map_err(|_| KnowledgeError::Internal)?;
            let correlation_id: Uuid = row
                .try_get("correlation_id")
                .map_err(|_| KnowledgeError::Internal)?;
            let next_state = if cancelled {
                "cancelled"
            } else if attempt < max_attempts {
                "retry_scheduled"
            } else {
                "failed"
            };
            sqlx::query("UPDATE ingestion_jobs SET state = $1, error_code = $2, lease_owner = NULL, lease_expires_at = NULL, next_run_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP WHERE id = $3")
                .bind(next_state)
                .bind((next_state != "cancelled").then_some("lease_expired"))
                .bind(job_id)
                .execute(&mut *tx)
                .await
                .map_err(|_| KnowledgeError::Internal)?;
            if matches!(next_state, "failed" | "cancelled") {
                sqlx::query("UPDATE source_revisions SET state = $1, completed_at = CURRENT_TIMESTAMP WHERE id = $2 AND state = 'processing'")
                    .bind(next_state)
                    .bind(revision_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|_| KnowledgeError::Internal)?;
            }
            sqlx::query("INSERT INTO ingestion_job_events (id, job_id, state, error_code, correlation_id) VALUES ($1,$2,$3,$4,$5)")
                .bind(Uuid::new_v4())
                .bind(job_id)
                .bind(next_state)
                .bind((next_state != "cancelled").then_some("lease_expired"))
                .bind(correlation_id)
                .execute(&mut *tx)
                .await
                .map_err(|_| KnowledgeError::Internal)?;
        }
        tx.commit().await.map_err(|_| KnowledgeError::Internal)?;
        Ok(rows.len() as u64)
    }

    pub async fn complete_ingestion_job(
        &self,
        job: &LeasedIngestionJob,
        worker_id: &str,
        title: &str,
        normalized_text: &str,
        extractor_id: &str,
        extractor_version: &str,
        warnings: &[&str],
        extracted_segments: &[ripple_knowledge_ingest::DocumentSegment],
        chunks: &[ripple_knowledge_ingest::TextChunk],
    ) -> Result<(), KnowledgeError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|_| KnowledgeError::DatabaseUnavailable)?;
        let owned: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM ingestion_jobs WHERE id = $1 AND state = 'leased' AND lease_owner = $2 AND lease_expires_at > CURRENT_TIMESTAMP AND cancel_requested_at IS NULL)")
            .bind(job.id).bind(worker_id).fetch_one(&mut *tx).await.map_err(|_| KnowledgeError::Internal)?;
        if !owned {
            return Err(KnowledgeError::Conflict);
        }
        let document_id = Uuid::new_v4();
        sqlx::query("INSERT INTO documents (id, source_revision_id, title, normalized_text) VALUES ($1,$2,$3,$4)")
            .bind(document_id)
            .bind(job.revision_id)
            .bind(title)
            .bind(normalized_text)
            .execute(&mut *tx)
            .await
            .map_err(|_| KnowledgeError::Internal)?;
        sqlx::query("UPDATE source_revisions SET extractor_id = $1, extractor_version = $2, warnings = $3 WHERE id = $4")
            .bind(extractor_id)
            .bind(extractor_version)
            .bind(serde_json::to_value(warnings).map_err(|_| KnowledgeError::Internal)?)
            .bind(job.revision_id)
            .execute(&mut *tx)
            .await
            .map_err(|_| KnowledgeError::Internal)?;
        for segment in extracted_segments {
            sqlx::query("INSERT INTO document_segments (id, document_id, source_revision_id, ordinal, char_start, char_end, line_start, line_end, heading_path) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)")
                .bind(Uuid::new_v4())
                .bind(document_id)
                .bind(job.revision_id)
                .bind(segment.ordinal)
                .bind(segment.char_start)
                .bind(segment.char_end)
                .bind(segment.line_start)
                .bind(segment.line_end)
                .bind(serde_json::to_value(&segment.heading_path).map_err(|_| KnowledgeError::Internal)?)
                .execute(&mut *tx)
                .await
                .map_err(|_| KnowledgeError::Internal)?;
        }
        let chunk_ids: Vec<Uuid> = chunks.iter().map(|_| Uuid::new_v4()).collect();
        for (index, chunk) in chunks.iter().enumerate() {
            let predecessor = index.checked_sub(1).map(|previous| chunk_ids[previous]);
            let successor = (index + 1 < chunk_ids.len()).then_some(chunk_ids[index + 1]);
            sqlx::query("INSERT INTO chunks (id, document_id, source_revision_id, ordinal, content, text_sha256, token_count, char_start, char_end, predecessor_id, successor_id) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)")
                .bind(chunk_ids[index]).bind(document_id).bind(job.revision_id).bind(chunk.ordinal).bind(&chunk.content).bind(Sha256::digest(chunk.content.as_bytes()).to_vec()).bind(chunk.token_count).bind(chunk.char_start).bind(chunk.char_end).bind(predecessor).bind(successor).execute(&mut *tx).await.map_err(|_| KnowledgeError::Internal)?;
        }
        sqlx::query("UPDATE source_revisions SET state = 'ready', completed_at = CURRENT_TIMESTAMP WHERE id = $1").bind(job.revision_id).execute(&mut *tx).await.map_err(|_| KnowledgeError::Internal)?;
        sqlx::query("UPDATE sources SET active_revision_id = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2").bind(job.revision_id).bind(job.source_id).execute(&mut *tx).await.map_err(|_| KnowledgeError::Internal)?;
        sqlx::query("UPDATE ingestion_jobs SET state = 'succeeded', progress_current = COALESCE(progress_total, progress_current), lease_owner = NULL, lease_expires_at = NULL, error_code = NULL, updated_at = CURRENT_TIMESTAMP WHERE id = $1 AND state = 'leased' AND lease_owner = $2 AND lease_expires_at > CURRENT_TIMESTAMP AND cancel_requested_at IS NULL")
            .bind(job.id)
            .bind(worker_id)
            .execute(&mut *tx)
            .await
            .map_err(|_| KnowledgeError::Internal)?;
        sqlx::query("INSERT INTO ingestion_job_events (id, job_id, state, progress_current, progress_total, correlation_id) SELECT $1, id, 'succeeded', progress_total, progress_total, correlation_id FROM ingestion_jobs WHERE id = $2")
            .bind(Uuid::new_v4())
            .bind(job.id)
            .execute(&mut *tx)
            .await
            .map_err(|_| KnowledgeError::Internal)?;
        tx.commit().await.map_err(|_| KnowledgeError::Internal)
    }

    pub async fn list_ingestion_jobs(
        &self,
        scope: &AccessScope,
        collection_id: Option<CollectionId>,
        state: Option<&str>,
        limit: u32,
    ) -> Result<Vec<ripple_knowledge_domain::IngestionJobResponse>, KnowledgeError> {
        let limit = limit.clamp(1, 100) as i64;
        let rows = sqlx::query("SELECT j.id, j.source_id, j.revision_id, j.state, j.progress_current, j.progress_total, j.created_at FROM ingestion_jobs j JOIN collection_memberships m ON m.collection_id = j.collection_id AND m.user_id = $1 WHERE j.collection_id = COALESCE($2, j.collection_id) AND j.state = COALESCE($3, j.state) ORDER BY j.created_at DESC LIMIT $4")
            .bind(scope.user_id).bind(collection_id).bind(state).bind(limit).fetch_all(&self.pool).await.map_err(|_| KnowledgeError::DatabaseUnavailable)?;
        rows.into_iter()
            .map(|row| {
                Ok(ripple_knowledge_domain::IngestionJobResponse {
                    id: row.try_get("id").map_err(|_| KnowledgeError::Internal)?,
                    source_id: row
                        .try_get("source_id")
                        .map_err(|_| KnowledgeError::Internal)?,
                    revision_id: row
                        .try_get("revision_id")
                        .map_err(|_| KnowledgeError::Internal)?,
                    state: row.try_get("state").map_err(|_| KnowledgeError::Internal)?,
                    progress_current: row
                        .try_get("progress_current")
                        .map_err(|_| KnowledgeError::Internal)?,
                    progress_total: row
                        .try_get("progress_total")
                        .map_err(|_| KnowledgeError::Internal)?,
                    created_at: row
                        .try_get("created_at")
                        .map_err(|_| KnowledgeError::Internal)?,
                })
            })
            .collect()
    }

    pub async fn request_cancel_job(
        &self,
        scope: &AccessScope,
        job_id: Uuid,
    ) -> Result<(), KnowledgeError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|_| KnowledgeError::DatabaseUnavailable)?;
        let row = sqlx::query("SELECT j.revision_id, j.state, j.correlation_id FROM ingestion_jobs j JOIN collection_memberships m ON m.collection_id = j.collection_id AND m.user_id = $2 WHERE j.id = $1 AND j.state IN ('queued','leased','running','retry_scheduled') AND (m.role IN ('collection_admin','editor') OR $3) FOR UPDATE")
            .bind(job_id)
            .bind(scope.user_id)
            .bind(scope.is_server_admin())
            .fetch_optional(&mut *tx)
            .await
            .map_err(|_| KnowledgeError::Internal)?
            .ok_or(KnowledgeError::NotFound)?;
        let revision_id: Option<Uuid> = row
            .try_get("revision_id")
            .map_err(|_| KnowledgeError::Internal)?;
        let state: String = row.try_get("state").map_err(|_| KnowledgeError::Internal)?;
        let correlation_id: Uuid = row
            .try_get("correlation_id")
            .map_err(|_| KnowledgeError::Internal)?;
        let immediate = matches!(state.as_str(), "queued" | "retry_scheduled");
        sqlx::query("UPDATE ingestion_jobs SET cancel_requested_at = CURRENT_TIMESTAMP, state = CASE WHEN $1 THEN 'cancelled' ELSE state END, updated_at = CURRENT_TIMESTAMP WHERE id = $2")
            .bind(immediate)
            .bind(job_id)
            .execute(&mut *tx)
            .await
            .map_err(|_| KnowledgeError::Internal)?;
        if immediate {
            sqlx::query("UPDATE source_revisions SET state = 'cancelled', completed_at = CURRENT_TIMESTAMP WHERE id = $1 AND state IN ('pending','processing')")
                .bind(revision_id)
                .execute(&mut *tx)
                .await
                .map_err(|_| KnowledgeError::Internal)?;
            sqlx::query("INSERT INTO ingestion_job_events (id, job_id, state, correlation_id) VALUES ($1,$2,'cancelled',$3)")
                .bind(Uuid::new_v4())
                .bind(job_id)
                .bind(correlation_id)
                .execute(&mut *tx)
                .await
                .map_err(|_| KnowledgeError::Internal)?;
        }
        tx.commit().await.map_err(|_| KnowledgeError::Internal)
    }

    pub async fn create_upload_source(
        &self,
        scope: &AccessScope,
        collection_id: CollectionId,
        display_name: &str,
        mime_type: &str,
        object_key: &str,
        sha256: &[u8; 32],
        byte_size: u64,
        request_id: Uuid,
    ) -> Result<
        (
            ripple_knowledge_domain::SourceResponse,
            ripple_knowledge_domain::IngestionJobResponse,
        ),
        KnowledgeError,
    > {
        self.require_collection_role(scope, collection_id, false)
            .await?;
        if display_name.trim().is_empty()
            || display_name.len() > 512
            || mime_type.trim().is_empty()
            || mime_type.len() > 128
        {
            return Err(KnowledgeError::Validation);
        }
        let collection_role = self.collection_role(scope, collection_id).await?;
        if !scope.is_server_admin()
            && !matches!(
                collection_role,
                CollectionRole::CollectionAdmin | CollectionRole::Editor
            )
        {
            return Err(KnowledgeError::Forbidden);
        }
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|_| KnowledgeError::DatabaseUnavailable)?;
        sqlx::query("INSERT INTO object_blobs (object_key, organization_id, sha256, byte_size) VALUES ($1,$2,$3,$4) ON CONFLICT (object_key) DO NOTHING")
            .bind(object_key)
            .bind(scope.organization_id)
            .bind(sha256.as_slice())
            .bind(byte_size as i64)
            .execute(&mut *tx)
            .await
            .map_err(|_| KnowledgeError::Internal)?;
        let identity_key = normalize_source_identity(display_name);
        let source_lock_key = format!("{collection_id}:upload:{identity_key}");
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(source_lock_key)
            .execute(&mut *tx)
            .await
            .map_err(|_| KnowledgeError::Internal)?;
        let existing_source = sqlx::query(
            "SELECT id, active_revision_id, state, created_at FROM sources WHERE collection_id = $1 AND kind = 'upload' AND identity_key = $2 FOR UPDATE",
        )
        .bind(collection_id)
        .bind(&identity_key)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| KnowledgeError::Internal)?;

        let (source_id, active_revision_id, source_state, source_created_at) = match existing_source
        {
            Some(row) => (
                row.try_get("id").map_err(|_| KnowledgeError::Internal)?,
                row.try_get("active_revision_id")
                    .map_err(|_| KnowledgeError::Internal)?,
                row.try_get::<String, _>("state")
                    .map_err(|_| KnowledgeError::Internal)?,
                row.try_get("created_at")
                    .map_err(|_| KnowledgeError::Internal)?,
            ),
            None => {
                let source_id = Uuid::new_v4();
                let created_at: DateTime<Utc> = sqlx::query_scalar("INSERT INTO sources (id, collection_id, kind, identity_key, display_name, state, created_by) VALUES ($1,$2,'upload',$3,$4,'active',$5) RETURNING created_at")
                    .bind(source_id)
                    .bind(collection_id)
                    .bind(&identity_key)
                    .bind(display_name.trim())
                    .bind(scope.user_id)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(|_| KnowledgeError::Conflict)?;
                (source_id, None, "active".to_owned(), created_at)
            }
        };

        if source_state == "deleted" {
            return Err(KnowledgeError::Conflict);
        }
        sqlx::query(
            "UPDATE sources SET display_name = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2",
        )
        .bind(display_name.trim())
        .bind(source_id)
        .execute(&mut *tx)
        .await
        .map_err(|_| KnowledgeError::Internal)?;

        let existing_revision = sqlx::query(
            "SELECT id, state FROM source_revisions WHERE source_id = $1 AND content_sha256 = $2",
        )
        .bind(source_id)
        .bind(sha256.as_slice())
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| KnowledgeError::Internal)?;

        let (revision_id, job_id, job_state, progress_current, progress_total, job_created_at) =
            match existing_revision {
                Some(row) => {
                    let revision_id: Uuid =
                        row.try_get("id").map_err(|_| KnowledgeError::Internal)?;
                    let existing_job = sqlx::query("SELECT id, state, progress_current, progress_total, created_at FROM ingestion_jobs WHERE revision_id = $1 ORDER BY created_at DESC LIMIT 1")
                        .bind(revision_id)
                        .fetch_optional(&mut *tx)
                        .await
                        .map_err(|_| KnowledgeError::Internal)?
                        .ok_or(KnowledgeError::Internal)?;
                    (
                        revision_id,
                        existing_job
                            .try_get("id")
                            .map_err(|_| KnowledgeError::Internal)?,
                        existing_job
                            .try_get("state")
                            .map_err(|_| KnowledgeError::Internal)?,
                        existing_job
                            .try_get("progress_current")
                            .map_err(|_| KnowledgeError::Internal)?,
                        existing_job
                            .try_get("progress_total")
                            .map_err(|_| KnowledgeError::Internal)?,
                        existing_job
                            .try_get("created_at")
                            .map_err(|_| KnowledgeError::Internal)?,
                    )
                }
                None => {
                    let revision_id = Uuid::new_v4();
                    let job_id = Uuid::new_v4();
                    let created_at = Utc::now();
                    let mut dedupe_material = Vec::with_capacity(48);
                    dedupe_material.extend_from_slice(source_id.as_bytes());
                    dedupe_material.extend_from_slice(sha256);
                    let dedupe_key = Sha256::digest(dedupe_material).to_vec();
                    sqlx::query("INSERT INTO source_revisions (id, source_id, content_sha256, byte_size, mime_type, original_object_key, state) VALUES ($1,$2,$3,$4,$5,$6,'pending')")
                        .bind(revision_id)
                        .bind(source_id)
                        .bind(sha256.as_slice())
                        .bind(byte_size as i64)
                        .bind(mime_type.trim())
                        .bind(object_key)
                        .execute(&mut *tx)
                        .await
                        .map_err(|_| KnowledgeError::Internal)?;
                    sqlx::query("INSERT INTO ingestion_jobs (id, collection_id, source_id, revision_id, kind, dedupe_key, state, correlation_id, progress_total) VALUES ($1,$2,$3,$4,'ingest_revision',$5,'queued',$6,$7)")
                        .bind(job_id)
                        .bind(collection_id)
                        .bind(source_id)
                        .bind(revision_id)
                        .bind(dedupe_key)
                        .bind(request_id)
                        .bind(byte_size as i64)
                        .execute(&mut *tx)
                        .await
                        .map_err(|_| KnowledgeError::Internal)?;
                    sqlx::query("INSERT INTO ingestion_job_events (id, job_id, state, progress_current, progress_total, correlation_id) VALUES ($1,$2,'queued',0,$3,$4)")
                        .bind(Uuid::new_v4())
                        .bind(job_id)
                        .bind(byte_size as i64)
                        .bind(request_id)
                        .execute(&mut *tx)
                        .await
                        .map_err(|_| KnowledgeError::Internal)?;
                    (
                        revision_id,
                        job_id,
                        "queued".to_owned(),
                        0,
                        Some(byte_size as i64),
                        created_at,
                    )
                }
            };
        audit(
            &mut tx,
            scope.organization_id,
            Some(scope.user_id),
            AuditAction::SourceCreated,
            AuditOutcome::Succeeded,
            "source",
            Some(source_id),
            request_id,
        )
        .await?;
        tx.commit().await.map_err(|_| KnowledgeError::Internal)?;
        Ok((
            ripple_knowledge_domain::SourceResponse {
                id: source_id,
                collection_id,
                kind: "upload".into(),
                display_name: display_name.trim().into(),
                active_revision_id,
                state: source_state,
                created_at: source_created_at,
            },
            ripple_knowledge_domain::IngestionJobResponse {
                id: job_id,
                source_id: Some(source_id),
                revision_id: Some(revision_id),
                state: job_state,
                progress_current,
                progress_total,
                created_at: job_created_at,
            },
        ))
    }

    pub async fn create_user(
        &self,
        scope: &AccessScope,
        username: &str,
        password: &str,
        request_id: Uuid,
    ) -> Result<ripple_knowledge_domain::UserResponse, KnowledgeError> {
        if !scope.is_server_admin() {
            return Err(KnowledgeError::Forbidden);
        }
        validate_username(username)?;
        validate_password(password)?;
        let id = Uuid::new_v4();
        let normalized = normalize_username(username);
        let password_hash = hash_password(password)?;
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|_| KnowledgeError::DatabaseUnavailable)?;
        let created_at: DateTime<Utc> = sqlx::query_scalar("INSERT INTO users (id, organization_id, username_normalized, password_hash, global_role) VALUES ($1, $2, $3, $4, 'user') RETURNING created_at")
            .bind(id).bind(scope.organization_id).bind(&normalized).bind(password_hash).fetch_one(&mut *tx).await.map_err(|_| KnowledgeError::Conflict)?;
        audit(
            &mut tx,
            scope.organization_id,
            Some(scope.user_id),
            AuditAction::UserCreated,
            AuditOutcome::Succeeded,
            "user",
            Some(id),
            request_id,
        )
        .await?;
        tx.commit().await.map_err(|_| KnowledgeError::Internal)?;
        Ok(ripple_knowledge_domain::UserResponse {
            id,
            organization_id: scope.organization_id,
            username: normalized,
            global_role: GlobalRole::User,
            created_at,
        })
    }

    pub async fn create_collection(
        &self,
        scope: &AccessScope,
        name: &str,
        description: Option<&str>,
        request_id: Uuid,
    ) -> Result<CollectionResponse, KnowledgeError> {
        if !scope.is_server_admin() {
            return Err(KnowledgeError::Forbidden);
        }
        validate_collection(name, description)?;
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|_| KnowledgeError::DatabaseUnavailable)?;
        let id = Uuid::new_v4();
        let description = description.unwrap_or("").trim();
        sqlx::query("INSERT INTO collections (id, organization_id, name, description, created_by) VALUES ($1, $2, $3, $4, $5)").bind(id).bind(scope.organization_id).bind(name.trim()).bind(description).bind(scope.user_id).execute(&mut *tx).await.map_err(|_| KnowledgeError::Conflict)?;
        sqlx::query("INSERT INTO collection_memberships (collection_id, user_id, role) VALUES ($1, $2, 'collection_admin')").bind(id).bind(scope.user_id).execute(&mut *tx).await.map_err(|_| KnowledgeError::Internal)?;
        audit(
            &mut tx,
            scope.organization_id,
            Some(scope.user_id),
            AuditAction::CollectionCreated,
            AuditOutcome::Succeeded,
            "collection",
            Some(id),
            request_id,
        )
        .await?;
        let response = CollectionResponse {
            id,
            organization_id: scope.organization_id,
            name: name.trim().to_owned(),
            description: description.to_owned(),
            role: CollectionRole::CollectionAdmin,
            created_at: Utc::now(),
        };
        tx.commit().await.map_err(|_| KnowledgeError::Internal)?;
        Ok(response)
    }

    pub async fn list_collections(
        &self,
        scope: &AccessScope,
    ) -> Result<Vec<CollectionResponse>, KnowledgeError> {
        let rows = sqlx::query("SELECT c.id, c.organization_id, c.name, c.description, c.created_at, COALESCE(m.role, 'collection_admin') AS role FROM collections c LEFT JOIN collection_memberships m ON m.collection_id = c.id AND m.user_id = $1 WHERE c.organization_id = $2 AND c.deleted_at IS NULL AND ($3 = TRUE OR m.user_id IS NOT NULL) ORDER BY c.created_at DESC")
            .bind(scope.user_id).bind(scope.organization_id).bind(scope.is_server_admin()).fetch_all(&self.pool).await.map_err(|_| KnowledgeError::DatabaseUnavailable)?;
        rows.into_iter().map(collection_from_row).collect()
    }

    pub async fn list_members(
        &self,
        scope: &AccessScope,
        collection_id: CollectionId,
    ) -> Result<Vec<CollectionMemberResponse>, KnowledgeError> {
        self.require_collection_role(scope, collection_id, false)
            .await?;
        let rows = sqlx::query("SELECT u.id, u.username_normalized, m.role, m.created_at FROM collection_memberships m JOIN users u ON u.id = m.user_id JOIN collections c ON c.id = m.collection_id WHERE m.collection_id = $1 AND c.organization_id = $2 ORDER BY u.username_normalized")
            .bind(collection_id).bind(scope.organization_id).fetch_all(&self.pool).await.map_err(|_| KnowledgeError::DatabaseUnavailable)?;
        rows.into_iter()
            .map(|r| {
                Ok(CollectionMemberResponse {
                    user_id: r.try_get("id").map_err(|_| KnowledgeError::Internal)?,
                    username: r
                        .try_get("username_normalized")
                        .map_err(|_| KnowledgeError::Internal)?,
                    role: parse_collection_role(
                        &r.try_get::<String, _>("role")
                            .map_err(|_| KnowledgeError::Internal)?,
                    )?,
                    created_at: r
                        .try_get("created_at")
                        .map_err(|_| KnowledgeError::Internal)?,
                })
            })
            .collect()
    }

    pub async fn upsert_member(
        &self,
        scope: &AccessScope,
        collection_id: CollectionId,
        user_id: UserId,
        role: CollectionRole,
        request_id: Uuid,
    ) -> Result<(), KnowledgeError> {
        self.require_collection_role(scope, collection_id, true)
            .await?;
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|_| KnowledgeError::DatabaseUnavailable)?;
        let user_exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id = $1 AND organization_id = $2 AND enabled = TRUE)").bind(user_id).bind(scope.organization_id).fetch_one(&mut *tx).await.map_err(|_| KnowledgeError::Internal)?;
        if !user_exists {
            return Err(KnowledgeError::NotFound);
        }
        sqlx::query("INSERT INTO collection_memberships (collection_id, user_id, role) VALUES ($1, $2, $3) ON CONFLICT (collection_id, user_id) DO UPDATE SET role = EXCLUDED.role, updated_at = CURRENT_TIMESTAMP").bind(collection_id).bind(user_id).bind(role.as_str()).execute(&mut *tx).await.map_err(|_| KnowledgeError::Internal)?;
        audit(
            &mut tx,
            scope.organization_id,
            Some(scope.user_id),
            AuditAction::CollectionMemberGranted,
            AuditOutcome::Succeeded,
            "collection_member",
            Some(user_id),
            request_id,
        )
        .await?;
        tx.commit().await.map_err(|_| KnowledgeError::Internal)
    }

    pub async fn delete_member(
        &self,
        scope: &AccessScope,
        collection_id: CollectionId,
        user_id: UserId,
        request_id: Uuid,
    ) -> Result<(), KnowledgeError> {
        self.require_collection_role(scope, collection_id, true)
            .await?;
        if user_id == scope.user_id {
            return Err(KnowledgeError::Conflict);
        }
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|_| KnowledgeError::DatabaseUnavailable)?;
        let result = sqlx::query(
            "DELETE FROM collection_memberships WHERE collection_id = $1 AND user_id = $2",
        )
        .bind(collection_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(|_| KnowledgeError::Internal)?;
        if result.rows_affected() == 0 {
            return Err(KnowledgeError::NotFound);
        }
        audit(
            &mut tx,
            scope.organization_id,
            Some(scope.user_id),
            AuditAction::CollectionMemberRemoved,
            AuditOutcome::Succeeded,
            "collection_member",
            Some(user_id),
            request_id,
        )
        .await?;
        tx.commit().await.map_err(|_| KnowledgeError::Internal)
    }

    async fn collection_role(
        &self,
        scope: &AccessScope,
        collection_id: CollectionId,
    ) -> Result<CollectionRole, KnowledgeError> {
        if scope.is_server_admin() {
            return Ok(CollectionRole::CollectionAdmin);
        }
        let role: Option<String> = sqlx::query_scalar("SELECT m.role FROM collection_memberships m JOIN collections c ON c.id = m.collection_id WHERE m.collection_id = $1 AND m.user_id = $2 AND c.organization_id = $3 AND c.deleted_at IS NULL")
            .bind(collection_id)
            .bind(scope.user_id)
            .bind(scope.organization_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|_| KnowledgeError::DatabaseUnavailable)?;
        role.as_deref()
            .map(parse_collection_role)
            .transpose()?
            .ok_or(KnowledgeError::NotFound)
    }

    async fn require_collection_role(
        &self,
        scope: &AccessScope,
        collection_id: CollectionId,
        manage: bool,
    ) -> Result<(), KnowledgeError> {
        if scope.is_server_admin() {
            return Ok(());
        }
        let role: Option<String> = sqlx::query_scalar("SELECT m.role FROM collection_memberships m JOIN collections c ON c.id = m.collection_id WHERE m.collection_id = $1 AND m.user_id = $2 AND c.organization_id = $3 AND c.deleted_at IS NULL").bind(collection_id).bind(scope.user_id).bind(scope.organization_id).fetch_optional(&self.pool).await.map_err(|_| KnowledgeError::DatabaseUnavailable)?;
        let role = role.ok_or(KnowledgeError::NotFound)?;
        if manage && parse_collection_role(&role)? != CollectionRole::CollectionAdmin {
            return Err(KnowledgeError::Forbidden);
        }
        Ok(())
    }

    async fn verify_postgres(&self) -> Result<(), KnowledgeError> {
        let version: i32 = sqlx::query_scalar("SHOW server_version_num")
            .fetch_one(&self.pool)
            .await
            .map_err(|_| KnowledgeError::DatabaseUnavailable)?;
        if version < 150_000 {
            Err(KnowledgeError::DatabaseIncompatible)
        } else {
            Ok(())
        }
    }
    async fn verify_pgvector(&self) -> Result<(), KnowledgeError> {
        if sqlx::query("SELECT extversion FROM pg_extension WHERE extname = 'vector'")
            .fetch_optional(&self.pool)
            .await
            .map_err(|_| KnowledgeError::DatabaseUnavailable)?
            .is_some()
        {
            Ok(())
        } else {
            Err(KnowledgeError::DatabaseIncompatible)
        }
    }
    async fn verify_migration_ledger(&self) -> Result<(), KnowledgeError> {
        let applied =
            sqlx::query("SELECT version, checksum FROM _sqlx_migrations ORDER BY version")
                .fetch_all(&self.pool)
                .await
                .map_err(|_| KnowledgeError::MigrationIntegrity)?;
        let compiled: std::collections::BTreeMap<i64, Vec<u8>> = MIGRATOR
            .iter()
            .map(|migration| (migration.version, migration.checksum.to_vec()))
            .collect();
        if applied.len() != compiled.len() {
            return Err(KnowledgeError::MigrationIntegrity);
        }
        for row in applied {
            let version: i64 = row
                .try_get("version")
                .map_err(|_| KnowledgeError::MigrationIntegrity)?;
            let checksum: Vec<u8> = row
                .try_get("checksum")
                .map_err(|_| KnowledgeError::MigrationIntegrity)?;
            let Some(expected) = compiled.get(&version) else {
                return Err(KnowledgeError::MigrationIntegrity);
            };
            if checksum.ct_eq(expected).unwrap_u8() != 1 {
                return Err(KnowledgeError::MigrationIntegrity);
            }
        }
        Ok(())
    }
}
async fn issue_session(
    tx: &mut Transaction<'_, Postgres>,
    user_id: UserId,
    organization_id: OrganizationId,
    global_role: GlobalRole,
    username: String,
    device_name: &str,
    config: &AuthConfig,
) -> Result<IssuedSession, KnowledgeError> {
    let access_token = random_token(ACCESS_TOKEN_BYTES);
    let refresh_token = random_token(REFRESH_TOKEN_BYTES);
    let now = Utc::now();
    let access_expires_at = now + config.access_ttl;
    let refresh_expires_at = now + config.refresh_ttl;
    let session_id = Uuid::new_v4();
    sqlx::query("INSERT INTO sessions (id, user_id, access_token_digest, refresh_token_digest, device_name, access_expires_at, refresh_expires_at) VALUES ($1, $2, $3, $4, $5, $6, $7)").bind(session_id).bind(user_id).bind(token_digest(&access_token)).bind(token_digest(&refresh_token)).bind(device_name).bind(access_expires_at).bind(refresh_expires_at).execute(&mut **tx).await.map_err(|_| KnowledgeError::Internal)?;
    Ok(IssuedSession {
        access_token,
        refresh_token,
        access_expires_at,
        identity: IdentityResponse {
            user_id,
            organization_id,
            username,
            global_role,
            session_id,
            expires_at: access_expires_at,
        },
    })
}
async fn audit(
    tx: &mut Transaction<'_, Postgres>,
    organization_id: OrganizationId,
    actor: Option<UserId>,
    action: AuditAction,
    outcome: AuditOutcome,
    target_type: &str,
    target_id: Option<Uuid>,
    request_id: Uuid,
) -> Result<(), KnowledgeError> {
    sqlx::query("INSERT INTO audit_events (id, organization_id, actor_user_id, action, outcome, target_type, target_id, request_id) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)").bind(Uuid::new_v4()).bind(organization_id).bind(actor).bind(action.as_str()).bind(outcome.as_str()).bind(target_type).bind(target_id).bind(request_id).execute(&mut **tx).await.map_err(|_| KnowledgeError::Internal)?;
    Ok(())
}
fn collection_from_row(row: sqlx::postgres::PgRow) -> Result<CollectionResponse, KnowledgeError> {
    Ok(CollectionResponse {
        id: row.try_get("id").map_err(|_| KnowledgeError::Internal)?,
        organization_id: row
            .try_get("organization_id")
            .map_err(|_| KnowledgeError::Internal)?,
        name: row.try_get("name").map_err(|_| KnowledgeError::Internal)?,
        description: row
            .try_get("description")
            .map_err(|_| KnowledgeError::Internal)?,
        role: parse_collection_role(
            &row.try_get::<String, _>("role")
                .map_err(|_| KnowledgeError::Internal)?,
        )?,
        created_at: row
            .try_get("created_at")
            .map_err(|_| KnowledgeError::Internal)?,
    })
}
fn parse_global_role(value: &str) -> Result<GlobalRole, KnowledgeError> {
    match value {
        "server_admin" => Ok(GlobalRole::ServerAdmin),
        "user" => Ok(GlobalRole::User),
        _ => Err(KnowledgeError::Internal),
    }
}
fn parse_collection_role(value: &str) -> Result<CollectionRole, KnowledgeError> {
    match value {
        "collection_admin" => Ok(CollectionRole::CollectionAdmin),
        "editor" => Ok(CollectionRole::Editor),
        "viewer" => Ok(CollectionRole::Viewer),
        _ => Err(KnowledgeError::Internal),
    }
}
fn normalize_username(value: &str) -> String {
    value.trim().to_lowercase()
}
fn normalize_source_identity(value: &str) -> String {
    value.trim().to_lowercase()
}

fn validate_username(value: &str) -> Result<(), KnowledgeError> {
    let normalized = normalize_username(value);
    if normalized.len() < 3
        || normalized.len() > 64
        || !normalized
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    {
        Err(KnowledgeError::Validation)
    } else {
        Ok(())
    }
}
fn validate_password(value: &str) -> Result<(), KnowledgeError> {
    if value.len() < 12 || value.len() > 256 {
        Err(KnowledgeError::Validation)
    } else {
        Ok(())
    }
}
fn validate_device_name(value: &str) -> Result<(), KnowledgeError> {
    if value.trim().is_empty() || value.len() > 128 {
        Err(KnowledgeError::Validation)
    } else {
        Ok(())
    }
}
fn validate_collection(name: &str, description: Option<&str>) -> Result<(), KnowledgeError> {
    if name.trim().is_empty() || name.trim().len() > 160 || description.unwrap_or("").len() > 2000 {
        Err(KnowledgeError::Validation)
    } else {
        Ok(())
    }
}
fn hash_password(password: &str) -> Result<String, KnowledgeError> {
    Argon2::default()
        .hash_password(password.as_bytes(), &SaltString::generate(&mut OsRng))
        .map(|hash| hash.to_string())
        .map_err(|_| KnowledgeError::Internal)
}
fn verify_password(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .ok()
        .and_then(|parsed| {
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .ok()
        })
        .is_some()
}
fn random_token(bytes: usize) -> String {
    let mut buffer = vec![0u8; bytes];
    OsRng.fill_bytes(&mut buffer);
    hex::encode(buffer)
}
pub fn token_digest(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}
