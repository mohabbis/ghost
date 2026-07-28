# Filing Profiles

Audience-aware, deterministic filing previews. This layer answers "where should
these files go?" for a given audience, **without touching the filesystem**. It is
the "Propose plan" stage of the trust pipeline; the actual move/rename is still
done only by the Organizer's audited, undoable executor.

See `docs/audiences.md` for who each profile serves.

## Design rules

- **Rules first, AI never** (for this layer): classification is a pure function
  of the file *name* and extension. No network, no model, no IO, no file-content
  reads.
- **Name-only, disk-free:** the preview reasons over a list of names the caller
  passes in, so it is safe to run on a paste-in list before any folder access is
  granted.
- **Deterministic:** the same input always yields the same output; every module
  has a `*_is_deterministic` test.
- **Conservative:** period parsing requires real delimiters, so a stray 4-digit
  number inside a word is not read as a year. Unrecognized files are surfaced for
  review, never silently guessed.

## Modules (`src-tauri/src/filing/`)

| Module | Role |
|---|---|
| `period.rs` | Extract a `Period` (`Annual`/`Quarter`/`Month`/`Day`) from a name. Handles ISO/US dates, `Q2 2026`, `June 2026`, `2026-06`, `FY2026`, bare years. |
| `finance.rs` | Classify a file as a financial report type (`ReportKind`) with confidence + reason; flags spreadsheets. |
| `academic.rs` | Classify coursework (`CourseworkKind`), extract course code (`CS101`) and academic `Term` (`Fall 2026`). |
| `preview.rs` | `Audience` enum + `preview_filing(audience, root, names)` → `FilingPreview` (per-file proposed directory, counts, review flags). |
| `savings.rs` | `estimate_savings(inputs)` → annual hours (and, with an hourly rate, cost) saved; echoes back every assumption used. |

## Proposed folder shapes

- **Finance:** `<root>/<Report Type>/<Year>/<Period>` — e.g.
  `Financial Reports/Income Statements/2026/Q2`. Undated reports go under
  `…/<Report Type>/Undated` and are flagged for review.
- **Student:** `<root>/<Course?>/<Type>/<Term-or-Period>` — e.g.
  `Coursework/CS101/Assignments/2026 Fall`.
- **Unrecognized:** `<root>/Needs Review` (never guessed, never mutated).

## Commands (`commands/filing.rs`) — risk class: safe-read

- `preview_file_filing(audience, root?, file_names)` → `FilingPreview`.
  Read-only planning over names; no filesystem/network/OS-input/secret access.
- `estimate_filing_savings(inputs)` → `SavingsEstimate`. Pure arithmetic; the
  estimate lists every default it applied so the figure is auditable.

## UI surface

Both commands are wired into the app's **Plan Filing** view (`data-view="filing"`
in `src/index.html`, `filingInit` in `src/main.js`). The user picks a profile,
pastes a list of file names (nothing is read from disk), and previews the
proposed folders with per-file confidence/review badges; a second panel runs the
savings estimator over volume/cadence/time inputs and shows every assumption.
The view is read-only by design and points users to the Organizer to actually
apply changes with approval, audit, and undo.

## Savings model

```text
files_per_year          = files_per_period * periods_per_year
manual_hours            = files_per_year * minutes_per_file_manual / 60
                          + files_per_year * error_rate * rework_minutes / 60
assisted_hours          = files_per_year * minutes_per_file_assisted / 60
hours_saved             = max(0, manual_hours - assisted_hours)
cost_saved (if rate set)= hours_saved * hourly_rate
```

Defaults: assisted handling `0.5 min/file` (review + approve), rework
`10 min/error`, error rate `0` unless supplied. Assisted rework is assumed `0`
because a reviewed, structured filing step is what avoids hand-keying mistakes.
Inputs are clamped defensively (no negatives; error rate `0..=1`; assisted time
never exceeds manual time).

## Extending

Adding an audience is additive: a new profile module (a `classify_*` function),
a new `Audience` variant with a `default_root`, a `file_<audience>` arm in
`preview.rs`, and tests. The trust pipeline is untouched.
