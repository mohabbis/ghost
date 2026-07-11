//! Audit ledger and undo journal: the passive records behind safe execution.
//!
//! These are pure, serializable data structures with **no filesystem logic**.
//! The [`audit_log`] records *what* the executor did (and what it refused);
//! the [`undo_journal`] records *how to reverse* each reversible operation,
//! captured before the operation runs. The organizer's executor produces them
//! and the organizer's undo runner consumes the journal — keeping all disk
//! mutation in `crate::organizer`, while this module stays a neutral ledger.
//!
//! ```text
//! Intent -> Plan -> Policy -> Approval -> Execution -> Audit -> Undo
//!                                                      ^^^^^^^^^^^^^^
//!                                                      these records
//! ```

pub mod audit_log;
pub mod pii;
pub mod undo_journal;

pub use audit_log::{ActionOutcome, AuditEvent, AuditLog, Provenance};
pub use undo_journal::{UndoJournal, UndoOp};
