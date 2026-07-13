import type { ExecutionReceipt } from "./types.js";

/** Human-readable execution receipt panel. */
export function renderExecutionReceipt(receipt: ExecutionReceipt): string {
  if (!receipt) return "";
  const stop = receipt.stopped_early
    ? `<p class="ghost2-receipt__warn">Stopped early: ${escapeHtml(receipt.stop_reason || "verification failed")}</p>`
    : "";
  const undo = receipt.undo_available
    ? '<span class="ghost2-receipt__undo">Undo available</span>'
    : '<span class="ghost2-receipt__undo ghost2-receipt__undo--none">No undo</span>';
  const steps = (receipt.steps || [])
    .map((s) => {
      const v = s.verification || {};
      const status = String(v.status || s.outcome || "").toLowerCase();
      return `<li class="ghost2-receipt-step ghost2-receipt-step--${escapeAttr(status)}">
        <strong>${escapeHtml(s.label)}</strong>
        <span class="ghost2-receipt-step__expected">Expected: ${escapeHtml(v.expected || "")}</span>
        <span class="ghost2-receipt-step__observed">Observed: ${escapeHtml(v.observed || "")}</span>
      </li>`;
    })
    .join("");
  return `<section class="ghost2-receipt">
    <h3 class="ghost2-receipt__title">Execution receipt</h3>
    <p class="ghost2-receipt__meta">${escapeHtml(receipt.plan_title)} · ${receipt.started_at} → ${receipt.finished_at}</p>
    <p class="ghost2-receipt__counts">
      <strong>${receipt.applied}</strong> applied ·
      <strong>${receipt.skipped}</strong> skipped ·
      <strong>${receipt.failed}</strong> failed
    </p>
    ${stop}
    ${undo}
    <ul class="ghost2-receipt__steps">${steps || "<li>No step detail.</li>"}</ul>
  </section>`;
}

function escapeHtml(s: string): string {
  return String(s)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function escapeAttr(s: string): string {
  return escapeHtml(s).replace(/'/g, "&#39;");
}
