# Target resolution

How a replayed click finds its element, and how changes to that logic are
validated. Code: `src-tauri/src/core/replay_support.rs`; benchmark:
`src-tauri/tests/resolution_benchmark.rs`.

## Locator data captured per click

`ElementInfo` stores, per recorded click (all optional beyond role/name/app):

- role, accessible name, app — the semantic descriptor;
- `identifier` — stable automation id, strongest signal when present;
- `window_title`, `window_rel` — the containing window's title and the click
  position relative to its top-left corner;
- `fallback_coords` — absolute screen point, last resort.

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
4. **None** — callers decide: plain replay falls back to recorded
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
- **macOS**: not wired — finding a window by title needs process enumeration
  + per-app AXWindows traversal, or CGWindowList (which requires the Screen
  Recording permission and is therefore off the table under the privacy
  defaults). The strategy is inert there (`|_| None`); the spiral covers
  moderate moves. Wiring a permission-free lookup is tracked follow-up work.

## Changing this code

The benchmark suite is the gate: 13 canonical scenarios (moved element,
moved window, decoys, renames, title drift, old recordings) each pin the
exact click point and winning strategy — including the "must NOT resolve"
cases where blind-clicking would hit the wrong control. Any change to the
chain or the matcher must keep it green, and new behavior needs new
scenarios:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test resolution_benchmark
```
