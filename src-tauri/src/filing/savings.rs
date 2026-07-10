//! Deterministic time / cost **savings estimator** for repetitive filing.
//!
//! Pure arithmetic with an explicit, inspectable model — no hidden magic. It
//! turns a few honest inputs (how many files, how often, how long each takes by
//! hand) into an annual time-and-cost estimate for doing the same filing with a
//! reviewed, assisted workflow. Every default used is echoed back in
//! [`SavingsEstimate::assumptions`] so the number is defensible, not a guess.
//!
//! Audience-agnostic: a finance team can supply a loaded hourly rate to get a
//! dollar figure; a student can leave it out and still see hours saved.

use serde::{Deserialize, Serialize};

/// Default assisted handling time per file (minutes): time to glance at a
/// proposed destination and approve it.
pub const DEFAULT_ASSISTED_MINUTES: f64 = 0.5;
/// Default minutes to correct one misfiled/mis-keyed file when done by hand.
pub const DEFAULT_REWORK_MINUTES: f64 = 10.0;

/// Inputs to the savings model. Only the first three fields are required in
/// practice; the rest have documented defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavingsInputs {
    /// Average number of files handled each period.
    pub files_per_period: f64,
    /// Periods per year (12 = monthly, 4 = quarterly, 52 = weekly, 1 = annual).
    pub periods_per_year: f64,
    /// Minutes spent per file doing it by hand today (locate, rename, file).
    pub minutes_per_file_manual: f64,
    /// Minutes per file with the assisted workflow. Defaults to
    /// [`DEFAULT_ASSISTED_MINUTES`]; clamped to never exceed the manual time.
    #[serde(default)]
    pub minutes_per_file_assisted: Option<f64>,
    /// Fully-loaded hourly labor cost. When `None`, only time figures are
    /// produced (no dollar estimate).
    #[serde(default)]
    pub hourly_rate: Option<f64>,
    /// Fraction of files (`0.0..=1.0`) that get misfiled by hand and need
    /// rework. Defaults to `0.0` (conservative — no error savings claimed).
    #[serde(default)]
    pub manual_error_rate: Option<f64>,
    /// Minutes to fix one such error. Defaults to [`DEFAULT_REWORK_MINUTES`].
    #[serde(default)]
    pub minutes_per_error_rework: Option<f64>,
}

/// The computed annual estimate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavingsEstimate {
    pub files_per_year: f64,
    pub manual_hours_per_year: f64,
    pub assisted_hours_per_year: f64,
    pub hours_saved_per_year: f64,
    pub rework_hours_avoided_per_year: f64,
    pub manual_cost_per_year: Option<f64>,
    pub assisted_cost_per_year: Option<f64>,
    pub cost_saved_per_year: Option<f64>,
    /// Percentage of manual time eliminated (`0.0..=100.0`).
    pub reduction_pct: f64,
    /// Human-readable list of the assumptions/defaults applied.
    pub assumptions: Vec<String>,
}

fn round(value: f64, places: i32) -> f64 {
    if !value.is_finite() {
        return 0.0;
    }
    let factor = 10f64.powi(places);
    (value * factor).round() / factor
}

