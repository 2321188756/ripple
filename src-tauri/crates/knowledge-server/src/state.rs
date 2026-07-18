use ripple_knowledge_domain::AccessScope;
use ripple_knowledge_ingest::LocalObjectStore;
use ripple_knowledge_store::{AuthConfig, KnowledgeStore};
use sha2::{Digest, Sha256};

#[derive(Clone)]
pub struct AppState {
    pub store: Option<KnowledgeStore>,
    pub object_store: LocalObjectStore,
    pub bootstrap_token_digest: Vec<u8>,
    pub auth: AuthConfig,
}

impl AppState {
    pub fn bootstrap_digest(bootstrap_token: &str) -> Vec<u8> {
        Sha256::digest(bootstrap_token.as_bytes()).to_vec()
    }
}

#[derive(Clone)]
pub struct AuthenticatedScope(pub AccessScope);
