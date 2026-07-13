/** Render semantic action steps for unified review (not raw mouse events). */
export function renderActionPlanSteps(plan) {
    if (!plan?.steps?.length) {
        return '<p class="ghost2-empty">No actions in this plan.</p>';
    }
    const rows = plan.steps
        .map((step) => {
        const badge = decisionBadge(step.decision);
        return `<li class="ghost2-step">
        <span class="ghost2-step__label">✓ ${escapeHtml(step.label)}</span>
        ${badge}
      </li>`;
    })
        .join("");
    return `<ul class="ghost2-plan">${rows}</ul>`;
}
export function planSummaryHtml(plan) {
    const s = plan.summary || {};
    return `<div class="ghost2-summary">
    <strong>${plan.title}</strong> —
    ${s.total_steps ?? 0} steps
    (${s.filesystem_steps ?? 0} files,
     ${s.ui_steps ?? 0} UI,
     ${s.verify_steps ?? 0} verify)
  </div>`;
}
function decisionBadge(decision) {
    if (!decision)
        return "";
    if ("deny" in decision || decision.kind === "deny") {
        return '<span class="badge badge--deny">denied</span>';
    }
    if ("require_confirmation" in decision || decision.kind === "require_confirmation") {
        return '<span class="badge badge--confirm">review</span>';
    }
    return '<span class="badge badge--allow">ok</span>';
}
function escapeHtml(s) {
    return String(s)
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;");
}
