use thiserror::Error;

#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
    #[error("HTTP request error: {0}")]
    RequestError(#[from] reqwest::Error),
    #[error("HTTP status error: {0}")]
    HttpStatus(u16),
    #[error("Network error: {0}")]
    Undefined(String),
    #[error("Cloudflare challenge required for domain: {domain}, url: {target_url}")]
    CloudflareBlocked { domain: String, target_url: String },
}
