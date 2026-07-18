//! Stable public contracts for the shared Knowledge Service.
//!
//! This crate deliberately contains no HTTP, database, provider, or secret
//! handling code. It is safe for use by both service and future desktop clients.

use chrono::{DateTime, Utc};
use http::StatusCode;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub const API_VERSION: &str = "v1";
pub const REQUEST_ID_HEADER: &str = "x-request-id";

pub type OrganizationId = Uuid;
pub type UserId = Uuid;
pub type CollectionId = Uuid;
pub type SessionId = Uuid;
pub type EmbeddingProfileId = Uuid;
pub type EmbeddingProfileVersionId = Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingProviderKind {
    OpenAiCompatible,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingProfileResponse {
    pub id: EmbeddingProfileId,
    pub name: String,
    pub provider_kind: EmbeddingProviderKind,
    pub active_version_id: Option<EmbeddingProfileVersionId>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingProfileVersionResponse {
    pub id: EmbeddingProfileVersionId,
    pub profile_id: EmbeddingProfileId,
    pub version: i32,
    pub base_url: String,
    pub model: String,
    pub expected_dimension: i32,
    pub batch_size: i32,
    pub request_timeout_ms: i32,
    pub max_retries: i32,
    pub has_secret: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateEmbeddingProfileRequest {
    pub name: String,
    pub base_url: String,
    pub model: String,
    pub expected_dimension: i32,
    pub batch_size: i32,
    pub request_timeout_ms: i32,
    pub max_retries: i32,
    pub secret_ref: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateEmbeddingProfileVersionRequest {
    pub base_url: String,
    pub model: String,
    pub expected_dimension: i32,
    pub batch_size: i32,
    pub request_timeout_ms: i32,
    pub max_retries: i32,
    pub secret_ref: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyState {
    Ready,
    Unavailable,
    NotConfigured,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyHealth {
    pub name: String,
    pub state: DependencyState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveHealthResponse {
    pub status: &'static str,
    pub service: &'static str,
    pub version: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadyHealthResponse {
    pub status: &'static str,
    pub service: &'static str,
    pub version: &'static str,
    pub checked_at: DateTime<Utc>,
    pub dependencies: Vec<DependencyHealth>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependenciesHealthResponse {
    pub service: &'static str,
    pub version: &'static str,
    pub dependencies: Vec<DependencyHealth>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum GlobalRole {
    ServerAdmin,
    User,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum CollectionRole {
    CollectionAdmin,
    Editor,
    Viewer,
}

impl CollectionRole {
    pub fn can_manage_members(self) -> bool {
        matches!(self, Self::CollectionAdmin)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::CollectionAdmin => "collection_admin",
            Self::Editor => "editor",
            Self::Viewer => "viewer",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditAction {
    ServiceBootstrap,
    UserCreated,
    SourceCreated,
    LoginSucceeded,
    SessionRefreshed,
    SessionRevoked,
    CollectionCreated,
    CollectionMemberGranted,
    CollectionMemberUpdated,
    CollectionMemberRemoved,
}

impl AuditAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ServiceBootstrap => "service_bootstrap",
            Self::UserCreated => "user_created",
            Self::SourceCreated => "source_created",
            Self::LoginSucceeded => "login_succeeded",
            Self::SessionRefreshed => "session_refreshed",
            Self::SessionRevoked => "session_revoked",
            Self::CollectionCreated => "collection_created",
            Self::CollectionMemberGranted => "collection_member_granted",
            Self::CollectionMemberUpdated => "collection_member_updated",
            Self::CollectionMemberRemoved => "collection_member_removed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditOutcome {
    Succeeded,
    Denied,
}

impl AuditOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Denied => "denied",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessScope {
    pub user_id: UserId,
    pub organization_id: OrganizationId,
    pub global_role: GlobalRole,
    pub session_id: SessionId,
}

impl AccessScope {
    pub fn is_server_admin(&self) -> bool {
        self.global_role == GlobalRole::ServerAdmin
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityResponse {
    pub user_id: UserId,
    pub organization_id: OrganizationId,
    pub username: String,
    pub global_role: GlobalRole,
    pub session_id: SessionId,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BootstrapRequest {
    pub bootstrap_token: String,
    pub username: String,
    pub password: String,
    pub device_name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    pub device_name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub access_expires_at: DateTime<Utc>,
    pub identity: IdentityResponse,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateSourceRequest {
    pub collection_id: CollectionId,
    pub display_name: String,
    pub mime_type: String,
    pub content_base64: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceResponse {
    pub id: Uuid,
    pub collection_id: CollectionId,
    pub kind: String,
    pub display_name: String,
    pub active_revision_id: Option<Uuid>,
    pub state: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListJobsQuery {
    pub collection_id: Option<CollectionId>,
    pub state: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IngestionJobResponse {
    pub id: Uuid,
    pub source_id: Option<Uuid>,
    pub revision_id: Option<Uuid>,
    pub state: String,
    pub progress_current: i64,
    pub progress_total: Option<i64>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChunkResponse {
    pub id: Uuid,
    pub ordinal: i32,
    pub token_count: i32,
    pub content: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RetrievalRequest {
    pub query: String,
    pub collection_ids: Vec<CollectionId>,
    pub max_results: Option<u32>,
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RetrievalCitation {
    pub key: String,
    pub source_id: Uuid,
    pub revision_id: Uuid,
    pub chunk_id: Uuid,
    pub source_name: String,
    pub ordinal: i32,
    pub char_start: i32,
    pub char_end: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct RetrievalHitResponse {
    pub content: String,
    pub score: f64,
    pub citation: RetrievalCitation,
}

#[derive(Debug, Clone, Serialize)]
pub struct RetrievalResponse {
    pub run_id: Uuid,
    pub hits: Vec<RetrievalHitResponse>,
    pub degraded: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UserResponse {
    pub id: UserId,
    pub organization_id: OrganizationId,
    pub username: String,
    pub global_role: GlobalRole,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateCollectionRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpsertCollectionMemberRequest {
    pub user_id: UserId,
    pub role: CollectionRole,
}

#[derive(Debug, Clone, Serialize)]
pub struct CollectionResponse {
    pub id: CollectionId,
    pub organization_id: OrganizationId,
    pub name: String,
    pub description: String,
    pub role: CollectionRole,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CollectionMemberResponse {
    pub user_id: UserId,
    pub username: String,
    pub role: CollectionRole,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiErrorResponse {
    pub error: ApiErrorBody,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiErrorBody {
    pub code: &'static str,
    pub message: &'static str,
    pub request_id: Uuid,
}

#[derive(Debug, Error)]
pub enum KnowledgeError {
    #[error("service configuration is invalid")]
    Configuration,
    #[error("knowledge database is unavailable")]
    DatabaseUnavailable,
    #[error("knowledge database is incompatible")]
    DatabaseIncompatible,
    #[error("knowledge database migration integrity check failed")]
    MigrationIntegrity,
    #[error("request validation failed")]
    Validation,
    #[error("authentication is required")]
    Unauthenticated,
    #[error("request is not authorized")]
    Forbidden,
    #[error("requested resource was not found")]
    NotFound,
    #[error("request conflicts with existing state")]
    Conflict,
    #[error("internal knowledge service failure")]
    Internal,
    #[error("embedding provider failure")]
    EmbeddingProvider,
    #[error("embedding response is invalid")]
    EmbeddingInvalid,
}

impl KnowledgeError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Configuration => "configuration_invalid",
            Self::DatabaseUnavailable => "dependency_unavailable",
            Self::DatabaseIncompatible => "dependency_incompatible",
            Self::MigrationIntegrity => "migration_integrity_failed",
            Self::Validation => "validation_failed",
            Self::Unauthenticated => "unauthenticated",
            Self::Forbidden => "forbidden",
            Self::NotFound => "not_found",
            Self::Conflict => "conflict",
            Self::Internal => "internal_error",
            Self::EmbeddingProvider => "embedding_provider_failed",
            Self::EmbeddingInvalid => "embedding_response_invalid",
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            Self::Configuration => "The knowledge service configuration is invalid.",
            Self::DatabaseUnavailable => "A required knowledge service dependency is unavailable.",
            Self::DatabaseIncompatible => {
                "A required knowledge service dependency is incompatible."
            }
            Self::MigrationIntegrity => "The knowledge service migration integrity check failed.",
            Self::Validation => "The request did not pass validation.",
            Self::Unauthenticated => "Authentication is required for this request.",
            Self::Forbidden => "The authenticated identity cannot perform this action.",
            Self::NotFound => "The requested resource was not found.",
            Self::Conflict => "The request conflicts with the current service state.",
            Self::Internal => "The knowledge service could not complete the request.",
            Self::EmbeddingProvider => "The embedding provider could not complete the request.",
            Self::EmbeddingInvalid => "The embedding provider returned an invalid response.",
        }
    }

    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::Configuration | Self::DatabaseIncompatible | Self::MigrationIntegrity => {
                StatusCode::SERVICE_UNAVAILABLE
            }
            Self::DatabaseUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            Self::Validation => StatusCode::BAD_REQUEST,
            Self::Unauthenticated => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Conflict => StatusCode::CONFLICT,
            Self::Internal | Self::EmbeddingProvider | Self::EmbeddingInvalid => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }

    pub fn response(&self, request_id: Uuid) -> ApiErrorResponse {
        ApiErrorResponse {
            error: ApiErrorBody {
                code: self.code(),
                message: self.message(),
                request_id,
            },
        }
    }
}
