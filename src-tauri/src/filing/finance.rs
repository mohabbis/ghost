//! Finance/operations profile: classification of recurring financial-report
//! files (one of the [`super`] filing profiles; see also [`super::academic`]).
//!
//! Rules first, AI never (for this layer): a pure function of the file *name*
//! and extension. No network, no model, no IO, no file-content reads. This is
//! the "recognize the report" half of the filing feature; the reporting period
//! is parsed separately in [`super::period`].
//!
//! Each result carries a `confidence` in `0.0..=1.0` and a human-readable
//! `reason` so a review UI can show *why* a file was recognized and flag
//! low-confidence items for manual review before anything is filed.

use serde::{Deserialize, Serialize};

/// A recurring financial-report type Ghost recognizes by filename.
///
/// This is deliberately domain-generic (standard accounting/finance artifacts),
/// not tied to any one company's process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportKind {
    IncomeStatement,
    BalanceSheet,
    CashFlow,
    TrialBalance,
    GeneralLedger,
    Reconciliation,
    AccountsReceivable,
    AccountsPayable,
    Payroll,
    Tax,
    Budget,
    Forecast,
    Expense,
    Invoice,
    Sales,
    BankStatement,
    Compliance,
    Audit,
    /// Recognized as a financial report, but the specific type is unclear.
    Generic,
}

impl ReportKind {
    /// The destination subfolder name for this report type.
    pub fn folder_name(self) -> &'static str {
        match self {
            ReportKind::IncomeStatement => "Income Statements",
            ReportKind::BalanceSheet => "Balance Sheets",
            ReportKind::CashFlow => "Cash Flow Statements",
            ReportKind::TrialBalance => "Trial Balances",
            ReportKind::GeneralLedger => "General Ledger",
            ReportKind::Reconciliation => "Reconciliations",
            ReportKind::AccountsReceivable => "Accounts Receivable",
            ReportKind::AccountsPayable => "Accounts Payable",
            ReportKind::Payroll => "Payroll",
            ReportKind::Tax => "Tax",
            ReportKind::Budget => "Budgets",
            ReportKind::Forecast => "Forecasts",
            ReportKind::Expense => "Expense Reports",
            ReportKind::Invoice => "Invoices",
            ReportKind::Sales => "Sales Reports",
            ReportKind::BankStatement => "Bank Statements",
            ReportKind::Compliance => "Compliance Reports",
            ReportKind::Audit => "Audit",
            ReportKind::Generic => "General Reports",
        }
    }

    /// A short, human-readable label.
    pub fn label(self) -> &'static str {
        match self {
            ReportKind::IncomeStatement => "Income Statement",
            ReportKind::BalanceSheet => "Balance Sheet",
            ReportKind::CashFlow => "Cash Flow Statement",
            ReportKind::TrialBalance => "Trial Balance",
            ReportKind::GeneralLedger => "General Ledger",
            ReportKind::Reconciliation => "Reconciliation",
            ReportKind::AccountsReceivable => "Accounts Receivable",
            ReportKind::AccountsPayable => "Accounts Payable",
            ReportKind::Payroll => "Payroll",
            ReportKind::Tax => "Tax",
            ReportKind::Budget => "Budget",
            ReportKind::Forecast => "Forecast",
            ReportKind::Expense => "Expense Report",
            ReportKind::Invoice => "Invoice",
            ReportKind::Sales => "Sales Report",
            ReportKind::BankStatement => "Bank Statement",
            ReportKind::Compliance => "Compliance Report",
            ReportKind::Audit => "Audit",
            ReportKind::Generic => "Financial Report",
        }
    }
}

/// The outcome of recognizing one file as a financial report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportClassification {
    pub kind: ReportKind,
    /// Confidence in `0.0..=1.0`. Below [`LOW_CONFIDENCE`] the report is flagged
    /// for manual review before filing.
    pub confidence: f32,
    /// Whether the file is a spreadsheet (xlsx/csv/…), the common shape of a
    /// manually filed financial report.
    pub is_spreadsheet: bool,
    /// Why this type was chosen — surfaced to the user.
    pub reason: String,
}

/// Reports at or below this confidence are surfaced as "needs review".
pub const LOW_CONFIDENCE: f32 = 0.5;

const SPECIFIC_CONFIDENCE: f32 = 0.9;
const GENERIC_CONFIDENCE: f32 = 0.55;

/// Spreadsheet-family extensions — the typical container for a manually filed
/// financial report.
const SPREADSHEET_EXTS: &[&str] = &[
    "xls", "xlsx", "xlsm", "xlsb", "xltx", "ods", "csv", "tsv", "numbers", "gsheet",
];

