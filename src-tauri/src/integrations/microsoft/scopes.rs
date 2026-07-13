//! Delegated OAuth scopes for Microsoft integrations (separate from identity).

pub mod identity {
    pub const SCOPES: &[&str] = &["openid", "email", "profile", "offline_access"];
}

pub mod fabric {
    /// Fabric REST API delegated scope (workspace read + item metadata).
    pub const SCOPES: &[&str] = &["https://api.fabric.microsoft.com/.default"];
}

pub mod power_bi {
    pub const SCOPES: &[&str] = &["https://analysis.windows.net/powerbi/api/.default"];
}
