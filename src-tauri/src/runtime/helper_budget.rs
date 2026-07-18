//! Cooperative mid-op budgets for AX / ScreenCaptureKit helper IPC.
//!
//! ActionStep `timeout_ms` already bounds the Rust dispatch/retry loop. Long
//! helper ops (AX tree walks, ScreenCaptureKit still/stream) can still run past
//! that wall clock unless the remaining budget is plumbed into the helper JSON
//! and checked cooperatively inside the helper.
//!
//! `budget_ms` is wall-clock milliseconds from helper request start (not an
//! absolute epoch deadline). Zero means already exhausted — fail closed without
//! spawning work that could mutate UI or write frames.

use std::time::{Duration, Instant};

/// Remaining wall-clock budget for one helper IPC call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HelperBudget {
    pub budget_ms: u32,
}

impl HelperBudget {
    pub fn new(budget_ms: u32) -> Self {
        Self { budget_ms }
    }

    /// Build from a remaining [`Duration`].
    ///
    /// A zero remaining duration yields `budget_ms: 0` so callers can fail
    /// closed before starting heavy work.
    pub fn from_remaining(remaining: Duration) -> Self {
        let ms = remaining.as_millis().min(u128::from(u32::MAX)) as u32;
        Self { budget_ms: ms }
    }

    /// Remaining budget relative to a step timeout started at `started`.
    pub fn from_step_timeout(started: Instant, timeout: Option<Duration>) -> Option<Self> {
        timeout.map(|limit| Self::from_remaining(limit.saturating_sub(started.elapsed())))
    }

    /// Whether the budget is already exhausted (do not start the helper op).
    pub fn is_exhausted(self) -> bool {
        self.budget_ms == 0
    }

    /// Clamp a proposed duration to this budget (budget can only shorten).
    pub fn clamp_duration_ms(self, duration_ms: u32) -> u32 {
        duration_ms.min(self.budget_ms)
    }
}

/// Detail substring the Swift helper uses when a cooperative budget check fails.
pub const HELPER_BUDGET_EXCEEDED: &str = "helper budget exceeded";

/// True when a helper `detail` reports a cooperative mid-op budget timeout.
pub fn is_helper_budget_timeout(detail: &str) -> bool {
    let lower = detail.to_lowercase();
    lower.contains(HELPER_BUDGET_EXCEEDED)
        || lower.contains("helper timed out")
        || (lower.contains("budget") && lower.contains("exceeded"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_remaining_clamps_and_preserves_zero() {
        assert_eq!(HelperBudget::from_remaining(Duration::ZERO).budget_ms, 0);
        assert_eq!(
            HelperBudget::from_remaining(Duration::from_millis(1_500)).budget_ms,
            1_500
        );
        assert_eq!(
            HelperBudget::from_remaining(Duration::from_secs(u64::from(u32::MAX) + 10)).budget_ms,
            u32::MAX
        );
    }

    #[test]
    fn from_step_timeout_none_when_unset() {
        assert!(HelperBudget::from_step_timeout(Instant::now(), None).is_none());
    }

    #[test]
    fn from_step_timeout_tracks_elapsed() {
        let started = Instant::now() - Duration::from_millis(40);
        let budget = HelperBudget::from_step_timeout(started, Some(Duration::from_millis(100)))
            .expect("budget");
        assert!(budget.budget_ms <= 100);

        let exhausted_start = Instant::now() - Duration::from_secs(5);
        let exhausted =
            HelperBudget::from_step_timeout(exhausted_start, Some(Duration::from_millis(100)))
                .expect("budget");
        assert!(exhausted.is_exhausted());
    }

    #[test]
    fn clamp_duration_only_shortens() {
        let b = HelperBudget::new(250);
        assert_eq!(b.clamp_duration_ms(400), 250);
        assert_eq!(b.clamp_duration_ms(100), 100);
        assert_eq!(HelperBudget::new(0).clamp_duration_ms(400), 0);
    }

    #[test]
    fn timeout_detail_detection() {
        assert!(is_helper_budget_timeout("helper budget exceeded"));
        assert!(is_helper_budget_timeout("AX walk: helper budget exceeded"));
        assert!(is_helper_budget_timeout("helper timed out"));
        assert!(!is_helper_budget_timeout("no matching element"));
        assert!(!is_helper_budget_timeout("screen recording denied"));
    }
}
