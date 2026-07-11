//! Deterministic invoice/ledger reconciliation matching.
//!
//! Pure, local-first: no network, no AI, no mutation. Given invoices and
//! ledger/statement lines the user has already provided as local input,
//! [`propose_matches`] scores plausible pairings and tags each with a
//! [`ReviewRequirement`]. Ghost may prepare, classify, compare, and recommend
//! reconciliation matches this way; it must never post a journal entry,
//! release payment, or apply a match itself — that stays a separate,
//! policy-gated, audited, undoable step outside this module.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// One line from an external ledger/bank statement export. Read-only input;
/// Ghost never fetches this from a bank API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LedgerLine {
    pub id: String,
    pub reference: Option<String>,
    pub amount_cents: i64,
    pub date: NaiveDate,
    pub description: String,
}

/// One invoice record to reconcile against the ledger.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InvoiceRecord {
    pub id: String,
    pub reference: Option<String>,
    pub amount_cents: i64,
    pub date: NaiveDate,
    pub vendor: String,
}

/// Whether a proposed match qualifies for the auto-apply/batch-select-all
/// bucket or must always wait for an explicit human decision. Either way,
/// applying the match still goes through Ghost's normal policy check,
/// approval, audit log, and undo write — this only decides whether the
/// approval step can be a single click across many items or must be
/// per-item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewRequirement {
    WithinAutoApplyThreshold,
    RequiresReview,
}

/// Why a match scored the way it did, surfaced to the review UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchReason {
    pub exact_reference: bool,
    pub amount_delta_cents: i64,
    pub date_delta_days: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProposedMatch {
    pub ledger_line_id: String,
    pub invoice_id: String,
    pub confidence: f32,
    pub reason: MatchReason,
    pub review: ReviewRequirement,
}

/// An invoice with no plausible ledger line at all — always a real
/// exception, always surfaced to the human, never auto-applied.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnmatchedInvoice {
    pub invoice_id: String,
    pub reason: String,
}

/// Tolerances used to decide whether an invoice and a ledger line plausibly
/// refer to the same transaction. Set explicitly by the user, same as a
/// Zone boundary; never inferred or widened automatically.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchTolerance {
    pub max_amount_delta_cents: i64,
    pub max_date_delta_days: i64,
}

impl Default for MatchTolerance {
    fn default() -> Self {
        MatchTolerance {
            max_amount_delta_cents: 0,
            max_date_delta_days: 5,
        }
    }
}

/// Confidence assigned when the reference matches exactly and the amount is
/// exact.
const EXACT_REFERENCE_CONFIDENCE: f32 = 0.98;
/// Confidence assigned when amount and date both fall within tolerance but
/// there was no exact reference match.
const AMOUNT_AND_DATE_CONFIDENCE: f32 = 0.8;

/// Score one invoice/ledger-line pairing. Returns `None` when the pair is not
/// a plausible match at all (no exact reference+amount hit, and outside
/// amount/date tolerance) — those invoices surface as unmatched, not as a
/// low-confidence guess.
fn score(
    invoice: &InvoiceRecord,
    ledger: &LedgerLine,
    tolerance: &MatchTolerance,
) -> Option<(f32, MatchReason)> {
    let amount_delta = (invoice.amount_cents - ledger.amount_cents).abs();
    let date_delta = (invoice.date - ledger.date).num_days().abs();
    let exact_reference = matches!(
        (&invoice.reference, &ledger.reference),
        (Some(a), Some(b)) if a == b && !a.is_empty()
    );

    let confidence = if exact_reference && amount_delta == 0 {
        EXACT_REFERENCE_CONFIDENCE
    } else if amount_delta <= tolerance.max_amount_delta_cents
        && date_delta <= tolerance.max_date_delta_days
    {
        AMOUNT_AND_DATE_CONFIDENCE
    } else {
        return None;
    };

    Some((
        confidence,
        MatchReason {
            exact_reference,
            amount_delta_cents: amount_delta,
            date_delta_days: date_delta,
        },
    ))
}

/// Thresholds under which a proposed match is safe to sit in the
/// auto-apply/batch-select-all bucket. Set explicitly by the user; never
/// raised by AI suggestion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutoApplyRule {
    pub max_amount_cents: i64,
    pub min_confidence: f32,
    pub requires_exact_reference: bool,
}

impl AutoApplyRule {
    fn qualifies(&self, invoice: &InvoiceRecord, m: &ProposedMatch) -> bool {
        m.confidence >= self.min_confidence
            && invoice.amount_cents.abs() <= self.max_amount_cents
            && (!self.requires_exact_reference || m.reason.exact_reference)
    }
}

