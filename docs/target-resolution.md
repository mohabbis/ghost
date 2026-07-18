# Target resolution

How a replayed click finds its element, and how changes to that logic are
validated. Code: `src-tauri/src/core/replay_support.rs`; benchmark:
`src-tauri/tests/resolution_benchmark.rs`.

This documents the **current** Tauri replay resolution chain. The intended macOS
subsystem direction (AX quality scoring, ScreenCaptureKit + Vision fallback,
shared semantic locators) is in
[`macos-automation-architecture.md`](macos-automation-architecture.md).

## Locator data captured per click

`ElementInfo` stores, per recorded click (all optional beyond role/name/app):

- role, accessible name, app — the semantic descriptor;
- `identifier` — stable automation id, strongest signal when present;
- `window_title`, `window_rel` — the containing window's title and the click
  position relative to its top-left corner;
- `fallback_coords` — absolute screen point, last resort;
- `template_png` — a small screenshot crop taken around the click point, for
  the pixel-level template-match strategy below. Only populated when
  `PerformanceSettings::capture_element_templates` is enabled (off by
  default — it adds a screenshot capture's latency to every recorded click).

macOS reads the AXWindow ancestor (title + AXPosition); Windows reads the
GA_ROOT window (text + GetWindowRect). Recordings made before these fields
existed deserialize with them absent and behave exactly as before.

## Strategy chain (strongest signal first)

`try_resolve_click_point_traced` tries, in order:

1. **Recorded point** — the descriptor still matches at the recorded
   coordinates.
2. **Window-relative** — find the recorded window by title (platform
   closure), add the recorded in-window offset, and *verify the descriptor at
   that point*. Survives arbitrarily large window moves in one lookup. Never
   blind-clicks: if the window moved but its contents rearranged, this step
   fails through.
3. **Spiral** — scan outward around the recorded point (4 radii × 8
   directions, up to 260 px).
4. **Template match** — when `ElementInfo.template_png` is set, crop the
   current screenshot to a region around the recorded point (bounded by the
   spiral's own max radius, 260 px) and search it for the captured template
   (`core/template_match.rs`: normalized cross-correlation over a 4x
   downsampled grayscale image, matched by nearest-neighbor sampling to
   avoid blurring template edges into surrounding content). Accepts a match
   only above `template_match::DEFAULT_MIN_SCORE` (0.80). Pixel-level, so it
   can succeed where every strategy above it needs an accessibility-tree
   descriptor that either changed or was never recorded.
   Deliberately pure Rust over the `image` crate rather than an
   `opencv-rust` binding — OpenCV needs a prebuilt system library
   (pkg-config/vcpkg/brew) this repo's 3-OS CI matrix doesn't install.
5. **None** — callers decide: plain replay falls back to recorded
   coordinates; guarded replay retries with backoff, then errors unless
   `continue_on_error`.

Every click press records which strategy won (`ResolutionKind`) into the
run's `step_trace`, persisted on the `ExecutionRecord` and shown in the
replay-history UI.

## Descriptor matching rules

`descriptor_matches`, in order:

- equal non-empty `identifier` on both sides decides (accept/reject);
- role must match (case-insensitive);
- named target: name must match;
- nameless target with usable app: app must match;
- nameless target, unknown app: if **both** sides carry a window title,
  titles must match (discriminates same-role elements in different windows);
  if either side lacks one (old recordings), role alone matches — identical
  to pre-title behavior.

Window titles never *reject* a named match: titles legitimately drift
(document names), so they only discriminate where nothing stronger exists.

## Platform window lookup

- **Windows**: `FindWindowA(title)` + `GetWindowRect` (`platform/windows.rs`).
- **macOS**: permission-free, under the Accessibility permission replay
  already requires (`platform/macos.rs::find_window_origin`): libproc
  enumerates pids (no extra permission), the process name is matched against
  the recorded element's `app` (including the 32-byte `proc_name`
  truncation), and that app's `AXWindows` are walked for an `AXTitle` match
  whose `AXPosition` supplies the origin. Deliberately **not** CGWindowList,
  which would require the Screen Recording permission. Best-effort at every
  step — any failure falls through to the spiral / recorded coordinates.

The `window_origin` closure receives the full recorded `ElementInfo`
(Windows uses only `window_title`; macOS also needs `app`). A third closure,
`screenshot`, supplies the current screen on demand for the template-match
strategy; it's called at most once per resolution (only when strategies 1-3
have failed and a template is present), so a platform that can't cheaply
screenshot can pass `|| None` to skip strategy 4 entirely.

## Changing this code

The benchmark suite is the gate: 16 canonical scenarios (moved element,
moved window, decoys, renames, title drift, old recordings, template match
found/not-found) each pin the exact click point and winning strategy —
including the "must NOT resolve" cases where blind-clicking would hit the
wrong control. (Template-match scenarios compare their click point within a
small pixel tolerance rather than exactly — see
`TEMPLATE_MATCH_TOLERANCE_PX` in the benchmark file — since that strategy is
pixel-approximate by nature, unlike the exact accessibility-tree matches.)
Any change to the chain or the matcher must keep it green, and new behavior
needs new scenarios:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test resolution_benchmark
```
