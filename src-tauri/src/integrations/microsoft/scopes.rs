//! Delegated OAuth scopes for Microsoft integrations (separate from identity).

pub mod identity {
    pub const SCOPES: &[&str] = &["openid", "email", "profile", "offline_access"];
}

pub mod fabric {
    /// Placeholder — exact Fabric scopes to be finalized against Microsoft docs.
    pub const SCOPES: &[&str] = &[];
}

pub mod power_bi {
    pub const SCOPES: &[&str] = &["https://analysis.windows.net/powerbi/api/.default"];
}
