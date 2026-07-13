//! Delegated OAuth scopes for Google Cloud Storage export.

pub mod storage {
    /// Read/write objects in buckets the user can access. v1 uses a single
    /// bucket picker in Settings to keep scope narrow in practice.
    pub const SCOPES: &[&str] = &[
        "openid",
        "email",
        "profile",
        "https://www.googleapis.com/auth/devstorage.read_write",
    ];
}