/// Compute an annual time/cost savings estimate from `inputs`.
///
/// Deterministic and defensive: negative inputs are treated as zero, the error
/// rate is clamped to `0.0..=1.0`, and assisted time can never exceed manual
/// time. Assisted rework is assumed to be zero — a reviewed, structured filing
/// step is what avoids the hand-keying mistakes — and that assumption is
/// disclosed in the returned `assumptions`.
pub fn estimate_savings(inputs: &SavingsInputs) -> SavingsEstimate {
    let files_per_period = inputs.files_per_period.max(0.0);
    let periods_per_year = inputs.periods_per_year.max(0.0);
    let manual_min = inputs.minutes_per_file_manual.max(0.0);

    let mut assumptions = Vec::new();

    let assisted_min = match inputs.minutes_per_file_assisted {
        Some(m) if m >= 0.0 => m.min(manual_min),
        _ => {
            assumptions.push(format!(
                "Assisted handling assumed at {DEFAULT_ASSISTED_MINUTES} min/file (review + approve)."
            ));
            DEFAULT_ASSISTED_MINUTES.min(manual_min)
        }
    };

    let error_rate = match inputs.manual_error_rate {
        Some(r) => r.clamp(0.0, 1.0),
        None => 0.0,
    };
    let rework_min = match inputs.minutes_per_error_rework {
        Some(m) if m >= 0.0 => m,
        _ => DEFAULT_REWORK_MINUTES,
    };
    if error_rate > 0.0 {
        assumptions.push(format!(
            "Manual error rate {:.0}% at {rework_min} min rework/file; assisted rework assumed 0 (reviewed before filing).",
            error_rate * 100.0
        ));
    }

    let files_per_year = files_per_period * periods_per_year;

    let manual_base_hours = files_per_year * manual_min / 60.0;
    let rework_hours = files_per_year * error_rate * rework_min / 60.0;
    let manual_hours = manual_base_hours + rework_hours;
    let assisted_hours = files_per_year * assisted_min / 60.0;
    let hours_saved = (manual_hours - assisted_hours).max(0.0);

    let reduction_pct = if manual_hours > 0.0 {
        (hours_saved / manual_hours) * 100.0
    } else {
        0.0
    };

    let (manual_cost, assisted_cost, cost_saved) = match inputs.hourly_rate {
        Some(rate) if rate >= 0.0 => (
            Some(round(manual_hours * rate, 2)),
            Some(round(assisted_hours * rate, 2)),
            Some(round(hours_saved * rate, 2)),
        ),
        _ => (None, None, None),
    };

    if assumptions.is_empty() {
        assumptions.push("All values supplied by caller; no defaults applied.".to_string());
    }

    SavingsEstimate {
        files_per_year: round(files_per_year, 1),
        manual_hours_per_year: round(manual_hours, 1),
        assisted_hours_per_year: round(assisted_hours, 1),
        hours_saved_per_year: round(hours_saved, 1),
        rework_hours_avoided_per_year: round(rework_hours, 1),
        manual_cost_per_year: manual_cost,
        assisted_cost_per_year: assisted_cost,
        cost_saved_per_year: cost_saved,
        reduction_pct: round(reduction_pct, 1),
        assumptions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> SavingsInputs {
        SavingsInputs {
            files_per_period: 20.0,
            periods_per_year: 12.0,
            minutes_per_file_manual: 5.0,
            minutes_per_file_assisted: None,
            hourly_rate: None,
            manual_error_rate: None,
            minutes_per_error_rework: None,
        }
    }

    #[test]
    fn computes_basic_time_savings() {
        let est = estimate_savings(&base());
        assert_eq!(est.files_per_year, 240.0);
        // 240 * 5 / 60 = 20 manual hours
        assert_eq!(est.manual_hours_per_year, 20.0);
        // 240 * 0.5 / 60 = 2 assisted hours
        assert_eq!(est.assisted_hours_per_year, 2.0);
        assert_eq!(est.hours_saved_per_year, 18.0);
        assert_eq!(est.reduction_pct, 90.0);
        assert!(est.manual_cost_per_year.is_none());
    }

    #[test]
    fn computes_cost_when_rate_supplied() {
        let mut inputs = base();
        inputs.hourly_rate = Some(30.0);
        let est = estimate_savings(&inputs);
        assert_eq!(est.manual_cost_per_year, Some(600.0)); // 20h * $30
        assert_eq!(est.assisted_cost_per_year, Some(60.0)); // 2h * $30
        assert_eq!(est.cost_saved_per_year, Some(540.0));
    }

    #[test]
    fn error_rework_increases_manual_time_and_is_avoided() {
        let mut inputs = base();
        inputs.manual_error_rate = Some(0.1); // 10%
        inputs.minutes_per_error_rework = Some(10.0);
        let est = estimate_savings(&inputs);
        // rework = 240 * 0.1 * 10 / 60 = 4 hours
        assert_eq!(est.rework_hours_avoided_per_year, 4.0);
        // manual = 20 base + 4 rework = 24
        assert_eq!(est.manual_hours_per_year, 24.0);
        assert_eq!(est.hours_saved_per_year, 22.0);
    }

    #[test]
    fn assisted_never_exceeds_manual() {
        let mut inputs = base();
        inputs.minutes_per_file_manual = 0.3;
        inputs.minutes_per_file_assisted = Some(5.0); // absurd; clamp to manual
        let est = estimate_savings(&inputs);
        assert_eq!(est.assisted_hours_per_year, est.manual_hours_per_year);
        assert_eq!(est.hours_saved_per_year, 0.0);
    }

    #[test]
    fn negative_and_nonfinite_inputs_are_defensive() {
        let mut inputs = base();
        inputs.files_per_period = -5.0;
        inputs.minutes_per_file_manual = -1.0;
        let est = estimate_savings(&inputs);
        assert_eq!(est.files_per_year, 0.0);
        assert_eq!(est.manual_hours_per_year, 0.0);
        assert_eq!(est.hours_saved_per_year, 0.0);
        assert_eq!(est.reduction_pct, 0.0);
    }

    #[test]
    fn error_rate_is_clamped() {
        let mut inputs = base();
        inputs.manual_error_rate = Some(5.0); // 500% -> clamp to 100%
        let est = estimate_savings(&inputs);
        // rework = 240 * 1.0 * 10 / 60 = 40 hours
        assert_eq!(est.rework_hours_avoided_per_year, 40.0);
    }

    #[test]
    fn assumptions_are_disclosed() {
        let est = estimate_savings(&base());
        assert!(est
            .assumptions
            .iter()
            .any(|a| a.contains("Assisted handling assumed")));
    }

    #[test]
    fn estimate_is_deterministic() {
        assert_eq!(estimate_savings(&base()), estimate_savings(&base()));
    }
}
