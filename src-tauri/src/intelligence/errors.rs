use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderError {
    NotConfigured,
    Timeout,
    Cancelled,
    RateLimited,
    InvalidResponse,
    SchemaRejected,
    NetworkUnavailable,
    ProviderUnavailable,
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderError::NotConfigured => write!(f, "Intelligence provider is not configured"),
            ProviderError::Timeout => write!(f, "Provider request timed out"),
            ProviderError::Cancelled => write!(f, "Provider request was cancelled"),
            ProviderError::RateLimited => write!(f, "Provider rate limit reached"),
            ProviderError::InvalidResponse => write!(f, "Provider returned an invalid response"),
            ProviderError::SchemaRejected => {
                write!(
                    f,
                    "Provider output did not match the required suggestion schema"
                )
            }
            ProviderError::NetworkUnavailable => write!(f, "Network unavailable"),
            ProviderError::ProviderUnavailable => write!(f, "Provider unavailable"),
        }
    }
}

impl std::error::Error for ProviderError {}
