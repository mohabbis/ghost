//! Local dogfood harness: seed a Zone with messy invoices, then exercise the
//! MCP Organizer tools end-to-end (plan → optional approve → execute).
//!
//! This is CLI-only (`ghost mcp demo`). It does **not** let an MCP client
//! approve its own plan — `--approve-and-run` is an explicit local user action.

use crate::mcp::approval::issue_approval_token;
use crate::mcp::handlers;
use crate::mcp::pending;
use crate::mcp::plan_hash::hash_organizer_plan;
use crate::organizer::planner::plan_zone;
use crate::policy::{DefaultDecision, FolderRule, TrustLevel};
use crate::storage::{open_default, zones};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};

pub struct DemoOptions {
    /// When true, issue a local approval token and execute (same trust path as
    /// the desktop Approve button — but triggered by this CLI flag).
    pub approve_and_run: bool,
    /// Root for demo files (default: `~/ghost-mcp-demo`).
    pub root: Option<PathBuf>,
}

const DEMO_ZONE_NAME: &str = "MCP Demo Inbox";

pub fn run_demo(opts: DemoOptions) -> Result<(), String> {
    let root = opts.root.unwrap_or_else(default_demo_root);
    let inbox = root.join("Inbox");
    let finance = root.join("Finance");
    // Fresh tree each run so re-runs don't collide with prior dated moves.
    if root.exists() {
        fs::remove_dir_all(&root).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(&inbox).map_err(|e| e.to_string())?;
    fs::create_dir_all(&finance).map_err(|e| e.to_string())?;

    seed_messy_inbox(&inbox)?;

    let zone = {
        // Drop the Db before calling MCP handlers — redb is single-process lock.
        let db = open_default().map_err(|e| e.to_string())?;
        let inbox_path = inbox.canonicalize().map_err(|e| e.to_string())?;
        let finance_path = finance.canonicalize().map_err(|e| e.to_string())?;
        let rules = [
            FolderRule {
                path: inbox_path,
                can_read: true,
                can_create: true,
                can_rename: true,
                can_move: true,
                can_copy: false,
                can_delete: false,
                trust: TrustLevel::Automate,
            },
            FolderRule {
                path: finance_path,
                can_read: true,
                can_create: true,
                can_rename: true,
                can_move: true,
                can_copy: false,
                can_delete: false,
                trust: TrustLevel::Automate,
            },
        ];
        if let Some(existing) = zones::list_zones(&db)
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|z| z.name == DEMO_ZONE_NAME)
        {
            // Paths were wiped/recreated — refresh folder rules on the existing Zone.
            for rule in &rules {
                zones::add_folder_rule(&db, &existing.id, rule).map_err(|e| e.to_string())?;
            }
            existing
        } else {
            zones::create_zone_with_rules(
                &db,
                DEMO_ZONE_NAME,
                Some("Local dogfood Zone for ghost mcp demo"),
                DefaultDecision::Ask,
                true, // dated rename so destinations show YYYY-MM prefixes
                &rules,
            )
            .map_err(|e| e.to_string())?
        }
    };

    println!("Seeded demo files under {}", root.display());
    println!("Zone: {} ({})", zone.name, zone.id);
    println!();

    // Same handlers the HTTP/stdio MCP server uses.
    let zones_json = handlers::handle_tool("ghost.list_zones", &json!({}))?;
    let zone_count = zones_json["zones"].as_array().map(|a| a.len()).unwrap_or(0);
    println!("ghost.list_zones → {zone_count} zone(s)");

    let plan_json = handlers::handle_tool("ghost.create_plan", &json!({ "zone_id": zone.id }))?;
    print_plan_summary(&plan_json);

    let approval = handlers::handle_tool("ghost.request_approval", &json!({ "zone_id": zone.id }))?;
    println!();
    println!("ghost.request_approval →");
    println!("  request_id: {}", approval["request_id"]);
    println!("  status:     {}", approval["status"]);
    println!("  expires:    {}", approval["expires_at"]);
    println!("  next:       approve in Ghost desktop, OR re-run with --approve-and-run");

    if !opts.approve_and_run {
        println!();
        println!("Mac / Cursor: paste this into ~/.cursor/mcp.json then restart Cursor:");
        println!("{}", cursor_mcp_json_snippet());
        println!();
        println!(
            "Then ask Cursor: \"Using Ghost MCP, list zones and create a plan for zone {}\"",
            zone.id
        );
        return Ok(());
    }

    // Explicit local approval (CLI user), then execute via the same MCP execute path.
    let token = {
        let db = open_default().map_err(|e| e.to_string())?;
        let plan = plan_zone(&db, &zone.id).map_err(|e| e.to_string())?;
        let plan_hash = hash_organizer_plan(&plan);
        let signed = issue_approval_token(&format!("plan_{}", zone.id), &plan_hash);
        let token = serde_json::to_string(&signed).map_err(|e| e.to_string())?;
        pending::mark_approved_for_zone(&zone.id, token.clone());
        token
    };
    println!();
    println!("Local CLI approval issued (same token shape as desktop Approve).");

    let exec = handlers::handle_tool(
        "ghost.execute_approved_plan",
        &json!({
            "zone_id": zone.id,
            "approval_token": token,
        }),
    )?;
    println!();
    println!("ghost.execute_approved_plan →");
    if let Some(id) = exec.get("execution_id").and_then(|v| v.as_str()) {
        println!("  execution_id: {id}");
    }
    if let Some(status) = exec.get("status").and_then(|v| v.as_str()) {
        println!("  status:       {status}");
    }
    if exec.get("stopped_early").and_then(|v| v.as_bool()) == Some(true) {
        println!(
            "  stopped_early: true ({})",
            exec.get("stop_reason")
                .and_then(|v| v.as_str())
                .unwrap_or("?")
        );
    }
    println!(
        "  applied/skipped/failed: see receipt; files now under {}",
        finance.join("Invoices").display()
    );
    if let Some(id) = exec.get("execution_id").and_then(|v| v.as_str()) {
        println!();
        println!("Undo with MCP: ghost.undo_run {{ \"execution_id\": \"{id}\" }}");
    }
    Ok(())
}

