//! Canonical workflow benchmarks
//!
//! These tests verify Ghost's core workflows against non-negotiable success rates.
//! See docs/canonical-workflows.md for detailed specs.
//!
//! Target success rate: ≥99.5% for Organizer workflows
//! Target success rate: ≥99.0% for desktop cleanup workflows

#[cfg(test)]
mod canonical_workflows {
    /// Build a [`ghost_lib::organizer::scanner::ScannedFile`] from just a
    /// filename, for feeding the real classifier in these tests without
    /// touching a real filesystem.
    fn scanned_file(filename: &str) -> ghost_lib::organizer::scanner::ScannedFile {
        let path = std::path::Path::new(filename);
        ghost_lib::organizer::scanner::ScannedFile {
            path: path.to_path_buf(),
            file_name: filename.to_string(),
            stem: path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| filename.to_string()),
            extension: path
                .extension()
                .map(|e| e.to_string_lossy().to_ascii_lowercase()),
            size: 0,
            modified: None,
            created: None,
        }
    }

    /// Test 1: Invoice Filing Workflow
    ///
    /// Scenario:
    /// - Source: simulated Downloads folder with mixed files
    /// - Goal: Move invoices to client folders
    ///
    /// Success criteria:
    /// - 100% of test invoices correctly classified by the real classifier
    /// - Zero false positives (non-invoices classified as Invoices)
    #[test]
    fn test_canonical_invoice_filing_workflow() {
        use ghost_lib::organizer::classifier::{classify, Category};

        let test_files = [
            ("invoice_acme_2026_06.pdf", Category::Invoices),
            ("invoice_acme_2026_07.pdf", Category::Invoices),
            ("INVOICE_CLIENT.pdf", Category::Invoices),
            ("receipt_uber.pdf", Category::Receipts),
            ("statement_chase.pdf", Category::Statements),
            ("IMG_4921.png", Category::Images),
            ("screenshot_2026.png", Category::Images),
        ];

        let mut correct_classifications = 0;
        for (filename, expected_category) in test_files.iter() {
            let classification = classify(&scanned_file(filename));
            if classification.category == *expected_category {
                correct_classifications += 1;
            }
            if *expected_category != Category::Invoices {
                assert_ne!(
                    classification.category,
                    Category::Invoices,
                    "false positive: {filename} classified as an invoice"
                );
            }
        }

        let accuracy = correct_classifications as f64 / test_files.len() as f64;
        assert!(
            accuracy >= 0.95,
            "Invoice filing accuracy {:.2}% < 95% target",
            accuracy * 100.0
        );
    }

    /// Test 2: Dated Rename Determinism
    ///
    /// Scenario: the opt-in "rename_dated" Zone setting prefixes a proposed
    /// filename with its file's `YYYY-MM` filing period (see
    /// `organizer::naming::dated_prefix`; there is no client-name component —
    /// that was never a real Ghost feature, only an illustrative label in the
    /// original version of this test).
    ///
    /// Success criteria:
    /// - Correct `YYYY-MM name` format
    /// - Deterministic: same input -> same output every time
    /// - Idempotent: re-prefixing an already-prefixed name is a no-op (so
    ///   replanning never stacks date prefixes)
    #[test]
    fn test_canonical_client_file_renaming() {
        use ghost_lib::organizer::naming::dated_prefix;
        use std::time::{Duration, UNIX_EPOCH};

        // 2026-07-04T00:00:00Z
        let july_2026 = UNIX_EPOCH + Duration::from_secs(1_783_123_200);

        let test_cases = ["invoice.pdf", "statement.pdf", "receipt.pdf"];

        for filename in test_cases.iter() {
            // Run 5 times and verify all results are identical (determinism).
            let results: Vec<String> = (0..5).map(|_| dated_prefix(filename, july_2026)).collect();
            for i in 1..results.len() {
                assert_eq!(
                    results[0], results[i],
                    "dated_prefix is not deterministic for {}: got {} and {}",
                    filename, results[0], results[i]
                );
            }

            assert_eq!(results[0], format!("2026-07 {filename}"));

            // Re-prefixing the already-prefixed name must not stack a second one.
            let twice = dated_prefix(&results[0], july_2026);
            assert_eq!(
                twice, results[0],
                "dated_prefix must be idempotent on an already-prefixed name"
            );
        }
    }

    // Test 3 ("CSV Export Organization": routing `sales_*`/`inventory_*`/
    // `report_*` CSVs into distinct reporting folders) and Test 5 ("Project
    // Archival by Date": archiving files older than 365 days) were removed.
    // Neither scenario is implemented anywhere in `organizer::classifier` or
    // `organizer::planner` — the real classifier routes every `.csv` to the
    // same `Category::Spreadsheets` regardless of filename prefix, and there
    // is no age-based archival logic in the codebase at all. Both tests
    // asserted a private mock function against itself and passed
    // unconditionally, giving the false impression that Ghost already ships
    // these two behaviors. If/when either becomes a real feature, it should
    // get a test wired to that real code, not a revived mock.

    /// Test 4: Desktop Cleanup Categorization
    ///
    /// Scenario:
    /// - Desktop contains screenshots, PDFs, notes, exports
    /// - Ghost categorizes by file type
    ///
    /// Success criteria:
    /// - The real classifier groups files into clear categories
    /// - An unknown extension is flagged low-confidence for review, not
    ///   silently guessed
    #[test]
    fn test_canonical_desktop_cleanup() {
        use ghost_lib::organizer::classifier::{classify, Category, LOW_CONFIDENCE};

        let desktop_files = [
            ("screenshot_2026-07-01.png", Category::Images),
            ("screenshot_2026-07-02.png", Category::Images),
            ("notes.txt", Category::Documents),
            ("report.pdf", Category::Documents),
            ("data.xlsx", Category::Spreadsheets),
            ("README.md", Category::Documents),
            ("IMG_0001.jpg", Category::Images),
        ];

        let mut categorized = 0;
        for (filename, expected_category) in desktop_files.iter() {
            let classification = classify(&scanned_file(filename));
            if classification.category == *expected_category {
                categorized += 1;
            }
        }
        assert_eq!(
            categorized,
            desktop_files.len(),
            "every file with a known extension should classify correctly"
        );

        // A file with no extension at all must not be silently guessed into a
        // real category — it should fall back to `Other` at low confidence so
        // the plan flags it for manual review.
        let unknown = classify(&scanned_file("random_file_no_ext"));
        assert_eq!(unknown.category, Category::Other);
        assert!(
            unknown.confidence <= LOW_CONFIDENCE,
            "an unclassifiable file must be flagged low-confidence, not guessed"
        );
    }

    /// Integration test: Organizer Trust Pipeline Stages
    ///
    /// Drives the real backend through the same sequence
    /// `commands/organizer.rs` runs — Intent -> Plan -> Policy -> Approval ->
    /// Execute -> Audit -> Undo — against a real temp filesystem and a real
    /// SQLite schema (in-memory). This replaces a previous version of this
    /// test that only asserted hardcoded local variables against themselves
    /// and could not fail no matter what the pipeline did.
    ///
    /// It stops one layer short of the `#[tauri::command]` wrappers
    /// themselves: those call `storage::open_default()`, which is hardcoded
    /// to the OS data directory and has no test seam for an in-memory DB. The
    /// commands are thin (open a connection, delegate to exactly these
    /// functions), so this covers the real risk — the trust pipeline logic —
    /// without touching a developer's real `ghost.db`.
    #[test]
    fn test_organizer_trust_pipeline_stages() {
        use ghost_lib::organizer::executor::execute_plan;
        use ghost_lib::organizer::planner::plan_zone;
        use ghost_lib::organizer::undo::revert;
        use ghost_lib::policy::{DefaultDecision, FolderRule, TrustLevel};
        use ghost_lib::storage::executions::{get_execution, save_execution, verify_chain};
        use ghost_lib::storage::migrations::migrate;
        use ghost_lib::storage::zones::{add_folder_rule, create_zone};
        use rusqlite::Connection;
        use std::fs;
        use std::path::{Path, PathBuf};
        use std::sync::atomic::{AtomicU32, Ordering};

        /// Self-contained temp dir (the organizer crate's own `testutil` is
        /// `#[cfg(test)]`-gated inside the lib and isn't visible to an
        /// external integration-test binary).
        struct TrustPipelineTempDir(PathBuf);
        impl TrustPipelineTempDir {
            fn new() -> Self {
                static COUNTER: AtomicU32 = AtomicU32::new(0);
                let n = COUNTER.fetch_add(1, Ordering::Relaxed);
                let path = std::env::temp_dir().join(format!(
                    "ghost_trust_pipeline_test_{}_{}",
                    std::process::id(),
                    n
                ));
                fs::create_dir_all(&path).unwrap();
                TrustPipelineTempDir(path)
            }
            fn path(&self) -> &Path {
                &self.0
            }
            fn file(&self, rel: &str, bytes: &[u8]) -> PathBuf {
                let p = self.0.join(rel);
                fs::write(&p, bytes).unwrap();
                p
            }
        }
        impl Drop for TrustPipelineTempDir {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }

        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();

        let dir = TrustPipelineTempDir::new();
        let report_pdf = dir.file("report.pdf", b"report body");
        dir.file("photo.JPG", b"photo body");
        dir.file("song.mp3", b"song body");

        // Stage 1 (Intent) + boundary: the user approves a Zone and grants a
        // folder rule. New Zones default to Ask; delete is never granted.
        let zone = create_zone(&conn, "Downloads", None, DefaultDecision::Ask, false)
            .expect("create_zone");
        let rule = FolderRule {
            path: dir.path().to_path_buf(),
            can_read: true,
            can_create: true,
            can_rename: true,
            can_move: true,
            can_copy: true,
            can_delete: false,
            trust: TrustLevel::AskFirst,
        };
        add_folder_rule(&conn, &zone.id, &rule).expect("add_folder_rule");

        // Stage 2-3 (Plan + Policy): plan_zone loads the persisted rule,
        // scans, classifies, and policy-checks every action. Read-only.
        let plan = plan_zone(&conn, &zone.id).expect("plan_zone");
        assert!(
            !plan.actions.is_empty(),
            "plan should propose organizing the loose files"
        );
        assert!(
            report_pdf.exists(),
            "planning must not mutate the filesystem"
        );

        // Stage 4 (Approval) is implicit here: calling execute_plan below *is*
        // the approved-execution path in the real command
        // (`organizer_execute`) too — a plan is never applied without this
        // explicit step.
        let rules = vec![rule];

        // Stage 5-6 (Undo-journal-before-execute, then Execute): the executor
        // writes undo data before each mutation and independently re-checks
        // policy per action.
        let report = execute_plan(&plan, &rules);
        assert_eq!(
            report.failed, 0,
            "no action should fail against an approved rule"
        );
        assert!(
            report.applied > 0,
            "at least one action should have applied"
        );
        assert!(
            !report_pdf.exists(),
            "the executor should have actually moved the file on disk"
        );

        // Stage 7 (Audit): the run is persisted and sealed into the
        // tamper-evident hash chain.
        let execution_id = save_execution(&conn, &zone.id, &report).expect("save_execution");
        let verification = verify_chain(&conn).expect("verify_chain");
        assert!(verification.intact, "a freshly sealed run must verify");
        assert_eq!(verification.sealed_count, 1);

        // Stage 8 (Undo): replaying the stored journal must restore the file.
        let stored = get_execution(&conn, &execution_id)
            .expect("get_execution")
            .expect("execution was saved");
        let undo_report = revert(&stored.undo);
        assert_eq!(undo_report.failed, 0);
        assert!(undo_report.reverted > 0);
        assert!(
            report_pdf.exists(),
            "undo should restore the moved file to its original path"
        );
    }

    /// Reliability benchmark: Deterministic classification
    ///
    /// Verify same input → same output across 100 runs, against the real
    /// classifier. This is the foundation of auditability: a plan preview and
    /// the plan actually executed must agree, which only holds if
    /// classification is a pure function of the file's metadata.
    #[test]
    fn test_invoice_filing_determinism_benchmark() {
        use ghost_lib::organizer::classifier::{classify, Category};

        let file = scanned_file("invoice_acme_2026_june.pdf");

        // Run classification 100 times.
        let results: Vec<Category> = (0..100).map(|_| classify(&file).category).collect();

        // All results should be identical.
        for (i, category) in results.iter().enumerate() {
            assert_eq!(
                results[0], *category,
                "Classification is not deterministic (run 0 vs run {i})",
            );
        }

        assert_eq!(results[0], Category::Invoices);
    }

    /// Undo structure correctness
    ///
    /// Verify the real `UndoJournal` reverses recorded steps newest-first, so
    /// a move into a folder is undone before that folder's own removal is
    /// attempted (the folder must be empty by the time `RemoveFolder` runs).
    #[test]
    fn test_undo_operation_correctness() {
        use ghost_lib::audit::{UndoJournal, UndoOp};
        use std::path::PathBuf;

        let mut journal = UndoJournal::new();
        journal.record(UndoOp::RemoveFolder {
            path: PathBuf::from("/archive/2026"),
        });
        journal.record(UndoOp::Restore {
            from: PathBuf::from("/archive/2026/move_file_1"),
            to: PathBuf::from("/move_file_1"),
        });
        journal.record(UndoOp::Restore {
            from: PathBuf::from("/archive/2026/move_file_2"),
            to: PathBuf::from("/move_file_2"),
        });

        let reversed: Vec<&UndoOp> = journal.reversed().collect();

        assert!(
            matches!(reversed[0], UndoOp::Restore { from, .. } if from.ends_with("move_file_2")),
            "reversal should undo moves first (newest first): {:?}",
            reversed[0]
        );
        assert!(
            matches!(reversed[1], UndoOp::Restore { from, .. } if from.ends_with("move_file_1")),
            "reversal should undo moves first: {:?}",
            reversed[1]
        );
        assert!(
            matches!(reversed[2], UndoOp::RemoveFolder { .. }),
            "reversal should remove folders last, once empty: {:?}",
            reversed[2]
        );
    }

    /// Policy engine: Deny by default
    ///
    /// Test that the real policy engine denies operations outside the
    /// approved Zone, regardless of trust level.
    #[test]
    fn test_policy_deny_by_default() {
        use ghost_lib::policy::{evaluate, Capability, FolderRule, PolicyDecision, TrustLevel};
        use std::path::PathBuf;

        let rules = vec![FolderRule {
            path: PathBuf::from("/Users/alice/Downloads"),
            can_read: true,
            can_create: true,
            can_rename: true,
            can_move: true,
            can_copy: true,
            can_delete: false,
            trust: TrustLevel::AskFirst,
        }];

        let test_paths = [
            ("/Users/alice/Downloads/file.pdf", true),
            ("/Users/alice/Downloads/subfolder/file.pdf", true),
            ("/Users/alice/Documents/file.pdf", false),
            ("/System/Library/file.pdf", false),
        ];

        for (path, inside_zone) in test_paths {
            let cap = Capability::MoveFile {
                from: PathBuf::from(path),
                to: PathBuf::from(format!("{path}.moved")),
            };
            let decision = evaluate(&cap, &rules);
            let denied = matches!(decision, PolicyDecision::Deny { .. });
            assert_eq!(
                !denied, inside_zone,
                "policy check failed for {path}: {decision:?}"
            );
        }
    }

    /// Approval requirement: No execution without review
    ///
    /// The real policy engine expresses "approval required" structurally: a
    /// granted mutation under the default `AskFirst` trust comes back as
    /// `RequireConfirmation`, never a bare `Allow` — the executor cannot treat
    /// it as pre-approved. Only explicit `Automate` trust removes the prompt.
    #[test]
    fn test_approval_gate_required_before_execution() {
        use ghost_lib::policy::{evaluate, Capability, FolderRule, PolicyDecision, TrustLevel};
        use std::path::PathBuf;

        let cap = Capability::MoveFile {
            from: PathBuf::from("/Users/alice/Downloads/report.pdf"),
            to: PathBuf::from("/Users/alice/Downloads/Documents/report.pdf"),
        };

        let ask_first_rule = FolderRule {
            path: PathBuf::from("/Users/alice/Downloads"),
            can_read: true,
            can_create: true,
            can_rename: true,
            can_move: true,
            can_copy: true,
            can_delete: false,
            trust: TrustLevel::AskFirst,
        };
        let decision = evaluate(&cap, std::slice::from_ref(&ask_first_rule));
        assert!(
            matches!(decision, PolicyDecision::RequireConfirmation { .. }),
            "a granted move must still require confirmation under AskFirst trust, got {decision:?}"
        );

        let automate_rule = FolderRule {
            trust: TrustLevel::Automate,
            ..ask_first_rule
        };
        let automate_decision = evaluate(&cap, &[automate_rule]);
        assert!(
            automate_decision.is_allowed(),
            "Automate trust should be the only way to skip the prompt, got {automate_decision:?}"
        );
    }
}
