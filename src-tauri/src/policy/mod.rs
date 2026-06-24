//! Policy engine: the central trust mechanism.
//!
//! Ghost's product rule is "may suggest anything, may only do what the user
//! approved inside a boundary the user controls." Every meaningful operation is
//! expressed as a [`Capability`] and evaluated by [`evaluate`] against the
//! active [`FolderRule`]s, returning a [`PolicyDecision`]. The engine is pure
//! and deny-by-default; persistence of Zones/rules lives in `crate::storage`.

pub mod capability;
pub mod decision;
pub mod engine;
pub mod risk;
pub mod zone;

pub use capability::Capability;
pub use decision::PolicyDecision;
pub use engine::evaluate;
pub use risk::RiskLevel;
pub use zone::{DefaultDecision, FolderRule, Zone};
