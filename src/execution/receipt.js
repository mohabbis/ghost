/** Plain-language label for a per-step verification status. */
const VERIFICATION_LABELS = {
    verified: "Verified",
    failed: "Mismatch",
    skipped: "Skipped",
    not_applicable: "Not checked",
};
/** Curly quotes match launch-demo / Product Hunt money-shot copy. */
const LQ = "\u201c";
const RQ = "\u201d";
/** Epoch-seconds strings from the runtime become readable local times. */
function formatTimestamp(raw) {
    if (!/^\d+$/.test(raw || ""))
        return raw || "";
    const seconds = Number(raw);
    if (!Number.isFinite(seconds) || seconds <= 0)
        return raw;
    return new Date(seconds * 1000).toLocaleString();
}
/**
 * Launch money-shot line: `Cell C7 expected “12,900” · observed “12,090” → stop`.
 * Exported so Replay History can preview the same copy without opening the receipt.
 */
export function formatMismatchHaltLine(label, expected, observed) {
    const step = (label || "Value").trim() || "Value";
    return `${step} expected ${LQ}${expected}${RQ} · observed ${LQ}${observed}${RQ} → stop`;
}
/** First failed verification step that carries both expected and observed values. */
export function findFirstMismatchStep(receipt) {
    for (const s of receipt?.steps || []) {
        const v = (s.verification || {});
        const status = String(v.status || s.outcome || "").toLowerCase();
        if (status !== "failed")
            continue;
        const expected = String(v.expected ?? "").trim();
        const observed = String(v.observed ?? "").trim();
        if (!expected && !observed)
            continue;
        return {
            label: String(s.label || v.label || "Value"),
            expected: expected || "(empty)",
            observed: observed || "(empty)",
        };
    }
    return null;
}
/** One-line, plain-language summary of what verification found. */
function verificationSummary(counts) {
    const verified = counts.verified || 0;
    const failed = counts.failed || 0;
    const skipped = counts.skipped || 0;
    if (failed > 0) {
        const s = failed === 1 ? "" : "s";
        return `<p class="ghost2-receipt__verify ghost2-receipt__verify--failed">${failed} value${s} did not match the approved plan — Ghost halted the run. Review the halt row below.</p>`;
    }
    if (verified > 0) {
        const skippedNote = skipped > 0 ? ` (${skipped} skipped)` : "";
        return `<p class="ghost2-receipt__verify ghost2-receipt__verify--ok">All ${verified} checked value${verified === 1 ? "" : "s"} matched what you approved${skippedNote}.</p>`;
    }
    return "";
}
/** Prominent halt banner for stopped_early / Failed verification. */
function renderHaltBanner(receipt) {
    const mismatch = findFirstMismatchStep(receipt);
    if (!mismatch && !receipt.stopped_early && !(receipt.failed > 0)) {
        return "";
    }
    const copy = mismatch
        ? formatMismatchHaltLine(mismatch.label, mismatch.expected, mismatch.observed)
        : escapeHtml(receipt.stop_reason || "verification failed — run stopped");
    const line = mismatch ? escapeHtml(copy) : copy;
    return `<div class="ghost2-receipt__halt" role="alert">
    <span class="ghost2-receipt__halt-tag">halt</span>
    <span class="ghost2-receipt__halt-copy">${line}</span>
  </div>`;
}
/** Human-readable execution receipt panel. */
export function renderExecutionReceipt(receipt) {
    if (!receipt)
        return "";
    const halt = renderHaltBanner(receipt);
    // Keep a short fallback only when we couldn't build the money-shot halt row.
    const stop = !halt && receipt.stopped_early
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
        let valueDetail = "";
        if (status === "failed" && (v.expected || v.observed)) {
            // Label is already in <strong>; keep the money-shot expected/observed → stop.
            const expected = String(v.expected || "(empty)");
            const observed = String(v.observed || "(empty)");
            valueDetail = `<span class="ghost2-receipt-step__halt-line">expected ${LQ}${escapeHtml(expected)}${RQ} · observed ${LQ}${escapeHtml(observed)}${RQ} → stop</span>`;
        }
        else {
            const expected = v.expected
                ? `<span class="ghost2-receipt-step__expected">Expected: ${escapeHtml(v.expected)}</span>`
                : "";
            const observed = v.observed
                ? `<span class="ghost2-receipt-step__observed">Observed: ${escapeHtml(v.observed)}</span>`
                : "";
            valueDetail = `${expected}${observed}`;
        }
        return `<li class="ghost2-receipt-step ghost2-receipt-step--${escapeAttr(status)}">
        <strong>${escapeHtml(s.label)}</strong>
        <span class="ghost2-verify-chip ghost2-verify-chip--${escapeAttr(status)}">${escapeHtml(label)}</span>
        ${valueDetail}
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
    ${halt}
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
