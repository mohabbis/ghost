# Ghost — Technical State of the Project

> Snapshot for context (as of 2026-07-07, `master` + PR #104 merged-ready).
> Audience: engineers picking this up to push it to a YC-submission bar.
> Everything below is grounded in the actual code in this repo — file paths and
> symbols are real and clickable.

---

## 1. What Ghost is

Ghost is a **local-first desktop automation product** for macOS and Windows,
built as a **Tauri 2 app** (Rust backend, vanilla-JS frontend). One sentence:

> Ghost turns repeated computer work into safe, reusable, permission-bounded
> routines — nothing runs on a server, and nothing mutates your machine that you
> did not preview and approve.

The product's whole thesis is **trustworthy execution**, expressed as a pipeline
that every meaningful operation flows through:

```
Intent → Plan → Policy check → Approval → Execution → Audit → Undo
```

The current shipping wedge is **Ghost Organizer** (safe file/folder cleanup:
scan → propose → approve → move/rename → audit → undo). Recording/replay of
cross-app routines is the second layer and is partially built. AI/"Intelligence"
is deliberately last and is feature-gated off by default.

**Version:** `1.1.0` (`src-tauri/Cargo.toml`). ~20k lines of Rust across
`src-tauri/src` plus a no-build-step vanilla JS/HTML/CSS frontend in `src/`.

---

## 2. Architecture at a glance

```
src/                    Desktop app frontend — vanilla JS/HTML/CSS, NO bundler.
                        tauri.conf.json serves ../src directly; main.js holds the UI logic.
public/                 Marketing site (static, in-browser demos), auto-deployed to Vercel.
src-tauri/src/          Rust backend (the real product logic).
docs/                   Planning + technical docs (this file lives here).
.github/workflows/      CI (rust.yml), release (release.yml), site deploy, security.yml.
```

**Backend module map** (the parts that matter):

| Area | Path | Role |
|---|---|---|
| App wiring + IPC registry | `src-tauri/src/lib.rs` | `tauri::generate_handler!` lists **81 commands**; experimental ones `#[cfg(feature = "experimental")]` |
| Command bridge | `src-tauri/src/commands/` | Thin IPC layer: `core.rs`, `organizer.rs`, `auth.rs`, `compression.rs`, `diagnostics.rs`, `updates.rs`, `experimental.rs` |
| **Policy engine** | `src-tauri/src/policy/` | Pure, deny-by-default trust evaluation. No IO. |
| **Organizer** | `src-tauri/src/organizer/` | scanner → classifier → naming → conflict → planner → executor → undo |
| **Audit + undo** | `src-tauri/src/audit/` | Append-only audit log + undo journal (pure data) |
| **Storage** | `src-tauri/src/storage/` | SQLite: Zones, folder rules, executions (+ tamper-evident chain), migrations |
| Recording/replay engine | `src-tauri/src/engine.rs`, `core/replay_support.rs`, `core/execution.rs` | Capture, replay pacing, per-step trace, target resolution |
| Event compression | `src-tauri/src/core/compression/` | Deterministic raw-input → reviewable steps (no LLM) |
| OS backends | `src-tauri/src/platform/` | `macos.rs`, `windows.rs`, `headless.rs` (Linux CI) |
| Experimental (gated) | `src-tauri/src/core/{ai,llm,cloud,vision,knowledge}.rs` | Off unless `--features experimental` |

The design rule the codebase actually enforces: **the app shell is thin; product
logic lives in modules that are unit-testable without the UI.** The policy engine
and organizer planner do zero IO and are exercised by hundreds of tests.

---

## 3. The trust pipeline, in real code

This is the part worth understanding deeply — it's the moat and the story.

### 3a. Policy engine — deny by default (`src-tauri/src/policy/`)

`policy/decision.rs` defines the only three outcomes the rest of the system acts on:

```rust
pub enum PolicyDecision {
    Allow,                                        // deterministic core may run it
    Deny { reason: String },                      // refused outright
    RequireConfirmation { reason: String, risk: RiskLevel },  // needs explicit user OK
}
```

`policy/engine.rs` is **pure** (takes a `Capability` + active `FolderRule`s,
returns an `Evaluation`; no storage, no IO). Two properties matter:

- **Component-aware containment.** `path_within` uses `Path::starts_with`
  (whole-component compare), so `/a/bc` is correctly *not* inside `/a/b`. A naive
  string-prefix check would leak access to sibling folders — this is the kind of
  detail that makes the "bounded" claim real.
- **Trust levels drive prompting.** `decide_mutation` maps a granted mutation
  through the rule's `TrustLevel` (`policy/zone.rs`): `Automate` → `Allow`,
  `AskFirst` → `RequireConfirmation` (the default, and the pre-feature behavior),
  `Never` → `Deny`. Two-sided ops (move spans a source rule and a dest rule)
  resolve to the **stricter** side (`stricter_rule`).

Every decision carries `rule_path: Option<PathBuf>` so the UI and audit log can
name *which user-approved boundary* authorized or refused each action.

### 3b. Organizer planner — read-only preview (`organizer/planner.rs`)

`plan_zone` **mutates nothing**: it scans directory metadata, classifies files,
proposes safe targets/names, detects conflicts, expresses each change as a
`Capability`, and runs every one through `policy::evaluate`. A Zone with no
create+move rule yields a plan where every file is `Skipped { NoDestination }` —
deny-by-default, surfaced, not silent.

### 3c. Executor — the only code that touches the filesystem (`organizer/executor.rs`)

`execute_plan` deliberately re-does the caution rather than trusting the plan:

1. **Re-checks policy at execution time** (`policy::evaluate_with_attribution`) —
   anything now denied is skipped and audited, never applied.
2. **Verifies state** — source must still exist; target must *not*. `relocate`
   refuses to overwrite: `"target already exists, refusing to overwrite"`.
3. **Writes undo before mutating** — `undo.record(UndoOp::Restore { … })` runs
   *before* the `fs` call (trust invariant #8).
4. **Applies, verifies, records one audit event.** A failure on one action leaves
   every other file recoverable.

It only ever applies `CreateFolder` / `MoveFile` / `RenameFile`. **It never
deletes and never copies over an existing file.**

### 3d. Audit + undo (`src-tauri/src/audit/`)

- `audit_log.rs`: every action → one `AuditEvent { capability, outcome, rule_path,
  provenance, timestamp }`. `ActionOutcome` is `Applied | Skipped{reason} |
  Failed{error}`. `Provenance` records whether a change was `Automated` (an
  `automate` rule) or `UserApproved`.
- `undo_journal.rs`: pure data. `UndoOp` is `RemoveFolder { path }` (removes
  **only if empty** — never recursive delete) or `Restore { from, to }`. The
  runner (`organizer/undo.rs`) replays the journal in reverse.

### 3e. Server-side re-plan (`commands/organizer.rs`)

A deliberate trust choice worth calling out in any technical pitch:

> `organizer_execute` does **not** accept a plan from the frontend. It re-plans
> server-side from the Zone id, and the executor re-checks every action through
> `policy::evaluate`. A stale or tampered plan posted from JS can never reach the
> filesystem.

The preview the UI shows (`organizer_plan`) and the plan actually executed are
produced by the same deterministic backend against the same persisted rules.

### 3f. Tamper-evident audit chain (`storage/executions.rs`)

Each stored Organizer run is sealed with a SHA-256 hash over **its own row bytes
plus the previous run's hash** — an offline-verifiable chain (`execution_row_hash`,
`verify_chain`). Fields are length-delimited before hashing so `("a","bc")` and
`("ab","c")` can't collide. Runs written before the v5 migration carry an empty
seal and are reported as "unsealed, pre-upgrade" rather than as tampering.

---

## 4. Data + storage (`src-tauri/src/storage/`)

SQLite via `rusqlite`, with **forward-only versioned migrations**
(`migrations.rs`, `LATEST_VERSION = 5`):

- v1–v3: Zones, folder rules, execution history, `rename_dated` opt-in.
- v4: per-rule `trust_level` (`automate`/`ask_first`/`never`, CHECK-constrained;
  existing rules default to `ask_first`) + a local `organizer_milestones` table
  (time-to-first-value instrumentation).
- v5: `hash` + `prev_hash` columns for the tamper-evident chain above.

All local. No cloud dependency in the stock build.

---

## 5. Command surface (`src-tauri/src/lib.rs`)

**81 registered Tauri commands.** They split cleanly:

- **Stable core (always compiled):** recording/replay (`start_recording`,
  `replay_workflow`, `dry_run_workflow`, `get_replay_progress`,
  `get_replay_history`, pause/resume/cancel, playback speed), inspection,
  workflow storage, permissions (`check/request_accessibility`,
  `*_input_monitoring`, `restart_app`), `compress_workflow`, signed updater
  (`check_for_update` / `install_update`), local auth
  (`auth_setup/unlock/lock/status`), config/telemetry/diagnostics, the full
  `organizer_*` surface, and `is_experimental_enabled`.
- **Experimental (`#[cfg(feature = "experimental")]`):** AI analysis, workflow
  generation from prompt, cloud sync, analytics, visual checks. **A stock build
  neither registers nor exposes these**; the frontend hides the panel unless the
  always-present `is_experimental_enabled` reports the feature on.

Every command is classified by what it touches (files / OS input / screen /
network / auth / app state) in `docs/command-registry.md`.

---

## 6. Recording, replay, and target resolution

- **Event compression** (`core/compression/`): pure, deterministic, **no LLM, no
  network**. Turns a noisy `InputEvent` stream into `CompressedStep`s with
  confidence scores. Privacy is default: typed text redacted unless the caller
  opts out; secure fields (via `core::guard::is_sensitive_element`) are **never**
  retained even with retention on.
- **Replay inspectability** (`core/replay_support.rs`, `core/execution.rs`): each
  replay advances a shared `ReplayProgress` so `get_replay_progress` reports live
  per-step status and the failing step index. `dry_run_workflow` returns a
  per-step preview (typed text never included).
- **Target resolution** (`core/replay_support.rs`, gated by
  `tests/resolution_benchmark.rs`): every replayed click records how its target
  resolved (`ResolutionKind`: recorded point / window-relative / spiral
  re-resolution / coordinate fallback / no descriptor). Window-relative
  re-resolution is live on Windows (`FindWindowA`) and macOS (libproc + AXWindows
  under the existing Accessibility permission — never Screen Recording).

Replay invariants that are enforced and tested: press/release pairing, timestamp
pacing, interruptible pause/cancel inside the loop, semantic-before-coordinate
resolution, double-click preservation.

---

## 7. What works today ✅

- **Ghost Organizer end-to-end trust pipeline** — plan (read-only) → policy →
  approval → execute (re-checked, undo-before-mutate, no overwrite/delete) →
  audit → undo. Fully wired to `organizer_*` commands and the app UI.
- **Deny-by-default policy engine** with per-rule trust levels and rule
  attribution, pure and heavily unit-tested.
- **Tamper-evident, offline-verifiable audit chain** over executions.
- **Deterministic event compression + inspectable replay** with per-click
  resolution tracing and a cross-run reliability summary.
- **Signed auto-update** surface (verifies update signature against an embedded
  public key; installs only after user approval).
- **Local at-rest protection** — `argon2` + `aes-gcm` (see `auth.rs`).
- **CI green on all three OSes** — Check / Test / Clippy / Rustfmt + a
  `cargo tauri build --no-bundle` smoke test on macOS & Windows (`rust.yml`).
  363 backend tests + integration suites (`ipc_contract`, `resolution_benchmark`,
  `canonical_workflows`, `frontend_dom_contract`) pass. An IPC-contract test
  asserts the frontend only invokes registered commands with matching params;
  a DOM-contract test asserts every `getElementById` the JS looks up is authored
  in the markup.

## 8. What does NOT work / is incomplete ⚠️

- **Ghost Guard routing of compressed steps** — event compression produces the
  reviewable timeline, and the guard now audits that semantic timeline directly
  (`guard::audit_compressed`, exposed as `ghost_guard_audit_compressed`), so risk
  findings line up with the review-timeline steps the user sees. Routing those
  same steps through the *policy* engine (`policy/`) for recorded routines — as
  opposed to organizer file ops — is still follow-up work.
- **Routines replay is not yet a first-class, guarded, approvable product** the
  way Organizer is. The capture/replay/trace plumbing exists; the
  review→guard→policy→approve→execute→vault→undo loop for arbitrary cross-app
  routines is not fully closed.
- **AI / Intelligence layer is experimental and off by default** — `core/ai.rs`,
  `llm.rs`, `cloud.rs`, `vision.rs`, `knowledge.rs` compile only under
  `--features experimental`, and **CI does not run the experimental leg**. Treat
  these as prototypes, not shipped features.
- **No frontend build/test harness** — `src/` is served raw. UI logic in
  `main.js` is validated by static contract tests (the IPC-contract test for
  JS→Rust calls and the DOM-contract test for JS→element wiring), not by
  DOM/interaction (behavior) tests.
- **Organizer destination model is MVP-simple** — first folder rule granting
  create+move is the destination root; files sort into
  `<root>/<Category>/`. Richer routing/rules are not built.
- **Signing/notarization not proven in this repo** — checklists exist
  (`docs/macos-signing-checklist.md`, `docs/windows-signing-checklist.md`) but
  releases are developer-signed until the signing keys/secrets are configured.

---

## 9. Secrets & configuration (where things plug in)

Assume the intent is "we have the accounts; wire the secrets." The touch points:

- **Updater signing key** — `tauri-plugin-updater` verifies update artifacts
  against an **embedded public key**; the matching private key signs artifacts in
  `release.yml`. Update endpoint + key must be configured for real auto-update.
- **Code signing** — macOS: Developer ID cert + notarization (Apple ID / Team ID
  / app-specific password or API key); Windows: Authenticode cert (Azure Trusted
  Signing is the documented path — `docs/azure-signing-cost.md`). Checklists in
  `docs/`.
- **Site deploy** — `public/` auto-deploys to Vercel (see
  `.github/workflows/deploy-website.yml`); those tokens live in the hosting
  provider, not the repo.
- **Experimental AI/cloud** — any model/cloud credentials only matter under
  `--features experimental`; the stock product needs none.
- **Local user data** — SQLite DB + workflows live under the OS app-data dir
  (`dirs`), optionally encrypted at rest via the local password (`auth.rs`).

No secrets are required to build, test, or run the core product locally.

---

## 10. What "extensive engineering to YC bar" concretely means

Prioritized, and mapped to where the work lands:

1. **Close the Routines loop** to Organizer's standard — route compressed steps
   through guard + policy + approval + undo (`core/compression/` →
   `core/guard.rs` → `policy/` → a vault/undo path). This is the single biggest
   gap between "impressive demo" and "second product layer shipped."
2. **Harden cross-app replay reliability** — expand the resolution benchmark
   (`tests/resolution_benchmark.rs`) with real-app scenarios; tighten
   window-relative + semantic resolution before coordinate fallback.
3. **Frontend test + build story** — introduce a lightweight harness so
   `src/main.js` behavior is testable; keep the no-heavy-bundler ethos.
4. **Ship signing/notarization for real** — execute the macOS/Windows checklists,
   wire the release secrets, verify the signed auto-update round-trips.
5. **Turn experimental Intelligence into a gated-but-real suggestion layer** —
   suggestion-only planning/classification that still routes through the
   deterministic approve→execute path (never lets model output act directly).
6. **Instrument time-to-first-value** — the `organizer_milestones` table (v4)
   exists; surface activation/retention metrics for the pitch.

The strong part of the story is already built and provable: a **deterministic,
deny-by-default, audited, reversible execution core** that a skeptical buyer can
read end to end. The remaining work is breadth (Routines, reliability, polish,
signing), not a rewrite of the trust foundation.

---

## 11. How to build & verify

```bash
# System deps (Linux): GTK/webkit + libxdo (see AGENTS.md). macOS/Windows: none extra.
cargo fmt   --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
cargo test  --manifest-path src-tauri/Cargo.toml          # 363 + integration suites
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo tauri build --no-bundle                             # compile smoke test
# Makefile shortcuts: `make ci` (fmt+clippy+test), `make check`, `make build`, `make dev`.
# Experimental leg is NOT in CI — run the above with `--features experimental` when touching it.
```

Frontend needs no compile — `tauri.conf.json` serves `src/` directly.
