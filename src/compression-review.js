/**
 * Compressed-Step Review Timeline UI
 *
 * Displays semantic workflow steps (Click, TypeText, Shortcut, Scroll, Wait)
 * with confidence scores, warnings, and risk indicators.
 */

function escapeHtml(value) {
  const div = document.createElement("div");
  div.textContent = String(value ?? "");
  return div.innerHTML;
}

function formatInvokeError(err) {
  if (err == null || err === "") return "Unknown error";
  if (typeof err === "string") return err;
  if (err instanceof Error && err.message) return err.message;
  if (typeof err === "object" && typeof err.message === "string") return err.message;
  return String(err);
}

export class CompressionReview {
  constructor(containerId, invokeFn) {
    this.container = document.getElementById(containerId);
    this.invoke = invokeFn;
    this.report = null;
    this.policyPlan = null;
  }

  async compress(events) {
    if (!this.invoke) {
      throw new Error("Tauri invoke is not available");
    }
    try {
      this.eventCount = events.length;
      this.policyPlan = null;
      this.report = await this.invoke("compress_workflow", { events });
      this.render();
      return this.report;
    } catch (err) {
      console.error("Compression failed:", err);
      this.renderError(formatInvokeError(err));
      throw err;
    }
  }

  setPolicyPlan(plan) {
    this.policyPlan = plan || null;
    this.render();
  }

  policyBadgeForStep(idx) {
    const step = this.policyPlan?.steps?.find((s) => s.step_index === idx);
    const decision = step?.decision;
    if (!decision) return "";
    if (decision.decision === "allow")
      return `<span class="org-badge org-badge--allow">Allowed</span>`;
    if (decision.decision === "deny")
      return `<span class="org-badge org-badge--deny" title="${escapeHtml(decision.reason || "")}">Denied</span>`;
    if (decision.decision === "require_confirmation")
      return `<span class="org-badge org-badge--confirm">Needs approval</span>`;
    return "";
  }

  policyStrip() {
    if (!this.policyPlan) return "";
    const p = this.policyPlan;
    const blocked = !p.can_proceed_with_approvals;
    return `
      <div class="compression-policy-strip${blocked ? " compression-policy-strip--blocked" : ""}">
        <strong>Policy</strong>
        ${p.confirmation_count || 0} need approval · ${p.allow_count || 0} allowed · ${p.denied_count || 0} denied
        ${
          blocked
            ? " — <em>Replay blocked until denied steps are fixed</em>"
            : " — Preview Policy, then Approve &amp; Replay"
        }
      </div>`;
  }

  render() {
    if (!this.report || !this.container) return;
    const lastRun = this.lastRunOutcomes();

    const html = `
      <div class="compression-review">
        <div class="compression-header">
          <div class="compression-stat">
            <span class="stat-label">Steps</span>
            <span class="stat-value">${this.report.compressed_step_count}</span>
          </div>
          <div class="compression-stat">
            <span class="stat-label">Reduction</span>
            <span class="stat-value">${(this.report.reduction_ratio * 100).toFixed(0)}%</span>
          </div>
          <div class="compression-stat">
            <span class="stat-label">Redacted</span>
            <span class="stat-value">${this.report.redacted_fields}</span>
          </div>
        </div>

        ${this.policyStrip()}

        ${
          this.report.warnings.length > 0
            ? `
          <div class="compression-warnings">
            <div class="warning-title">Review flags</div>
            ${this.report.warnings.map((w) => this.renderWarning(w)).join("")}
          </div>
        `
            : ""
        }

        <div class="compression-steps">
          ${this.report.steps.map((step, idx) => this.renderStep(step, idx, lastRun.get(idx))).join("")}
        </div>
      </div>
    `;

    this.container.innerHTML = html;
  }

  lastRunOutcomes() {
    const outcomes = new Map();
    const handoff = window.__ghostLastReplayTrace;
    const spans = this.report?.raw_spans;
    if (!handoff || !Array.isArray(spans) || spans.length === 0) return outcomes;
    if (handoff.eventCount !== this.eventCount) return outcomes;
    for (const t of handoff.trace || []) {
      if (t.kind !== "CoordinateFallback" && t.kind !== "NoDescriptor") continue;
      const stepIdx = spans.findIndex(
        ([start, len]) => start <= t.step_index && t.step_index < start + len,
      );
      if (stepIdx >= 0 && !outcomes.has(stepIdx)) {
        outcomes.set(
          stepIdx,
          t.kind === "CoordinateFallback"
            ? "lost its element last run — clicked recorded coordinates"
            : "ran on raw coordinates last run",
        );
      }
    }
    return outcomes;
  }

