# Full Repository Audit — 2026-07-13

Audit date: 2026-07-13. Reviewed `master` @ `eac84ca` (includes #230, #231) plus open PR #232 (`cursor/trust-pipeline-followups-ea2c`). Supersedes portions of `docs/ghost-product-repository-audit-2026-07-11.md` where status has changed.

## 1. Executive summary

| Area | Rating | Notes |
|---|---|---|
| **Overall readiness** | Technical preview | Organizer trust pipeline is production-grade for its scope; Routines/replay and release signing gaps remain. |
| **Ghost Organizer** | Strong | Policy-gated plan → execute → audit → undo; WAL crash recovery; tamper-evident chain. |
| **Ghost Routines (replay)** | Partial | Policy approve + compressed review UI shipped (#215, #231); WAL undo in PR #232; not yet Organizer-grade per-step policy or target-window validation. |
| **Release integrity** | Improved | v1.2.9 aligned across repo/site/README; updater pubkey embedded; macOS notarized; **Windows still unsigned**. |
| **Security CI** | Gap (being fixed) | `security.yml` skipped `master`; `cargo audit`/`deny` lacked manifest path; CodeQL scanned cpp/python only. |
| **Marketing accuracy** | Improved, not done | Site/README now say technical preview in places; undo/Guard Desk/upload claims still need ongoing qualification. |

**Recommended posture:** Keep Ghost labeled **technical preview**. Lead with Organizer. Qualify replay undo (typed text only), Guard Desk (prototype), and optional integrations (sign-in, Power BI, experimental AI).

## 2. Strengths (verified in code)

### Trust pipeline — Ghost Organizer

```text
Select folder → Scan → Plan → Policy → Approve → Execute → Audit → Undo
```

- **Server-side re-plan** on execute (`organizer_execute` never trusts a frontend plan).
- **Per-action policy re-check**; refuses silent overwrite/delete.
- **Write-ahead durability** (`begin_execution` / `update_execution_progress` / `finish_execution`).
- **Crash recovery UI** (`organizer_check_unfinished_run`, undo or dismiss).
- **Tamper-evident audit chain** (`organizer_verify_audit_chain`, sealed rows).
- **PII masking on export only** (`audit/pii.rs` — stored chain untouched).
- **Adversarial tests** for TOCTOU and symlink exclusion (#230).

### Recording / replay inspectability

- Deterministic **event compression** (`compress_workflow`, review timeline UI).
- **Ghost Guard** deterministic audits (raw + compressed).
- **Routine policy gating** (`routine_policy_plan`, `approve_routine_replay`, `replay_workflow` consumes one-shot approval).
- **Replay progress**, dry-run preview, resolution tracing, template-match fallback.
- **IPC + DOM contract tests** (`ipc_contract.rs`, `frontend_dom_contract.rs`).

### Release / version alignment (v1.2.9)

- `src-tauri/Cargo.toml`, `tauri.conf.json`, `README.md`, `public/index.html` all report **1.2.9**.
- Updater public key embedded (GHO-HIGH-003 closed).
- Google OAuth client ID bundled for sign-in availability (#230).

### Engineering hygiene

- Pinned Rust toolchain; `make ci` (fmt + clippy + test).
- Multi-OS CI (`rust.yml`): check, test, clippy, fmt, tauri compile smoke (macOS/Windows), experimental leg (ubuntu).
- `test_support::EnvVarGuard` for parallel env test stability (#230).

## 3. Critical / high open issues

| ID | Severity | Issue | Status / fix |
|---|---|---|---|
| REL-001 | High | Windows Authenticode signing missing — SmartScreen warnings | Deferred per product direction; see `RELEASING.md` |
| RPL-001 | High | Replay not Organizer-grade: no per-step execute policy, no target-window validation, undo limited to typed backspaces | PR #232 adds WAL + typed undo; Zones + full vault still open |
| SEC-001 | High | `security.yml` did not run on `master` pushes | Fixed in PR #232 branch |
| SEC-002 | High | `cargo audit` / `cargo deny` ran from repo root without `--manifest-path src-tauri/Cargo.toml` | Fixed in PR #232 branch |
| SEC-003 | Medium | CodeQL matrix was `cpp`/`python` only — no Rust analysis | Fixed: matrix → `rust` in PR #232 branch |
| MKT-001 | Medium | Marketing overclaims: absolute "nothing uploaded", "one-click undo", "Shipping now" | Partially fixed on site; monitor on each release |
| MKT-002 | Medium | Guard Desk / POS Bridge presented as shipping product vs prototype | Disclaimer added in app + site roadmap |
| EXP-001 | Medium | `replay_with_visual_check` (experimental) bypasses routine policy | Keep gated; document in QA checklist |
| DOC-001 | Low | `PROJECT_STATE.md` still described SQLite storage | Fixed → redb in PR #232 branch |
| DOC-002 | Low | `HANDOFF.md` stale version/test counts | Fixed → v1.2.9 / ~694 tests |
| DOC-003 | Low | `command-registry.md` missing rows for several stable commands | Fixed in PR #232 branch |

(See the "Status addendum — 2026-07-16" at the end of this document for the current status of these findings.)

## 4. Doc drift register (resolved / remaining)

| Document | Was wrong | Corrected to |
|---|---|---|
| `docs/PROJECT_STATE.md` §4 | SQLite + migrations | redb (`ghost.redb`) + one-time sqlite import |
| `HANDOFF.md` | v1.2.8, 564 tests | v1.2.9, ~694 tests |
| `docs/command-registry.md` | Missing `compress_workflow`, `is_experimental_enabled`, policy pack import/export, `organizer_verify_signed_report` | Rows added |
| `docs/implementation-tracker.md` | GHO-MED-005/006 still open on branch | Merged #230 @ `49b2966` |
| `docs/ghost-product-repository-audit-2026-07-11.md` | v1.2.4/1.2.6 mismatch, placeholder pubkey | Historical; see this doc for current state |

**Remaining doc follow-ups (not blocking):**

- Add replay WAL commands to `docs/organizer-commands.md` or a dedicated replay recovery doc when #232 merges.
- Refresh `docs/post-audit-implementation-handoff.md` checklist against closed GHO items.

## 5. CI / validation gaps (addressed in PR #232)

| Gap | Fix |
|---|---|
| No version consistency gate | New `version-consistency` job in `rust.yml` |
| `compression-review.test.mjs` not in CI | New `frontend-contract` job |
| Experimental test flake (`replay_undo` Enigo on headless CI) | Skip Enigo when journal has no backspace ops |
| Security workflow branch / manifest gaps | `security.yml` updates |

## 6. Product claim reconciliation (2026-07-13)

| Claim | Verdict | Evidence |
|---|---|---|
| v1.2.9 release | **Verified** | Cargo, tauri.conf, README, site, GitHub release |
| macOS notarized | **Disclosed** | README; not re-verified on artifacts in this audit |
| Windows signed | **Not met** | README warns unsigned |
| Signed auto-updater | **Met for v1.2.9+** | Pubkey in `tauri.conf.json`; user-gated install |
| Files never leave machine | **Qualified** | True for Organizer default path; opt-in sign-in / experimental integrations can use network |
| No account required | **Verified** | Sign-in optional in Settings |
| Organizer audit + undo | **Verified** | Executor + undo journal + integration tests |
| One-click undo (all actions) | **Misleading** | Organizer undo is strong; replay undo is typed-text only (PR #232) |
| Replay policy-gated | **Verified** | `approve_routine_replay` + `replay_workflow` (#215, #231) |
| Guard Desk compliance | **Prototype** | Local OCR + deterministic rules; POS Bridge is mock/simulation |
| Experimental AI gated | **Verified** | `experimental` feature + `is_experimental_enabled` + IPC contract tests |

## 7. Prioritized recommendations

### P0 (done or in PR #232)

1. Fix `security.yml` for `master`, manifest path, Rust CodeQL.
2. Fix headless CI `replay_undo` tests (lazy Enigo).
3. Align stale docs (`PROJECT_STATE`, `HANDOFF`, command registry, implementation tracker).
4. Qualify marketing copy (technical preview, undo scope, Guard Desk).
5. Add version gate + frontend contract test to CI.

### P1 (next engineering)

1. **Windows Authenticode** — Azure Trusted Signing (`RELEASING.md`).
2. **Replay hardening** — app/window Zones, per-step execute policy, route Guard findings into review timeline.
3. **Gate `replay_with_visual_check`** behind the same approval path or keep strictly experimental-only.
4. **Microsoft OAuth client ID** — Entra app registration (Google bundled in #230).

### P2 (quality / pitch)

1. Expand `resolution_benchmark.rs` with real-app scenarios.
2. Surface `organizer_time_to_value` in onboarding metrics.
3. Light frontend harness beyond `compression-review.test.mjs` for `main.js` policy/replay flows.

## 8. Validation performed for this audit

```bash
# On PR #232 branch (post-fixes):
cargo test --manifest-path src-tauri/Cargo.toml core::replay_undo::tests
cargo test --manifest-path src-tauri/Cargo.toml --features experimental
make ci
node --test src/compression-review.test.mjs
```

Report SHA and CI status in the PR that lands these fixes.

## 9. References

- `docs/ghost-product-repository-audit-2026-07-11.md` — initial external audit
- `docs/post-audit-implementation-handoff.md` — phased hardening checklist
- `docs/manual-qa-checklist.md` — desktop QA paths
- `AGENTS.md` / `CLAUDE.md` — agent contract and architecture map

## Status addendum — 2026-07-16

The sections above are a point-in-time record of `master` @ `eac84ca` and are left
unedited. Verified against the current tree (Ghost 2.0, v2.0.4), the following
findings have changed status:

| ID | 2026-07-13 status | Current status (verified in code) |
|---|---|---|
| RPL-001 | Open (High) | **Substantially resolved.** Replay is policy-bound end to end: `routine_policy_plan` → `approve_routine_replay` → one-shot, TTL-bound approval token consumed by `replay_workflow` (`commands/core.rs`) and `execute_routine_action_plan` (`commands/runtime_cmds.rs`); plans are re-derived server-side and `ensure_replayable` refuses a Deny. Replay WAL + undo merged in #232 (`ac7c890`; `storage/replay_runs.rs`, `replay_check_unfinished_run`/`replay_undo`). Ghost Guard findings are routed into the review timeline (`src/compression-review.js` invokes `ghost_guard_audit_compressed` and renders per-step findings, with a CI contract test). Remaining: bare-`Allow` app/window Zones and the routine vault (tracked follow-up). |
| SEC-001 | Fixed in PR #232 branch | **Merged** (#232, `ac7c890`): `security.yml` runs on `master`. |
| SEC-002 | Fixed in PR #232 branch | **Merged** (#232, `ac7c890`): `cargo audit`/`deny` use `--manifest-path src-tauri/Cargo.toml`. |
| SEC-003 | Fixed in PR #232 branch | **Merged** (#232, `ac7c890`): CodeQL matrix analyzes `rust`. |
| REL-001 | Deferred | Unchanged — Windows Authenticode / Azure Trusted Signing still unconfigured. |
| Updater signing | Improved | A real minisign pubkey ships in `tauri.conf.json` (`plugins.updater.pubkey`); `updater_configured` treats only a scrubbed placeholder key as inert. |
| Supply chain | Open | GitHub Actions are now pinned to full commit SHAs across all workflows, with Dependabot keeping the pins fresh (this change). |
