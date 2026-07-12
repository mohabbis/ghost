//! Layer B: business-system connectors (Fabric, Power BI, Google Cloud).
//!
//! These are not intelligence providers. They exchange data under separate
//! integration grants, never bypass Ghost's approval pipeline.

pub mod audit;
pub mod errors;
pub mod microsoft;

pub use audit::{AuditActor, IntegrationAuditEvent};
pub use errors::IntegrationConnectorError;
pub use microsoft::MicrosoftIntegrationService;
