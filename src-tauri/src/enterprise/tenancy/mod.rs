//! Tenant boundary primitives for enterprise workflows.
//!
//! A tenant scope is local metadata used to label runs and reports. It does not
//! imply cloud storage, telemetry, or cross-tenant network access.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TenantScope {
    pub tenant_id: String,
    pub display_name: String,
    pub allowed_zone_ids: Vec<String>,
}

impl TenantScope {
    pub fn allows_zone(&self, zone_id: &str) -> bool {
        self.allowed_zone_ids
            .iter()
            .any(|allowed| allowed == zone_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tenant_scope_allows_only_listed_zones() {
        let scope = TenantScope {
            tenant_id: "tenant-1".into(),
            display_name: "Local Office".into(),
            allowed_zone_ids: vec!["zone-a".into()],
        };
        assert!(scope.allows_zone("zone-a"));
        assert!(!scope.allows_zone("zone-b"));
    }
}
