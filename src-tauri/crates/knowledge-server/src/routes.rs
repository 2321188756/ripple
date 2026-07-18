use crate::state::{AppState, AuthenticatedScope};
use axum::{
    extract::{Multipart, Path, Query, State},
    http::{
        header::{AUTHORIZATION, CONTENT_LENGTH},
        HeaderValue, StatusCode,
    },
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Extension, Json, Router,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::Utc;
use ripple_knowledge_domain::{
    ApiErrorResponse, BootstrapRequest, CollectionId, CreateCollectionRequest, CreateSourceRequest,
    CreateUserRequest, DependenciesHealthResponse, DependencyHealth, DependencyState,
    KnowledgeError, LiveHealthResponse, LoginRequest, ReadyHealthResponse, RefreshRequest,
    SessionResponse, UpsertCollectionMemberRequest, REQUEST_ID_HEADER,
};
use ripple_knowledge_ingest::{supports_text_mime, ObjectStore};
use ripple_knowledge_store::IssuedSession;
use std::iter::once;
use tower_http::{
    limit::RequestBodyLimitLayer, sensitive_headers::SetSensitiveRequestHeadersLayer,
};
use uuid::Uuid;

const SERVICE_NAME: &str = "ripple-knowledge-service";
const SERVICE_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn router(state: AppState) -> Router {
    let protected = Router::new()
        .route("/auth/logout", post(logout))
        .route("/auth/me", get(me))
        .route("/users", post(create_user))
        .route("/sources/upload", post(create_upload_source))
        .route(
            "/sources/upload-multipart",
            post(create_multipart_upload_source),
        )
        .route("/retrieval/search", post(search_retrieval))
        .route("/jobs", get(list_jobs))
        .route("/jobs/{job_id}/cancel", post(cancel_job))
        .route(
            "/collections",
            post(create_collection).get(list_collections),
        )
        .route(
            "/collections/{collection_id}/members",
            get(list_members).put(upsert_member),
        )
        .route(
            "/collections/{collection_id}/members/{user_id}",
            delete(delete_member),
        )
        .route_layer(middleware::from_fn_with_state(state.clone(), require_auth));

    Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .route("/health/dependencies", get(dependencies))
        .nest(
            "/api/v1",
            Router::new()
                .route("/bootstrap", post(bootstrap))
                .route("/auth/login", post(login))
                .route("/auth/refresh", post(refresh))
                .merge(protected),
        )
        .layer(RequestBodyLimitLayer::new(14 * 1024 * 1024))
        .layer(middleware::from_fn(payload_limit_errors))
        .layer(SetSensitiveRequestHeadersLayer::new(once(AUTHORIZATION)))
        .layer(middleware::from_fn(correlation_id))
        .with_state(state)
}

async fn correlation_id(mut request: axum::extract::Request, next: Next) -> Response {
    let request_id = request
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| Uuid::parse_str(value).ok())
        .unwrap_or_else(Uuid::new_v4);
    request.extensions_mut().insert(request_id);
    let mut response = next.run(request).await;
    if let Ok(value) = HeaderValue::from_str(&request_id.to_string()) {
        response.headers_mut().insert(REQUEST_ID_HEADER, value);
    }
    response
}

async fn payload_limit_errors(request: axum::extract::Request, next: Next) -> Response {
    let request_id = correlation_from_extensions(request.extensions());
    if request
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<usize>().ok())
        .is_some_and(|length| length > 14 * 1024 * 1024)
    {
        return error_response(KnowledgeError::Validation, request_id);
    }
    next.run(request).await
}

async fn require_auth(
    State(state): State<AppState>,
    mut request: axum::extract::Request,
    next: Next,
) -> Response {
    let request_id = correlation_from_extensions(request.extensions());
    let Some(token) = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|token| !token.is_empty())
    else {
        return error_response(KnowledgeError::Unauthenticated, request_id);
    };
    let Some(store) = state.store else {
        return error_response(KnowledgeError::DatabaseUnavailable, request_id);
    };
    match store.resolve_access_scope(token).await {
        Ok((scope, _)) => {
            request.extensions_mut().insert(AuthenticatedScope(scope));
            next.run(request).await
        }
        Err(error) => error_response(error, request_id),
    }
}

