//! Resolution benchmark: canonical target-resolution scenarios that gate
//! matching-behavior changes.
//!
//! Each scenario models a recorded click (target descriptor + recorded
//! point) and a "live UI" (elements at exact points, window origins by
//! title), then asserts exactly where the resolver clicks and which strategy
//! won. Changing `descriptor_matches` or the strategy chain in
//! `core/replay_support.rs` must keep this suite green — a regression here
//! means a real workflow that used to replay correctly now mis-clicks or
//! falls back to blind coordinates.
//!
//! Run just this suite:
//! `cargo test --manifest-path src-tauri/Cargo.toml --test resolution_benchmark`

use ghost_lib::core::events::ElementInfo;
use ghost_lib::core::replay_support::{ResolutionKind, try_resolve_click_point_traced};
use image::{DynamicImage, Rgb, RgbImage};
use std::collections::HashMap;

/// One benchmark case: the world as it looks at replay time, and the single
/// acceptable outcome.
struct Scenario {
    name: &'static str,
    target: ElementInfo,
    recorded: (i32, i32),
    /// Elements present in the live UI, at exact points.
    world: Vec<((i32, i32), ElementInfo)>,
    /// Current window origins, by title.
    windows: HashMap<&'static str, (i32, i32)>,
    /// The "screenshot" template matching searches, when the scenario
    /// exercises that strategy. `None` for every scenario that doesn't
    /// populate `target.template_png` — the resolver never calls this
    /// closure unless a template is present, so it's irrelevant otherwise.
    screenshot: Option<DynamicImage>,
    /// Expected resolution — `None` means "must NOT resolve" (caller falls
    /// back or errors; blind-clicking a wrong element here is the failure
    /// the suite exists to catch).
    expect: Option<((i32, i32), ResolutionKind)>,
}

fn element(role: &str, name: &str, app: &str) -> ElementInfo {
    ElementInfo {
        role: role.into(),
        name: name.into(),
        app: app.into(),
        ..Default::default()
    }
}

fn with_window(mut el: ElementInfo, title: &str, rel: Option<(i32, i32)>) -> ElementInfo {
    el.window_title = Some(title.into());
    el.window_rel = rel;
    el
}

fn with_identifier(mut el: ElementInfo, id: &str) -> ElementInfo {
    el.identifier = Some(id.into());
    el
}

/// Template matching is pixel-approximate (downsampled search), not exact
/// like the accessibility-tree strategies above it — a few pixels of slop
/// is expected and harmless (real UI elements are tens of pixels wide).
/// Coordinates for a `TemplateMatched` outcome are compared within this
/// tolerance rather than exactly; every other strategy still requires an
/// exact match, since those genuinely are exact.
const TEMPLATE_MATCH_TOLERANCE_PX: i32 = 8;

fn run(scenario: &Scenario) -> Result<(), String> {
    let world = &scenario.world;
    let windows = &scenario.windows;
    let screenshot = &scenario.screenshot;
    let outcome = try_resolve_click_point_traced(
        &scenario.target,
        scenario.recorded.0,
        scenario.recorded.1,
        |x, y| {
            world
                .iter()
                .find(|(p, _)| *p == (x, y))
                .map(|(_, el)| el.clone())
        },
        |el| {
            el.window_title
                .as_deref()
                .and_then(|title| windows.get(title).copied())
        },
        || screenshot.clone(),
    );
    let matches = match (&outcome, &scenario.expect) {
        (
            Some((point, ResolutionKind::TemplateMatched)),
            Some((expected, ResolutionKind::TemplateMatched)),
        ) => {
            (point.0 - expected.0).abs() <= TEMPLATE_MATCH_TOLERANCE_PX
                && (point.1 - expected.1).abs() <= TEMPLATE_MATCH_TOLERANCE_PX
        }
        _ => outcome == scenario.expect,
    };
    if matches {
        Ok(())
    } else {
        Err(format!(
            "{}: expected {:?}, got {:?}",
            scenario.name, scenario.expect, outcome
        ))
    }
}

/// A background that varies in 4px blocks (matching
/// `core::template_match`'s downsample factor) so a pattern embedded into it
/// survives downsampling instead of being blurred away — see the same
/// reasoning in `core/template_match.rs`'s own unit tests.
fn block_canvas(w: u32, h: u32, seed: u32) -> RgbImage {
    RgbImage::from_fn(w, h, |x, y| {
        let (bx, by) = (x / 4, y / 4);
        let v = ((bx.wrapping_mul(37) + by.wrapping_mul(91) + seed) % 256) as u8;
        Rgb([v, v, v])
    })
}

