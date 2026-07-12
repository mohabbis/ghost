//! Normalized integration and provider errors (user-actionable, no token leakage).

use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationError {
    NotConfigured,
    AuthenticationRequired,
    ConsentRequired,
    TokenExpired,
    TokenRefreshFailed,
    PermissionDenied,
    ResourceNotFound,
    RateLimited,
    ProviderUnavailable,
    InvalidResponse,
    PolicyDenied,
    ApprovalRequired,
    ApprovalExpired,
    PlanChanged,
    NetworkUnavailable,
}

impl IntegrationError {
    pub fn user_message(&self) -> &'static str {
        match self {
            IntegrationError::NotConfigured => {
                "This integration is not configured yet. Open Settings to connect it."
            }
            IntegrationError::AuthenticationRequired => {
                "Sign in to your account before enabling this integration."
            }
            IntegrationError::ConsentRequired => {
                "Ghost needs your approval for additional permissions. Reconnect the integration and approve the requested scope."
            }
            IntegrationError::TokenExpired => {
                "Your authorization expired. Reconnect the integration to continue."
            }
            IntegrationError::TokenRefreshFailed => {
                "Ghost could not refresh your authorization. Sign in again and reconnect the integration."
            }
            IntegrationError::PermissionDenied => {
                "Ghost does not have permission for this resource. Check the integration scope or reconnect with the required access."
            }
            IntegrationError::ResourceNotFound => {
                "The requested workspace or dataset was not found or is no longer accessible."
            }
            IntegrationError::RateLimited => {
                "The service is rate-limiting requests. Wait a moment and try again."
            }
            IntegrationError::ProviderUnavailable => {
                "The provider is temporarily unavailable. Try again later."
            }
            IntegrationError::InvalidResponse => {
                "The provider returned an unexpected response. Try again or check your configuration."
            }
            IntegrationError::PolicyDenied => {
                "Ghost policy blocked this operation. Review the plan and policy settings."
            }
            IntegrationError::ApprovalRequired => {
                "This operation requires your explicit approval in Ghost before it can run."
            }
            IntegrationError::ApprovalExpired => {
                "The approval for this plan expired. Review and approve the plan again in Ghost."
            }
            IntegrationError::PlanChanged => {
                "The plan changed after approval. Review the updated plan and approve again."
            }
            IntegrationError::NetworkUnavailable => {
                "Ghost could not reach the network. Check your connection and try again."
            }
        }
    }
}

impl std::fmt::Display for IntegrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.user_message())
    }
}

impl std::error::Error for IntegrationError {}
