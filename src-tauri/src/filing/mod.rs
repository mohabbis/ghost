//! Audience-aware **filing** layer: deterministic, name-only classification that
//! proposes where recurring files belong, per audience "profile".
//!
//! Ghost is a general tool, not a single-industry one. The same trust pipeline
//! (preview -> approve -> execute -> audit -> undo, owned by the Organizer)
//! serves many audiences; this layer adds the domain knowledge that makes the
//! *preview* useful for each:
//!
//! - [`finance`] — recurring financial reports by type + reporting period
//!   (finance/operations admins).
//! - [`academic`] — student coursework by course + term + assignment type.
//! - [`engineering`] — software automation artifacts by type + run period.
//!
//! Everything here is pure and deterministic: it reads only the file *name* and
//! extension, never file contents, the network, or a model. It proposes; the
//! user approves; the deterministic executor does the actual, reversible move.

pub mod academic;
pub mod engineering;
pub mod finance;
pub mod period;
pub mod preview;
pub mod savings;

pub use period::Period;
pub use preview::{Audience, FiledItem, FilingPreview, preview_filing};
pub use savings::{SavingsEstimate, SavingsInputs, estimate_savings};