/// A block-checkerboard, distinct from anything `block_canvas` can produce,
/// so it only matches where it's deliberately embedded.
fn block_template(w: u32, h: u32) -> RgbImage {
    RgbImage::from_fn(w, h, |x, y| {
        let (bx, by) = (x / 4, y / 4);
        if (bx + by) % 2 == 0 {
            Rgb([250, 10, 10])
        } else {
            Rgb([10, 10, 250])
        }
    })
}

fn embed(canvas: &mut RgbImage, template: &RgbImage, at_x: u32, at_y: u32) {
    for y in 0..template.height() {
        for x in 0..template.width() {
            canvas.put_pixel(at_x + x, at_y + y, *template.get_pixel(x, y));
        }
    }
}

fn png_bytes(img: &DynamicImage) -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .expect("encoding a synthetic test image to PNG must not fail");
    buf.into_inner()
}

fn scenarios() -> Vec<Scenario> {
    vec![
        Scenario {
            name: "unmoved element resolves at recorded point",
            target: element("AXButton", "Save", "Notes"),
            recorded: (100, 100),
            world: vec![((100, 100), element("AXButton", "Save", "Notes"))],
            windows: HashMap::new(),
            screenshot: None,
            expect: Some(((100, 100), ResolutionKind::RecordedPoint)),
        },
        Scenario {
            name: "element moved within spiral range is re-resolved",
            target: element("AXButton", "Save", "Notes"),
            recorded: (100, 100),
            world: vec![((170, 100), element("AXButton", "Save", "Notes"))],
            windows: HashMap::new(),
            screenshot: None,
            expect: Some(((170, 100), ResolutionKind::SpiralReresolved)),
        },
        Scenario {
            name: "element gone entirely must not resolve",
            target: element("AXButton", "Save", "Notes"),
            recorded: (100, 100),
            world: vec![],
            windows: HashMap::new(),
            screenshot: None,
            expect: None,
        },
        Scenario {
            name: "same-role decoy with different name is skipped for the real target",
            target: element("AXButton", "Save", "Notes"),
            recorded: (100, 100),
            world: vec![
                // Decoy sits exactly at the recorded point.
                ((100, 100), element("AXButton", "Cancel", "Notes")),
                ((100, 170), element("AXButton", "Save", "Notes")),
            ],
            windows: HashMap::new(),
            screenshot: None,
            expect: Some(((100, 170), ResolutionKind::SpiralReresolved)),
        },
        Scenario {
            name: "stable identifier matches through a rename",
            target: with_identifier(element("AXButton", "Save", "Notes"), "save-btn"),
            recorded: (100, 100),
            world: vec![(
                (100, 100),
                with_identifier(element("AXButton", "Save (2 left)", "Notes"), "save-btn"),
            )],
            windows: HashMap::new(),
            screenshot: None,
            expect: Some(((100, 100), ResolutionKind::RecordedPoint)),
        },
        Scenario {
            name: "window moved far beyond spiral range resolves window-relatively",
            target: with_window(
                element("AXButton", "Save", "Notes"),
                "Report",
                Some((40, 30)),
            ),
            recorded: (140, 130),
            world: vec![(
                (940, 530),
                with_window(element("AXButton", "Save", "Notes"), "Report", None),
            )],
            windows: HashMap::from([("Report", (900, 500))]),
            screenshot: None,
            expect: Some(((940, 530), ResolutionKind::WindowRelative)),
        },
        Scenario {
            name: "window found but contents rearranged must not blind-click",
            target: with_window(
                element("AXButton", "Save", "Notes"),
                "Report",
                Some((40, 30)),
            ),
            recorded: (140, 130),
            world: vec![(
                // A different control now sits at the recorded offset.
                (940, 530),
                element("AXTextField", "Search", "Notes"),
            )],
            windows: HashMap::from([("Report", (900, 500))]),
            screenshot: None,
            expect: None,
        },
        Scenario {
            name: "recorded point wins even when the window lookup would also hit",
            target: with_window(
                element("AXButton", "Save", "Notes"),
                "Report",
                Some((40, 30)),
            ),
            recorded: (140, 130),
            world: vec![
                ((140, 130), element("AXButton", "Save", "Notes")),
                ((940, 530), element("AXButton", "Save", "Notes")),
            ],
            windows: HashMap::from([("Report", (900, 500))]),
            screenshot: None,
            expect: Some(((140, 130), ResolutionKind::RecordedPoint)),
        },
        Scenario {
            name: "small window move without title lookup still lands via spiral",
            target: with_window(
                element("AXButton", "Save", "Notes"),
                "Report",
                Some((40, 30)),
            ),
            recorded: (140, 130),
            world: vec![(
                (210, 130),
                with_window(element("AXButton", "Save", "Notes"), "Report", None),
            )],
            // Platform without window lookup (e.g. macOS today).
            windows: HashMap::new(),
            screenshot: None,
            expect: Some(((210, 130), ResolutionKind::SpiralReresolved)),
        },
        Scenario {
            name: "nameless target: window title rejects a same-role decoy in another window",
            target: with_window(element("AXButton", "", "Unknown"), "Invoices", None),
            recorded: (100, 100),
            world: vec![
                // Same role, but it lives in a different window.
                (
                    (100, 100),
                    with_window(element("AXButton", "Send", "Chat"), "Chat", None),
                ),
                (
                    (100, 170),
                    with_window(element("AXButton", "Pay", "Browser"), "Invoices", None),
                ),
            ],
            windows: HashMap::new(),
            screenshot: None,
            expect: Some(((100, 170), ResolutionKind::SpiralReresolved)),
        },
        Scenario {
            name: "nameless target from an old recording (no titles) keeps role-only match",
            target: element("AXButton", "", "Unknown"),
            recorded: (100, 100),
            world: vec![((100, 100), element("AXButton", "Anything", "AnyApp"))],
            windows: HashMap::new(),
            screenshot: None,
            expect: Some(((100, 100), ResolutionKind::RecordedPoint)),
        },
        Scenario {
            name: "named target survives window-title drift (document renamed)",
            target: with_window(element("AXButton", "Save", "Notes"), "Draft v1", None),
            recorded: (100, 100),
            world: vec![(
                (100, 100),
                with_window(element("AXButton", "Save", "Notes"), "Draft v2", None),
            )],
            windows: HashMap::new(),
            screenshot: None,
            expect: Some(((100, 100), ResolutionKind::RecordedPoint)),
        },
        Scenario {
            name: "recorded window title absent skips window strategy cleanly",
            target: element("AXButton", "Save", "Notes"),
            recorded: (100, 100),
            world: vec![((940, 530), element("AXButton", "Save", "Notes"))],
            // A window origin exists, but the recording has no title/offset,
            // and the element is beyond spiral range → must not resolve.
            windows: HashMap::from([("Report", (900, 500))]),
            screenshot: None,
            expect: None,
        },
        Scenario {
            name: "fuzzy match resolves when name has drifted slightly",
            target: element("AXButton", "Save Changes", "Notes"),
            recorded: (100, 100),
            world: vec![((100, 100), element("AXButton", "Save", "Notes"))],
            windows: HashMap::new(),
            screenshot: None,
            expect: Some(((100, 100), ResolutionKind::FuzzyReresolved)),
        },
        {
            // Element has no accessibility descriptor at all (`world` is
            // empty, so every AX-tree strategy fails through) — but a
            // template crop was captured at record time, and its pixels are
            // found nearby in the current screenshot, well outside the
            // spiral's 32 discrete candidate points.
            let mut canvas = block_canvas(400, 400, 1);
            let template = block_template(24, 24);
            embed(&mut canvas, &template, 156, 116); // (recorded + 56, recorded + 16)
            let screenshot = DynamicImage::ImageRgb8(canvas);
            let template_bytes = png_bytes(&DynamicImage::ImageRgb8(template));

            let mut target = element("AXButton", "Deposit", "LegacyTerminal");
            target.template_png = Some(template_bytes.into_boxed_slice());

            Scenario {
                name: "template match finds an element with no accessible descriptor",
                target,
                recorded: (100, 100),
                world: vec![],
                windows: HashMap::new(),
                screenshot: Some(screenshot),
                // Center of the 24x24 embedded template.
                expect: Some(((168, 128), ResolutionKind::TemplateMatched)),
            }
        },
        {
            // A template was captured, but nothing in the current screenshot
            // resembles it (the control is genuinely gone, not just moved) —
            // must not blind-click a low-confidence "best guess".
            let canvas = block_canvas(400, 400, 1);
            let unrelated_template = block_template(24, 24);
            let screenshot = DynamicImage::ImageRgb8(canvas);
            let template_bytes = png_bytes(&DynamicImage::ImageRgb8(unrelated_template));

            let mut target = element("AXButton", "Deposit", "LegacyTerminal");
            target.template_png = Some(template_bytes.into_boxed_slice());

            Scenario {
                name: "template present but not found anywhere must not resolve",
                target,
                recorded: (100, 100),
                world: vec![],
                windows: HashMap::new(),
                screenshot: Some(screenshot),
                expect: None,
            }
        },
    ]
}

#[test]
fn resolution_benchmark_all_scenarios_pass() {
    let all = scenarios();
    let failures: Vec<String> = all.iter().filter_map(|s| run(s).err()).collect();
    println!(
        "resolution benchmark: {}/{} scenarios passed",
        all.len() - failures.len(),
        all.len()
    );
    assert!(
        failures.is_empty(),
        "resolution regressions:\n  {}",
        failures.join("\n  ")
    );
}
