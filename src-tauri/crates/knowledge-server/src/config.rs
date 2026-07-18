use chrono::Duration as ChronoDuration;
use std::{env, net::SocketAddr, path::PathBuf};
use url::Url;

const ENV_PREFIX: &str = "RIPPLE_KNOWLEDGE_";

#[derive(Clone, Debug)]
pub struct ServiceConfig {
    pub listen_addr: SocketAddr,
    pub database_url: String,
    pub data_root: PathBuf,
    pub max_connections: u32,
    pub bootstrap_token: String,
    pub access_ttl: ChronoDuration,
    pub refresh_ttl: ChronoDuration,
}

impl ServiceConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let listen_addr = env_value("LISTEN_ADDR")
            .unwrap_or_else(|| "127.0.0.1:8787".to_owned())
            .parse()
            .map_err(|_| ConfigError::InvalidListenAddress)?;

        let database_url = env_value("DATABASE_URL").ok_or(ConfigError::MissingDatabaseUrl)?;
        let parsed_database_url =
            Url::parse(&database_url).map_err(|_| ConfigError::InvalidDatabaseUrl)?;
        if parsed_database_url.scheme() != "postgres"
            && parsed_database_url.scheme() != "postgresql"
        {
            return Err(ConfigError::InvalidDatabaseUrl);
        }
        if parsed_database_url.host_str().is_none() {
            return Err(ConfigError::InvalidDatabaseUrl);
        }

        let data_root = env_value("DATA_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("./knowledge-service-data"));
        if !data_root.is_absolute() {
            return Err(ConfigError::DataRootMustBeAbsolute);
        }

        let max_connections = env_value("MAX_CONNECTIONS")
            .map(|value| {
                value
                    .parse()
                    .map_err(|_| ConfigError::InvalidMaxConnections)
            })
            .transpose()?
            .unwrap_or(5);
        if max_connections == 0 || max_connections > 32 {
            return Err(ConfigError::InvalidMaxConnections);
        }

        let bootstrap_token =
            env_value("BOOTSTRAP_TOKEN").ok_or(ConfigError::MissingBootstrapToken)?;
        if bootstrap_token.len() < 32 {
            return Err(ConfigError::InvalidBootstrapToken);
        }

        let access_ttl = duration_from_env("ACCESS_TOKEN_TTL_MINUTES", 15, 1, 60)?;
        let refresh_ttl = duration_from_env("REFRESH_TOKEN_TTL_HOURS", 168, 1, 24 * 30)?;
        if refresh_ttl <= access_ttl {
            return Err(ConfigError::InvalidSessionLifetime);
        }

        Ok(Self {
            listen_addr,
            database_url,
            data_root,
            max_connections,
            bootstrap_token,
            access_ttl,
            refresh_ttl,
        })
    }
}

fn env_value(name: &str) -> Option<String> {
    env::var(format!("{ENV_PREFIX}{name}"))
        .ok()
        .filter(|value| !value.is_empty())
}

fn duration_from_env(
    name: &str,
    default: i64,
    minimum: i64,
    maximum: i64,
) -> Result<ChronoDuration, ConfigError> {
    let amount = env_value(name)
        .map(|value| {
            value
                .parse::<i64>()
                .map_err(|_| ConfigError::InvalidSessionLifetime)
        })
        .transpose()?
        .unwrap_or(default);
    if amount < minimum || amount > maximum {
        return Err(ConfigError::InvalidSessionLifetime);
    }
    if name.ends_with("MINUTES") {
        Ok(ChronoDuration::minutes(amount))
    } else {
        Ok(ChronoDuration::hours(amount))
    }
}

#[derive(Debug)]
pub enum ConfigError {
    MissingDatabaseUrl,
    MissingBootstrapToken,
    InvalidBootstrapToken,
    InvalidDatabaseUrl,
    InvalidListenAddress,
    DataRootMustBeAbsolute,
    InvalidMaxConnections,
    InvalidSessionLifetime,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::MissingDatabaseUrl => "RIPPLE_KNOWLEDGE_DATABASE_URL is required",
            Self::MissingBootstrapToken => "RIPPLE_KNOWLEDGE_BOOTSTRAP_TOKEN is required",
            Self::InvalidBootstrapToken => {
                "RIPPLE_KNOWLEDGE_BOOTSTRAP_TOKEN must be at least 32 characters"
            }
            Self::InvalidDatabaseUrl => {
                "RIPPLE_KNOWLEDGE_DATABASE_URL must be a valid PostgreSQL URL"
            }
            Self::InvalidListenAddress => "RIPPLE_KNOWLEDGE_LISTEN_ADDR is invalid",
            Self::DataRootMustBeAbsolute => "RIPPLE_KNOWLEDGE_DATA_ROOT must be an absolute path",
            Self::InvalidMaxConnections => {
                "RIPPLE_KNOWLEDGE_MAX_CONNECTIONS must be between 1 and 32"
            }
            Self::InvalidSessionLifetime => {
                "Knowledge Service session lifetime configuration is invalid"
            }
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for ConfigError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_postgres_urls_are_accepted() {
        let url = Url::parse("https://example.test").unwrap();
        assert_ne!(url.scheme(), "postgres");
    }
}