async fn live() -> Json<LiveHealthResponse> {
    Json(LiveHealthResponse {
        status: "live",
        service: SERVICE_NAME,
        version: SERVICE_VERSION,
    })
}

async fn ready(State(state): State<AppState>, Extension(request_id): Extension<Uuid>) -> Response {
    match state.store {
        Some(store) => match store.readiness().await {
            Ok(dependencies) => Json(ReadyHealthResponse {
                status: "ready",
                service: SERVICE_NAME,
                version: SERVICE_VERSION,
                checked_at: Utc::now(),
                dependencies,
            })
            .into_response(),
            Err(error) => error_response(error, request_id),
        },
        None => error_response(KnowledgeError::DatabaseUnavailable, request_id),
    }
}

async fn dependencies(State(state): State<AppState>) -> Response {
    let dependencies = match state.store {
        Some(store) => store
            .readiness()
            .await
            .unwrap_or_else(|_| unavailable_dependencies()),
        None => unavailable_dependencies(),
    };
    Json(DependenciesHealthResponse {
        service: SERVICE_NAME,
        version: SERVICE_VERSION,
        dependencies,
    })
    .into_response()
}

async fn bootstrap(
    State(state): State<AppState>,
    Extension(request_id): Extension<Uuid>,
    Json(input): Json<BootstrapRequest>,
) -> Response {
    let result = match required_store(&state) {
        Ok(store) => {
            store
                .bootstrap(
                    &state.bootstrap_token_digest,
                    &ripple_knowledge_store::token_digest(&input.bootstrap_token),
                    &input.username,
                    &input.password,
                    &input.device_name,
                    &state.auth,
                    request_id,
                )
                .await
        }
        Err(error) => Err(error),
    };
    result
        .map(session_response)
        .map(Json)
        .map(IntoResponse::into_response)
        .unwrap_or_else(|error| error_response(error, request_id))
}

async fn login(
    State(state): State<AppState>,
    Extension(request_id): Extension<Uuid>,
    Json(input): Json<LoginRequest>,
) -> Response {
    let result = required_store(&state)
        .and_then(|store| Ok((store, &input.username, &input.password, &input.device_name)));
    match result {
        Ok((store, username, password, device_name)) => store
            .login(username, password, device_name, &state.auth, request_id)
            .await
            .map(session_response)
            .map(Json)
            .map(IntoResponse::into_response)
            .unwrap_or_else(|error| error_response(error, request_id)),
        Err(error) => error_response(error, request_id),
    }
}

async fn refresh(
    State(state): State<AppState>,
    Extension(request_id): Extension<Uuid>,
    Json(input): Json<RefreshRequest>,
) -> Response {
    match required_store(&state) {
        Ok(store) => store
            .refresh(&input.refresh_token, &state.auth, request_id)
            .await
            .map(session_response)
            .map(Json)
            .map(IntoResponse::into_response)
            .unwrap_or_else(|error| error_response(error, request_id)),
        Err(error) => error_response(error, request_id),
    }
}

async fn logout(
    State(state): State<AppState>,
    Extension(request_id): Extension<Uuid>,
    Extension(scope): Extension<AuthenticatedScope>,
) -> Response {
    match required_store(&state) {
        Ok(store) => store
            .revoke_session(&scope.0, request_id)
            .await
            .map(|()| StatusCode::NO_CONTENT.into_response())
            .unwrap_or_else(|error| error_response(error, request_id)),
        Err(error) => error_response(error, request_id),
    }
}

async fn me(
    State(state): State<AppState>,
    Extension(request_id): Extension<Uuid>,
    Extension(scope): Extension<AuthenticatedScope>,
) -> Response {
    match required_store(&state) {
        Ok(store) => store
            .resolve_access_scope_from_scope(&scope.0)
            .await
            .map(Json)
            .map(IntoResponse::into_response)
            .unwrap_or_else(|error| error_response(error, request_id)),
        Err(error) => error_response(error, request_id),
    }
}

