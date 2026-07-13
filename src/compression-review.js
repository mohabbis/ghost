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
    this.guardReport = null;
  }

  async compress(events) {
    if (!this.invoke) {
      throw new Error("Tauri invoke is not available");
    }
    try {
      this.eventCount = events.length;
      const [report, guardReport] = await Promise.all([
        this.invoke("compress_workflow", { events }),
        this.invoke("ghost_guard_audit_compressed", { events }).catch((err) => {
          console.warn("Ghost Guard compressed audit failed:", err);
          return null;
        }),
      ]);
      this.report = report;
      this.guardReport = guardReport;
      this.render();
      return this.report;
    } catch (err) {
      console.error("Compression failed:", err);
      this.renderError(formatInvokeError(err));
      throw err;
    }
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
          ${
            this.guardReport
              ? `<div class="compression-stat compression-stat--guard">
            <span class="stat-label">Guard</span>
            <span class="stat-value">${this.guardReport.score}/100</span>
          </div>`
              : ""
          }
        </div>

        ${
          this.guardReport?.findings?.length
            ? `
          <div class="compression-guard">
            <div class="guard-title">Ghost Guard · ${escapeHtml(this.guardReport.risk_level)} risk</div>
            <div class="guard-summary">${escapeHtml(this.guardReport.summary)}</div>
            ${this.guardReport.findings
              .slice(0, 6)
              .map((f) => this.renderGuardFinding(f))
              .join("")}
          </div>
        `
            : ""
        }

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
    const guardNotes = this.guardFindingsForStep(idx);

    return `
      <div class="compression-step ${riskClass}">
        <span class="step-icon">${icon}</span>
        <div class="step-content">
          <div class="step-text">${description}</div>
          ${confidence ? `<div class="step-confidence">confidence: ${confidence}</div>` : ""}
          ${lastRunNote ? `<div class="step-confidence step-lastrun">Last run: ${escapeHtml(lastRunNote)}</div>` : ""}
          ${guardNotes.map((note) => `<div class="step-guard-note">${note}</div>`).join("")}
        </div>
      </div>
    `;
  }

  guardFindingsForStep(stepIdx) {
    if (!this.guardReport?.findings?.length) return [];
    return this.guardReport.findings
      .filter((f) => f.step_index === stepIdx)
      .map((f) => `${this.guardSeverityLabel(f.severity)} ${escapeHtml(f.title)}`);
  }

  guardSeverityLabel(severity) {
    const labels = { low: "·", medium: "!", high: "!!", critical: "!!!" };
    return labels[severity] || "·";
  }

  renderGuardFinding(finding) {
    const step =
      finding.step_index != null ? ` (step ${finding.step_index + 1})` : "";
    return `<div class="guard-finding guard-finding--${escapeHtml(finding.severity)}">${this.guardSeverityLabel(finding.severity)} ${escapeHtml(finding.title)}${step}</div>`;
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
    this.guardReport = null;
  }
}
