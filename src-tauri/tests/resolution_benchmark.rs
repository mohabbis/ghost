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
use ghost_lib::core::replay_support::{try_resolve_click_point_traced, ResolutionKind};
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

fn run(scenario: &Scenario) -> Result<(), String> {
    let world = &scenario.world;
    let windows = &scenario.windows;
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
    );
    if outcome == scenario.expect {
        Ok(())
    } else {
        Err(format!(
            "{}: expected {:?}, got {:?}",
            scenario.name, scenario.expect, outcome
        ))
    }
}

fn scenarios() -> Vec<Scenario> {
    vec![
        Scenario {
            name: "unmoved element resolves at recorded point",
            target: element("AXButton", "Save", "Notes"),
            recorded: (100, 100),
            world: vec![((100, 100), element("AXButton", "Save", "Notes"))],
            windows: HashMap::new(),
            expect: Some(((100, 100), ResolutionKind::RecordedPoint)),
        },
        Scenario {
            name: "element moved within spiral range is re-resolved",
            target: element("AXButton", "Save", "Notes"),
            recorded: (100, 100),
            world: vec![((170, 100), element("AXButton", "Save", "Notes"))],
            windows: HashMap::new(),
            expect: Some(((170, 100), ResolutionKind::SpiralReresolved)),
        },
        Scenario {
            name: "element gone entirely must not resolve",
            target: element("AXButton", "Save", "Notes"),
            recorded: (100, 100),
            world: vec![],
            windows: HashMap::new(),
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
            expect: Some(((100, 170), ResolutionKind::SpiralReresolved)),
        },
        Scenario {
            name: "nameless target from an old recording (no titles) keeps role-only match",
            target: element("AXButton", "", "Unknown"),
            recorded: (100, 100),
            world: vec![((100, 100), element("AXButton", "Anything", "AnyApp"))],
            windows: HashMap::new(),
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
            expect: None,
        },
        Scenario {
            name: "fuzzy match resolves when name has drifted slightly",
            target: element("AXButton", "Save Changes", "Notes"),
            recorded: (100, 100),
            world: vec![((100, 100), element("AXButton", "Save", "Notes"))],
            windows: HashMap::new(),
            expect: Some(((100, 100), ResolutionKind::FuzzyReresolved)),
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