/// Propose matches for every invoice against the ledger lines, tag each with
/// a [`ReviewRequirement`], and report invoices with no plausible match.
///
/// Pure function: no IO, no network, no mutation. Each ledger line can be
/// claimed by at most one invoice; invoices are scored against remaining
/// unclaimed lines in the order given, taking the best-scoring plausible
/// line. Applying a match is always a separate, policy-gated step this
/// function never performs.
pub fn propose_matches(
    invoices: &[InvoiceRecord],
    ledger: &[LedgerLine],
    tolerance: &MatchTolerance,
    auto_apply: &AutoApplyRule,
) -> (Vec<ProposedMatch>, Vec<UnmatchedInvoice>) {
    let mut matches = Vec::new();
    let mut unmatched = Vec::new();
    let mut claimed: HashSet<String> = HashSet::new();

    for invoice in invoices {
        let best = ledger
            .iter()
            .filter(|line| !claimed.contains(&line.id))
            .filter_map(|line| score(invoice, line, tolerance).map(|(c, r)| (c, line, r)))
            .max_by(|(a, ..), (b, ..)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        match best {
            Some((confidence, line, reason)) => {
                claimed.insert(line.id.clone());
                let mut proposed = ProposedMatch {
                    ledger_line_id: line.id.clone(),
                    invoice_id: invoice.id.clone(),
                    confidence,
                    reason,
                    review: ReviewRequirement::RequiresReview,
                };
                proposed.review = if auto_apply.qualifies(invoice, &proposed) {
                    ReviewRequirement::WithinAutoApplyThreshold
                } else {
                    ReviewRequirement::RequiresReview
                };
                matches.push(proposed);
            }
            None => unmatched.push(UnmatchedInvoice {
                invoice_id: invoice.id.clone(),
                reason: "no ledger line within amount/date tolerance".to_string(),
            }),
        }
    }

    (matches, unmatched)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn invoice(
        id: &str,
        reference: Option<&str>,
        amount_cents: i64,
        date: NaiveDate,
    ) -> InvoiceRecord {
        InvoiceRecord {
            id: id.to_string(),
            reference: reference.map(str::to_string),
            amount_cents,
            date,
            vendor: "Acme".to_string(),
        }
    }

    fn ledger_line(
        id: &str,
        reference: Option<&str>,
        amount_cents: i64,
        date: NaiveDate,
    ) -> LedgerLine {
        LedgerLine {
            id: id.to_string(),
            reference: reference.map(str::to_string),
            amount_cents,
            date,
            description: "wire".to_string(),
        }
    }

    fn permissive_auto_apply() -> AutoApplyRule {
        AutoApplyRule {
            max_amount_cents: 100_000,
            min_confidence: 0.9,
            requires_exact_reference: true,
        }
    }

    #[test]
    fn exact_reference_and_amount_match_with_high_confidence() {
        let invoices = vec![invoice("inv-1", Some("PO-42"), 10_000, date(2026, 1, 10))];
        let ledger = vec![ledger_line(
            "ln-1",
            Some("PO-42"),
            10_000,
            date(2026, 1, 12),
        )];

        let (matches, unmatched) = propose_matches(
            &invoices,
            &ledger,
            &MatchTolerance::default(),
            &permissive_auto_apply(),
        );

        assert!(unmatched.is_empty());
        assert_eq!(matches.len(), 1);
        assert!(matches[0].reason.exact_reference);
        assert_eq!(matches[0].confidence, EXACT_REFERENCE_CONFIDENCE);
        assert_eq!(
            matches[0].review,
            ReviewRequirement::WithinAutoApplyThreshold
        );
    }

    #[test]
    fn amount_and_date_within_tolerance_but_no_reference_requires_review() {
        let invoices = vec![invoice("inv-1", None, 25_000, date(2026, 2, 1))];
        let ledger = vec![ledger_line("ln-1", None, 25_000, date(2026, 2, 3))];

        let (matches, unmatched) = propose_matches(
            &invoices,
            &ledger,
            &MatchTolerance::default(),
            &permissive_auto_apply(),
        );

        assert!(unmatched.is_empty());
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].confidence, AMOUNT_AND_DATE_CONFIDENCE);
        // requires_exact_reference in the rule means this can't auto-apply.
        assert_eq!(matches[0].review, ReviewRequirement::RequiresReview);
    }

    #[test]
    fn outside_tolerance_is_unmatched_not_a_guess() {
        let invoices = vec![invoice("inv-1", None, 25_000, date(2026, 2, 1))];
        let ledger = vec![ledger_line("ln-1", None, 24_000, date(2026, 2, 1))];

        let (matches, unmatched) = propose_matches(
            &invoices,
            &ledger,
            &MatchTolerance::default(),
            &permissive_auto_apply(),
        );

        assert!(matches.is_empty());
        assert_eq!(unmatched.len(), 1);
        assert_eq!(unmatched[0].invoice_id, "inv-1");
    }

    #[test]
    fn each_ledger_line_claimed_at_most_once() {
        let invoices = vec![
            invoice("inv-1", Some("PO-1"), 10_000, date(2026, 1, 1)),
            invoice("inv-2", Some("PO-1"), 10_000, date(2026, 1, 1)),
        ];
        let ledger = vec![ledger_line("ln-1", Some("PO-1"), 10_000, date(2026, 1, 1))];

        let (matches, unmatched) = propose_matches(
            &invoices,
            &ledger,
            &MatchTolerance::default(),
            &permissive_auto_apply(),
        );

        assert_eq!(matches.len(), 1);
        assert_eq!(unmatched.len(), 1);
        assert_eq!(matches[0].invoice_id, "inv-1");
        assert_eq!(unmatched[0].invoice_id, "inv-2");
    }

    #[test]
    fn auto_apply_rule_rejects_amounts_over_threshold() {
        let invoices = vec![invoice("inv-1", Some("PO-9"), 999_999_00, date(2026, 1, 1))];
        let ledger = vec![ledger_line(
            "ln-1",
            Some("PO-9"),
            999_999_00,
            date(2026, 1, 1),
        )];
        let strict = AutoApplyRule {
            max_amount_cents: 100_000,
            min_confidence: 0.9,
            requires_exact_reference: true,
        };

        let (matches, _) = propose_matches(&invoices, &ledger, &MatchTolerance::default(), &strict);

        assert_eq!(matches[0].review, ReviewRequirement::RequiresReview);
    }
}
