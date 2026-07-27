function stepKindBadge(step) {
    const k = step.kind?.kind;
    if (!k)
        return "";
    if (k === "semantic_focus" || k === "semantic_set_value" || k === "semantic_verify") {
        return '<span class="badge badge--semantic">semantic</span>';
    }
    if (k === "ui_replay") {
        return '<span class="badge badge--coord">replay</span>';
    }
    if (k === "verify_path") {
        return '<span class="badge badge--verify">verify</span>';
    }
    return "";
}
/** Wire shape from Rust `PolicyDecision` (`tag = "decision"`). */
function decisionKind(decision) {
    if (!decision)
        return "";
    if (typeof decision.decision === "string")
        return decision.decision;
    if (typeof decision.kind === "string")
        return decision.kind;
    if ("deny" in decision)
        return "deny";
    if ("require_confirmation" in decision)
        return "require_confirmation";
    if ("allow" in decision)
        return "allow";
    return "";
}
function decisionRisk(decision) {
    if (!decision)
        return "";
    if (typeof decision.risk === "string")
        return decision.risk;
    const nested = decision.require_confirmation;
    if (nested && typeof nested === "object" && typeof nested.risk === "string") {
        return nested.risk;
    }
    return "";
}
function decisionReason(decision) {
    if (!decision)
        return "";
    if (typeof decision.reason === "string")
        return decision.reason;
    return "";
}
/** Render semantic action steps for unified review (not raw mouse events). */
export function renderActionPlanSteps(plan) {
    if (!plan?.steps?.length) {
        return '<p class="ghost2-empty">No actions in this plan.</p>';
    }
    const rows = plan.steps
        .map((step) => {
        const badge = decisionBadge(step.decision);
        const kindBadge = stepKindBadge(step);
        const risk = decisionKind(step.decision) === "require_confirmation" ? decisionRisk(step.decision) : "";
        const riskClass = risk ? ` ghost2-step--risk-${escapeAttr(risk)}` : "";
        const reason = step.reason
            ? `<span class="ghost2-step__reason">${escapeHtml(step.reason)}</span>`
            : "";
        return `<li class="ghost2-step${riskClass}">
        <span class="ghost2-step__label">✓ ${escapeHtml(step.label)}</span>
        ${kindBadge}
        ${badge}
        ${reason}
      </li>`;
    })
        .join("");
    return `<ul class="ghost2-plan">${rows}</ul>`;
}
export function planSummaryHtml(plan) {
    const s = plan.summary || {};
    return `<div class="ghost2-summary">
    <strong>${escapeHtml(plan.title)}</strong> —
    ${s.total_steps ?? 0} steps
    (${s.filesystem_steps ?? 0} files,
     ${s.ui_steps ?? 0} UI,
     ${s.verify_steps ?? 0} verify)
  </div>`;
}
/**
 * Aye-style approval strip shown above Confirm. Honest about the gate: nothing
 * mutates until the user confirms. Surfaces confirmation risk counts from the
 * plan summary / step decisions.
 */
export function planApprovalGateHtml(plan) {
    const s = plan?.summary;
    const steps = plan?.steps || [];
    let needsConfirm = typeof s?.needs_confirmation === "number" ? s.needs_confirmation : 0;
    let denied = typeof s?.denied === "number" ? s.denied : 0;
    if (!s) {
        needsConfirm = steps.filter((st) => decisionKind(st.decision) === "require_confirmation").length;
        denied = steps.filter((st) => decisionKind(st.decision) === "deny").length;
    }
    const riskCounts = {};
    for (const step of steps) {
        if (decisionKind(step.decision) !== "require_confirmation")
            continue;
        const risk = decisionRisk(step.decision) || "medium";
        riskCounts[risk] = (riskCounts[risk] || 0) + 1;
    }
    const riskChips = Object.entries(riskCounts)
        .map(([risk, n]) => `<span class="badge badge--confirm badge--risk-${escapeAttr(risk)}">${escapeHtml(risk)} · ${n}</span>`)
        .join(" ");
    const confirmLine = needsConfirm > 0
        ? `<p class="approve-gate__counts"><strong>${needsConfirm}</strong> step${needsConfirm === 1 ? "" : "s"} need confirmation${riskChips ? ` ${riskChips}` : ""}.</p>`
        : `<p class="approve-gate__counts">All applyable steps are allowed under current Zone rules.</p>`;
    const denyLine = denied > 0
        ? `<p class="approve-gate__denied"><strong>${denied}</strong> step${denied === 1 ? "" : "s"} denied by policy and will not run.</p>`
        : "";
    return `<aside class="approve-gate" role="status" aria-label="Approval required">
    <p class="approve-gate__promise"><strong>Nothing runs until you approve.</strong></p>
    ${confirmLine}
    ${denyLine}
  </aside>`;
}
export function decisionBadge(decision) {
    if (!decision)
        return "";
    const kind = decisionKind(decision);
    if (kind === "deny") {
        const reason = decisionReason(decision);
        const title = reason ? ` title="${escapeAttr(reason)}"` : "";
        return `<span class="badge badge--deny"${title}>denied</span>`;
    }
    if (kind === "require_confirmation") {
        const risk = decisionRisk(decision);
        const reason = decisionReason(decision);
        const label = risk ? `Needs approval · ${risk}` : "Needs approval";
        const title = reason ? ` title="${escapeAttr(reason)}"` : "";
        const riskClass = risk ? ` badge--risk-${escapeAttr(risk)}` : "";
        return `<span class="badge badge--confirm${riskClass}"${title}>${escapeHtml(label)}</span>`;
    }
    if (kind === "allow") {
        return '<span class="badge badge--allow">ok</span>';
    }
    return "";
}
function escapeHtml(s) {
    return String(s)
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;");
}
function escapeAttr(s) {
    return escapeHtml(s).replace(/'/g, "&#39;");
}