async fn search_retrieval(
    State(state): State<AppState>,
    Extension(request_id): Extension<Uuid>,
    Extension(scope): Extension<AuthenticatedScope>,
    Json(input): Json<ripple_knowledge_domain::RetrievalRequest>,
) -> Response {
    if input.collection_ids.is_empty() {
        return error_response(KnowledgeError::Validation, request_id);
    }
    match required_store(&state) {
        Ok(store) => store
            .lexical_search(
                &scope.0,
                &input.query,
                &input.collection_ids,
                input.max_results.unwrap_or(10),
                input.mode.as_deref().unwrap_or("deep_search_tool"),
            )
            .await
            .map(Json)
            .map(IntoResponse::into_response)
            .unwrap_or_else(|error| error_response(error, request_id)),
        Err(error) => error_response(error, request_id),
    }
}

async fn list_jobs(
    State(state): State<AppState>,
    Extension(request_id): Extension<Uuid>,
    Extension(scope): Extension<AuthenticatedScope>,
    Query(query): Query<ripple_knowledge_domain::ListJobsQuery>,
) -> Response {
    match required_store(&state) {
        Ok(store) => store
            .list_ingestion_jobs(
                &scope.0,
                query.collection_id,
                query.state.as_deref(),
                query.limit.unwrap_or(50),
            )
            .await
            .map(Json)
            .map(IntoResponse::into_response)
            .unwrap_or_else(|error| error_response(error, request_id)),
        Err(error) => error_response(error, request_id),
    }
}

async fn cancel_job(
    State(state): State<AppState>,
    Extension(request_id): Extension<Uuid>,
    Extension(scope): Extension<AuthenticatedScope>,
    Path(job_id): Path<Uuid>,
) -> Response {
    match required_store(&state) {
        Ok(store) => store
            .request_cancel_job(&scope.0, job_id)
            .await
            .map(|()| StatusCode::NO_CONTENT.into_response())
            .unwrap_or_else(|error| error_response(error, request_id)),
        Err(error) => error_response(error, request_id),
    }
}

async fn create_multipart_upload_source(
    State(state): State<AppState>,
    Extension(request_id): Extension<Uuid>,
    Extension(scope): Extension<AuthenticatedScope>,
    mut multipart: Multipart,
) -> Response {
    const MAX_FILE_BYTES: u64 = 10 * 1024 * 1024;
    let mut collection_id = None;
    let mut display_name = None;
    let mut mime_type = None;
    let mut stored_object = None;

    loop {
        let field = match multipart.next_field().await {
            Ok(Some(field)) => field,
            Ok(None) => break,
            Err(_) => return error_response(KnowledgeError::Validation, request_id),
        };
        let Some(name) = field.name().map(str::to_owned) else {
            return error_response(KnowledgeError::Validation, request_id);
        };
        match name.as_str() {
            "collection_id" if collection_id.is_none() => {
                let value = match field.text().await {
                    Ok(value) if value.len() <= 64 => value,
                    _ => return error_response(KnowledgeError::Validation, request_id),
                };
                collection_id = Uuid::parse_str(value.trim()).ok();
                if collection_id.is_none() {
                    return error_response(KnowledgeError::Validation, request_id);
                }
            }
            "display_name" if display_name.is_none() => {
                let value = match field.text().await {
                    Ok(value) if !value.trim().is_empty() && value.len() <= 512 => value,
                    _ => return error_response(KnowledgeError::Validation, request_id),
                };
                display_name = Some(value);
            }
            "mime_type" if mime_type.is_none() => {
                let value = match field.text().await {
                    Ok(value) if value.len() <= 128 && supports_text_mime(&value) => value,
                    _ => return error_response(KnowledgeError::Validation, request_id),
                };
                mime_type = Some(value);
            }
            "content" if stored_object.is_none() => {
                if collection_id.is_none() || display_name.is_none() || mime_type.is_none() {
                    return error_response(KnowledgeError::Validation, request_id);
                }
                let stream = futures::stream::try_unfold(field, |mut field| async move {
                    match field.chunk().await {
                        Ok(Some(chunk)) => Ok(Some((chunk, field))),
                        Ok(None) => Ok(None),
                        Err(error) => Err(std::io::Error::other(error.to_string())),
                    }
                });
                stored_object = match state
                    .object_store
                    .put_stream(
                        &scope.0.organization_id.to_string(),
                        Box::pin(stream),
                        MAX_FILE_BYTES,
                    )
                    .await
                {
                    Ok(object) if object.byte_size > 0 => Some(object),
                    Ok(_) | Err(ripple_knowledge_ingest::ObjectStoreError::TooLarge) => {
                        return error_response(KnowledgeError::Validation, request_id)
                    }
                    Err(_) => return error_response(KnowledgeError::Internal, request_id),
                };
            }
            _ => return error_response(KnowledgeError::Validation, request_id),
        }
    }

    let (Some(collection_id), Some(display_name), Some(mime_type), Some(object)) =
        (collection_id, display_name, mime_type, stored_object)
    else {
        return error_response(KnowledgeError::Validation, request_id);
    };
    match required_store(&state) {
        Ok(store) => store
            .create_upload_source(
                &scope.0,
                collection_id,
                &display_name,
                &mime_type,
                &object.key,
                &object.sha256,
                object.byte_size,
                request_id,
            )
            .await
            .map(|(source, job)| {
                (
                    StatusCode::ACCEPTED,
                    Json(serde_json::json!({ "source": source, "job": job })),
                )
                    .into_response()
            })
            .unwrap_or_else(|error| error_response(error, request_id)),
        Err(error) => error_response(error, request_id),
    }
}