/// Extensions we accept as *possible* report containers at all. A `.mp4` named
/// "invoice" is not a report; only document/spreadsheet-shaped files qualify.
const REPORT_EXTS: &[&str] = &[
    "xls", "xlsx", "xlsm", "xlsb", "xltx", "ods", "csv", "tsv", "numbers", "gsheet", "pdf", "doc",
    "docx", "rtf", "odt", "txt", "md", "pages", "xml", "json",
];

/// Ordered keyword table. **First match wins**, so more specific / less
/// ambiguous phrases must come before broad ones (e.g. "bank statement" before
/// a bare "statement", "expense report" before "expense").
#[allow(clippy::type_complexity)]
fn keyword_table() -> &'static [(&'static [&'static str], ReportKind)] {
    &[
        (
            &[
                "income statement",
                "profit and loss",
                "profit & loss",
                "p&l",
                "p and l",
                "statement of operations",
                "pnl",
            ],
            ReportKind::IncomeStatement,
        ),
        (
            &["balance sheet", "statement of financial position"],
            ReportKind::BalanceSheet,
        ),
        (
            &["cash flow", "cashflow", "statement of cash flows"],
            ReportKind::CashFlow,
        ),
        (&["trial balance"], ReportKind::TrialBalance),
        (
            &["general ledger", "gl detail", "g/l", "gl_detail"],
            ReportKind::GeneralLedger,
        ),
        (
            &["reconciliation", "reconcile", "bank rec", "recon"],
            ReportKind::Reconciliation,
        ),
        (
            &[
                "accounts receivable",
                "receivable",
                "ar aging",
                "ar_aging",
                "a/r",
                "aging receivable",
            ],
            ReportKind::AccountsReceivable,
        ),
        (
            &["accounts payable", "payable", "ap aging", "ap_aging", "a/p"],
            ReportKind::AccountsPayable,
        ),
        (
            &[
                "payroll",
                "paystub",
                "pay stub",
                "pay_stub",
                "wages",
                "timesheet",
            ],
            ReportKind::Payroll,
        ),
        (
            &[
                "sales tax",
                "tax return",
                "1099",
                "w-2",
                "w2",
                "941",
                "vat",
                "tax",
                "irs",
            ],
            ReportKind::Tax,
        ),
        (&["budget"], ReportKind::Budget),
        (
            &["forecast", "projection", "pro forma", "proforma"],
            ReportKind::Forecast,
        ),
        (
            &["expense report", "expense_report", "expenses", "expense"],
            ReportKind::Expense,
        ),
        (&["invoice"], ReportKind::Invoice),
        (
            &[
                "compliance",
                "regulatory",
                "bsa",
                "aml",
                "kyc",
                "sar",
                "ctr",
            ],
            ReportKind::Compliance,
        ),
        (&["audit"], ReportKind::Audit),
        (
            &["bank statement", "bank_statement"],
            ReportKind::BankStatement,
        ),
        (
            &[
                "sales report",
                "sales_report",
                "revenue",
                "bookings",
                "sales",
            ],
            ReportKind::Sales,
        ),
    ]
}

/// Broad "this is a financial report" signals that pick [`ReportKind::Generic`]
/// when no specific type matched.
const GENERIC_HINTS: &[&str] = &[
    "financial",
    "financials",
    "fiscal",
    "period end",
    "month end",
    "quarter end",
    "year end",
    "statement",
    "ledger",
];

fn is_spreadsheet_ext(ext: &str) -> bool {
    SPREADSHEET_EXTS.contains(&ext)
}

fn is_report_ext(ext: &str) -> bool {
    REPORT_EXTS.contains(&ext)
}

/// Split a file name into `(stem_lowercased, extension_lowercased)`.
fn split_name(file_name: &str) -> (String, Option<String>) {
    let path = std::path::Path::new(file_name);
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase());
    (stem, ext)
}

/// Classify a file name as a financial report, or return `None` when nothing
/// about the name/extension suggests a financial report.
///
/// Pure and deterministic: same input always yields the same output. Only the
/// file *name* and extension are inspected — never file contents.
pub fn classify_report(file_name: &str) -> Option<ReportClassification> {
    let (stem, ext) = split_name(file_name);
    let ext_ref = ext.as_deref();

    // Only document/spreadsheet-shaped files can be reports.
    match ext_ref {
        Some(e) if is_report_ext(e) => {}
        _ => return None,
    }

    let is_spreadsheet = ext_ref.is_some_and(is_spreadsheet_ext);

    // A `.`-normalized haystack so "p&l", "a/r", etc. match regardless of the
    // separators used in the file name.
    let haystack = normalize(&stem);

    for (needles, kind) in keyword_table() {
        if let Some(hit) = needles.iter().find(|n| haystack.contains(**n)) {
            return Some(ReportClassification {
                kind: *kind,
                confidence: SPECIFIC_CONFIDENCE,
                is_spreadsheet,
                reason: format!("matched \"{hit}\" -> {}", kind.label()),
            });
        }
    }

    if let Some(hit) = GENERIC_HINTS.iter().find(|n| haystack.contains(**n)) {
        return Some(ReportClassification {
            kind: ReportKind::Generic,
            confidence: GENERIC_CONFIDENCE,
            is_spreadsheet,
            reason: format!("matched general financial term \"{hit}\""),
        });
    }

    None
}