fn default_demo_root() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ghost-mcp-demo")
}

fn seed_messy_inbox(inbox: &Path) -> Result<(), String> {
    let samples = [
        (
            "invoice_acme_jan.pdf",
            b"%PDF-1.4 invoice acme january" as &[u8],
        ),
        ("Receipt_Starbucks.png", b"\x89PNG\r\nreceipt starbucks"),
        ("INV-0042-client.pdf", b"%PDF-1.4 invoice 0042"),
        (
            "notes.txt",
            b"not an invoice - should stay put or low confidence",
        ),
    ];
    for (name, bytes) in samples {
        fs::write(inbox.join(name), bytes).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn print_plan_summary(plan: &serde_json::Value) {
    println!();
    println!(
        "ghost.create_plan → status={}",
        plan.get("status").and_then(|v| v.as_str()).unwrap_or("?")
    );
    if let Some(summary) = plan.get("summary") {
        println!(
            "  scanned={} moves={} renames={} denied={} skipped={}",
            summary
                .get("files_scanned")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            summary
                .get("move_file")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            summary
                .get("rename_file")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            summary.get("denied").and_then(|v| v.as_u64()).unwrap_or(0),
            summary.get("skipped").and_then(|v| v.as_u64()).unwrap_or(0),
        );
    }
    let Some(actions) = plan.get("actions").and_then(|v| v.as_array()) else {
        println!("  (no actions — empty Inbox or Zone misconfigured)");
        return;
    };
    println!("  {} proposed action(s):", actions.len());
    for (i, a) in actions.iter().take(12).enumerate() {
        // Internally tagged: {"kind":"move_file","from":"...","to":"..."}
        let capability = &a["capability"];
        let kind = capability
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        let from = capability
            .get("from")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let to = capability.get("to").and_then(|v| v.as_str()).unwrap_or("");
        let path = capability
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let decision = a
            .pointer("/decision/decision")
            .or_else(|| a.get("decision"))
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        let reason = a.get("reason").and_then(|v| v.as_str()).unwrap_or("");
        println!("  {}. {kind} [{decision}]", i + 1);
        if !from.is_empty() {
            println!("       from: {from}");
        }
        if !to.is_empty() {
            println!("       to:   {to}");
        }
        if !path.is_empty() && from.is_empty() {
            println!("       path: {path}");
        }
        if !reason.is_empty() {
            println!("       why:  {reason}");
        }
    }
    if actions.len() > 12 {
        println!("  … {} more", actions.len() - 12);
    }
}

fn cursor_mcp_json_snippet() -> String {
    let ghost = if cfg!(target_os = "macos") {
        "/Applications/Ghost.app/Contents/MacOS/Ghost"
    } else if cfg!(target_os = "windows") {
        r"C:\\Program Files\\Ghost\\Ghost.exe"
    } else {
        "/workspace/src-tauri/target/debug/ghost"
    };
    format!(
        r#"{{
  "mcpServers": {{
    "ghost": {{
      "command": "{ghost}",
      "args": ["mcp", "serve"]
    }}
  }}
}}"#
    )
}