async fn create_upload_source(
    State(state): State<AppState>,
    Extension(request_id): Extension<Uuid>,
    Extension(scope): Extension<AuthenticatedScope>,
    Json(input): Json<CreateSourceRequest>,
) -> Response {
    let content = match STANDARD.decode(input.content_base64.as_bytes()) {
        Ok(content) => content,
        Err(_) => return error_response(KnowledgeError::Validation, request_id),
    };
    if content.is_empty()
        || content.len() > 10 * 1024 * 1024
        || !supports_text_mime(&input.mime_type)
    {
        return error_response(KnowledgeError::Validation, request_id);
    }
    let object = match state
        .object_store
        .put_bytes(
            &scope.0.organization_id.to_string(),
            &content,
            10 * 1024 * 1024,
        )
        .await
    {
        Ok(object) => object,
        Err(_) => return error_response(KnowledgeError::Internal, request_id),
    };
    match required_store(&state) {
        Ok(store) => store
            .create_upload_source(
                &scope.0,
                input.collection_id,
                &input.display_name,
                &input.mime_type,
                &object.key,
                &object.sha256,
                object.byte_size,
                request_id,
            )
            .await
            .map(|(source, job)| {
                (
                    StatusCode::ACCEPTED,
                    Json(serde_json::json!({ "source": source, "job": job })),
                )
                    .into_response()
            })
            .unwrap_or_else(|error| error_response(error, request_id)),
        Err(error) => error_response(error, request_id),
    }
}

async fn create_user(
    State(state): State<AppState>,
    Extension(request_id): Extension<Uuid>,
    Extension(scope): Extension<AuthenticatedScope>,
    Json(input): Json<CreateUserRequest>,
) -> Response {
    match required_store(&state) {
        Ok(store) => store
            .create_user(&scope.0, &input.username, &input.password, request_id)
            .await
            .map(|user| (StatusCode::CREATED, Json(user)).into_response())
            .unwrap_or_else(|error| error_response(error, request_id)),
        Err(error) => error_response(error, request_id),
    }
}

