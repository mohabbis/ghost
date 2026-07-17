/** Plain-language label for a per-step verification status. */
const VERIFICATION_LABELS = {
    verified: "Verified",
    failed: "Mismatch",
    skipped: "Skipped",
    not_applicable: "Not checked",
};
/** Epoch-seconds strings from the runtime become readable local times. */
function formatTimestamp(raw) {
    if (!/^\d+$/.test(raw || ""))
        return raw || "";
    const seconds = Number(raw);
    if (!Number.isFinite(seconds) || seconds <= 0)
        return raw;
    return new Date(seconds * 1000).toLocaleString();
}
/** One-line, plain-language summary of what verification found. */
function verificationSummary(counts) {
    const verified = counts.verified || 0;
    const failed = counts.failed || 0;
    const skipped = counts.skipped || 0;
    if (failed > 0) {
        const s = failed === 1 ? "" : "s";
        return `<p class="ghost2-receipt__verify ghost2-receipt__verify--failed">${failed} value${s} did not match the approved plan — review the step${s} marked “Mismatch” below.</p>`;
    }
    if (verified > 0) {
        const skippedNote = skipped > 0 ? ` (${skipped} skipped)` : "";
        return `<p class="ghost2-receipt__verify ghost2-receipt__verify--ok">All ${verified} checked value${verified === 1 ? "" : "s"} matched what you approved${skippedNote}.</p>`;
    }
    return "";
}
/** Human-readable execution receipt panel. */
export function renderExecutionReceipt(receipt) {
    if (!receipt)
        return "";
    const stop = receipt.stopped_early
        ? `<p class="ghost2-receipt__warn">Stopped early: ${escapeHtml(receipt.stop_reason || "verification failed")}</p>`
        : "";
    const undo = receipt.undo_available
        ? '<span class="ghost2-receipt__undo">Undo available</span>'
        : '<span class="ghost2-receipt__undo ghost2-receipt__undo--none">No undo</span>';
    const counts = {};
    const steps = (receipt.steps || [])
        .map((s) => {
        const v = s.verification || {};
        const status = String(v.status || s.outcome || "").toLowerCase();
        counts[status] = (counts[status] || 0) + 1;
        const label = VERIFICATION_LABELS[status] || status;
        const expected = v.expected
            ? `<span class="ghost2-receipt-step__expected">Expected: ${escapeHtml(v.expected)}</span>`
            : "";
        const observed = v.observed
            ? `<span class="ghost2-receipt-step__observed">Observed: ${escapeHtml(v.observed)}</span>`
            : "";
        return `<li class="ghost2-receipt-step ghost2-receipt-step--${escapeAttr(status)}">
        <strong>${escapeHtml(s.label)}</strong>
        <span class="ghost2-verify-chip ghost2-verify-chip--${escapeAttr(status)}">${escapeHtml(label)}</span>
        ${expected}
        ${observed}
      </li>`;
    })
        .join("");
    return `<section class="ghost2-receipt">
    <h3 class="ghost2-receipt__title">Execution receipt</h3>
    <p class="ghost2-receipt__meta">${escapeHtml(receipt.plan_title)} · ${escapeHtml(formatTimestamp(receipt.started_at))} → ${escapeHtml(formatTimestamp(receipt.finished_at))}</p>
    <p class="ghost2-receipt__counts">
      <strong>${receipt.applied}</strong> applied ·
      <strong>${receipt.skipped}</strong> skipped ·
      <strong>${receipt.failed}</strong> failed
    </p>
    ${verificationSummary(counts)}
    ${stop}
    ${undo}
    <ul class="ghost2-receipt__steps">${steps || "<li>No step detail.</li>"}</ul>
  </section>`;
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