/// Lowercase and normalize separators so keyword matching is robust to the many
/// ways report names are punctuated (`_`, `-`, `.`, multiple spaces).
fn normalize(stem: &str) -> String {
    let mut out = String::with_capacity(stem.len());
    let mut last_space = false;
    for c in stem.chars() {
        // Keep `&` and `/` (needed by "p&l", "a/r"); turn structural separators
        // into single spaces so "income_statement" == "income statement".
        let mapped = match c {
            '_' | '-' | '.' => ' ',
            other => other,
        };
        if mapped == ' ' {
            if !last_space {
                out.push(' ');
            }
            last_space = true;
        } else {
            out.push(mapped);
            last_space = false;
        }
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_specific_report_types() {
        let cases = [
            ("Q2 Income Statement.xlsx", ReportKind::IncomeStatement),
            ("balance_sheet_2026.xlsx", ReportKind::BalanceSheet),
            ("Statement of Cash Flows June.pdf", ReportKind::CashFlow),
            ("Trial Balance 2026-06.csv", ReportKind::TrialBalance),
            ("General Ledger detail.xlsx", ReportKind::GeneralLedger),
            ("Bank Reconciliation May.xlsx", ReportKind::Reconciliation),
            ("AR Aging 2026-06.xlsx", ReportKind::AccountsReceivable),
            ("AP_Aging_Q1.xlsx", ReportKind::AccountsPayable),
            ("Payroll Register 2026-06.csv", ReportKind::Payroll),
            ("Sales Tax Return Q2.pdf", ReportKind::Tax),
            ("2026 Operating Budget.xlsx", ReportKind::Budget),
            ("Revenue Forecast FY2027.xlsx", ReportKind::Forecast),
            ("Expense Report - travel.xlsx", ReportKind::Expense),
            ("invoice_4471.pdf", ReportKind::Invoice),
            ("BSA Compliance Report.pdf", ReportKind::Compliance),
            ("Bank Statement 2026-06.pdf", ReportKind::BankStatement),
            ("Monthly Sales Report.xlsx", ReportKind::Sales),
        ];
        for (name, expected) in cases {
            let c = classify_report(name).unwrap_or_else(|| panic!("no match for {name}"));
            assert_eq!(c.kind, expected, "file {name}");
            assert!(c.confidence > LOW_CONFIDENCE, "file {name} low confidence");
        }
    }

    #[test]
    fn profit_and_loss_variants_map_to_income_statement() {
        for name in ["P&L 2026.xlsx", "profit_and_loss.csv", "PnL_Q3.xlsx"] {
            assert_eq!(
                classify_report(name).unwrap().kind,
                ReportKind::IncomeStatement,
                "{name}"
            );
        }
    }

    #[test]
    fn generic_financial_terms_fall_back_to_generic() {
        let c = classify_report("Financials_2026-06.xlsx").unwrap();
        assert_eq!(c.kind, ReportKind::Generic);
        assert_eq!(c.confidence, GENERIC_CONFIDENCE);
    }

    #[test]
    fn non_report_files_return_none() {
        assert!(classify_report("vacation_photo.jpg").is_none());
        assert!(classify_report("invoice.mp4").is_none()); // wrong container
        assert!(classify_report("song.mp3").is_none());
        assert!(classify_report("random_notes.xlsx").is_none()); // no financial signal
    }

    #[test]
    fn spreadsheet_flag_reflects_extension() {
        assert!(classify_report("budget.xlsx").unwrap().is_spreadsheet);
        assert!(classify_report("budget.csv").unwrap().is_spreadsheet);
        assert!(!classify_report("budget.pdf").unwrap().is_spreadsheet);
    }

    #[test]
    fn more_specific_keyword_wins_over_generic() {
        // "bank statement" must beat the generic "statement" hint.
        assert_eq!(
            classify_report("Bank Statement 2026-06.pdf").unwrap().kind,
            ReportKind::BankStatement
        );
        // "expense report" beats a bare "expense".
        assert_eq!(
            classify_report("Expense Report Q2.xlsx").unwrap().kind,
            ReportKind::Expense
        );
    }

    #[test]
    fn classification_is_deterministic() {
        assert_eq!(
            classify_report("Q2 Income Statement.xlsx"),
            classify_report("Q2 Income Statement.xlsx")
        );
    }
}