async fn create_collection(
    State(state): State<AppState>,
    Extension(request_id): Extension<Uuid>,
    Extension(scope): Extension<AuthenticatedScope>,
    Json(input): Json<CreateCollectionRequest>,
) -> Response {
    match required_store(&state) {
        Ok(store) => store
            .create_collection(
                &scope.0,
                &input.name,
                input.description.as_deref(),
                request_id,
            )
            .await
            .map(|collection| (StatusCode::CREATED, Json(collection)).into_response())
            .unwrap_or_else(|error| error_response(error, request_id)),
        Err(error) => error_response(error, request_id),
    }
}

async fn list_collections(
    State(state): State<AppState>,
    Extension(request_id): Extension<Uuid>,
    Extension(scope): Extension<AuthenticatedScope>,
) -> Response {
    match required_store(&state) {
        Ok(store) => store
            .list_collections(&scope.0)
            .await
            .map(Json)
            .map(IntoResponse::into_response)
            .unwrap_or_else(|error| error_response(error, request_id)),
        Err(error) => error_response(error, request_id),
    }
}

async fn list_members(
    State(state): State<AppState>,
    Extension(request_id): Extension<Uuid>,
    Extension(scope): Extension<AuthenticatedScope>,
    Path(collection_id): Path<CollectionId>,
) -> Response {
    match required_store(&state) {
        Ok(store) => store
            .list_members(&scope.0, collection_id)
            .await
            .map(Json)
            .map(IntoResponse::into_response)
            .unwrap_or_else(|error| error_response(error, request_id)),
        Err(error) => error_response(error, request_id),
    }
}

async fn upsert_member(
    State(state): State<AppState>,
    Extension(request_id): Extension<Uuid>,
    Extension(scope): Extension<AuthenticatedScope>,
    Path(collection_id): Path<CollectionId>,
    Json(input): Json<UpsertCollectionMemberRequest>,
) -> Response {
    match required_store(&state) {
        Ok(store) => store
            .upsert_member(
                &scope.0,
                collection_id,
                input.user_id,
                input.role,
                request_id,
            )
            .await
            .map(|()| StatusCode::NO_CONTENT.into_response())
            .unwrap_or_else(|error| error_response(error, request_id)),
        Err(error) => error_response(error, request_id),
    }
}

async fn delete_member(
    State(state): State<AppState>,
    Extension(request_id): Extension<Uuid>,
    Extension(scope): Extension<AuthenticatedScope>,
    Path((collection_id, user_id)): Path<(CollectionId, Uuid)>,
) -> Response {
    match required_store(&state) {
        Ok(store) => store
            .delete_member(&scope.0, collection_id, user_id, request_id)
            .await
            .map(|()| StatusCode::NO_CONTENT.into_response())
            .unwrap_or_else(|error| error_response(error, request_id)),
        Err(error) => error_response(error, request_id),
    }
}

fn required_store(
    state: &AppState,
) -> Result<&ripple_knowledge_store::KnowledgeStore, KnowledgeError> {
    state
        .store
        .as_ref()
        .ok_or(KnowledgeError::DatabaseUnavailable)
}

fn session_response(issued: IssuedSession) -> SessionResponse {
    SessionResponse {
        access_token: issued.access_token,
        refresh_token: issued.refresh_token,
        access_expires_at: issued.access_expires_at,
        identity: issued.identity,
    }
}

fn correlation_from_extensions(extensions: &http::Extensions) -> Uuid {
    extensions
        .get::<Uuid>()
        .copied()
        .unwrap_or_else(Uuid::new_v4)
}

fn error_response(error: KnowledgeError, request_id: Uuid) -> Response {
    (
        error.status_code(),
        Json::<ApiErrorResponse>(error.response(request_id)),
    )
        .into_response()
}

fn unavailable_dependencies() -> Vec<DependencyHealth> {
    vec![
        DependencyHealth {
            name: "postgresql".into(),
            state: DependencyState::Unavailable,
        },
        DependencyHealth {
            name: "pgvector".into(),
            state: DependencyState::Unavailable,
        },
        DependencyHealth {
            name: "migration_audit".into(),
            state: DependencyState::Unavailable,
        },
    ]
}
