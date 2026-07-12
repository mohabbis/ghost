//! Power BI REST connector (planned — read/export only in phase one).

pub struct PowerBiClient;

impl Default for PowerBiClient {
    fn default() -> Self {
        PowerBiClient
    }
}

impl PowerBiClient {
    pub fn new() -> Self {
        Self
    }
}

/// Suggested schema for Ghost operations dashboards (export preview only).
pub mod schema {
    pub const GHOST_RUNS_TABLE: &str = "GhostRuns";
    pub const GHOST_ACTIONS_TABLE: &str = "GhostActions";
    pub const GHOST_POLICY_EVENTS_TABLE: &str = "GhostPolicyEvents";
}