  renderStep(step, idx, lastRunNote) {
    const icon = this.getStepIcon(step.kind);
    const description = this.getStepDescription(step);
    const riskClass = this.getRiskClass(step);
    const confidence =
      step.confidence !== undefined ? `${(step.confidence * 100).toFixed(0)}%` : "";
    const policyBadge = this.policyBadgeForStep(idx);

    return `
      <div class="compression-step ${riskClass}">
        <span class="step-icon">${icon}</span>
        <div class="step-content">
          <div class="step-text">${description}${policyBadge ? ` ${policyBadge}` : ""}</div>
          ${confidence ? `<div class="step-confidence">confidence: ${confidence}</div>` : ""}
          ${lastRunNote ? `<div class="step-confidence step-lastrun">Last run: ${escapeHtml(lastRunNote)}</div>` : ""}
        </div>
      </div>
    `;
  }

  renderWarning(warning) {
    const text = this.getWarningText(warning);
    return `<div class="warning-item">${text}</div>`;
  }

  getStepIcon(kind) {
    const icons = {
      click: "Click",
      type_text: "Type",
      shortcut: "Key",
      scroll: "Scroll",
      wait: "Wait",
      unknown: "?",
    };
    return icons[kind] || "•";
  }

  getStepDescription(step) {
    switch (step.kind) {
      case "click": {
        if (step.target) {
          return `Clicked <strong>"${escapeHtml(step.target.name)}"</strong> (${escapeHtml(step.target.role)})`;
        }
        if (step.fallback_coords) {
          return `Clicked at (${step.fallback_coords[0]}, ${step.fallback_coords[1]})`;
        }
        return "Clicked";
      }
      case "type_text": {
        if (step.secure_field) {
          return `Typed <em>[redacted: ${step.char_count} chars in secure field]</em>`;
        }
        if (step.redacted) {
          return `Typed <em>[redacted: ${step.char_count} chars]</em>`;
        }
        if (step.text) {
          return `Typed <strong>"${escapeHtml(this.truncate(step.text, 40))}"</strong>`;
        }
        return `Typed (${step.char_count} chars)`;
      }
      case "shortcut":
        return `Pressed <strong>${escapeHtml(step.combo)}</strong>`;
      case "scroll": {
        const dir = `${step.direction} ${step.magnitude}`;
        return `Scrolled ${escapeHtml(dir)}`;
      }
      case "wait":
        return `Waited ${step.ms}ms`;
      case "unknown":
        return `Unknown: ${escapeHtml(step.description || "")}`;
      default:
        return escapeHtml(JSON.stringify(step));
    }
  }

  getWarningText(warning) {
    switch (warning.kind) {
      case "coordinate_only_target":
        return `Step ${warning.step_index + 1}: coordinate-only target (may break if window moves)`;
      case "low_confidence":
        return `Step ${warning.step_index + 1}: low confidence (${(warning.confidence * 100).toFixed(0)}%)`;
      case "secure_field_typing":
        return `Step ${warning.step_index + 1}: typing in secure field (redacted)`;
      default:
        return escapeHtml(JSON.stringify(warning));
    }
  }

  getRiskClass(step) {
    if (step.kind === "unknown") return "step-unknown";
    if (step.confidence !== undefined && step.confidence < 0.5) return "step-low-confidence";
    if (step.kind === "type_text" && step.secure_field) return "step-secure";
    return "step-normal";
  }

  truncate(str, len) {
    return str.length > len ? `${str.slice(0, len)}…` : str;
  }

  renderError(msg) {
    if (this.container) {
      this.container.innerHTML = `
        <div class="compression-error">
          <p>${escapeHtml(msg)}</p>
        </div>
      `;
    }
  }

  clear() {
    if (this.container) {
      this.container.innerHTML = "";
    }
    this.report = null;
  }
}
