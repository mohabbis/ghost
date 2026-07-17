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
    this.policyPlan = null;
  }

  async compress(events) {
    if (!this.invoke) {
      throw new Error("Tauri invoke is not available");
    }
    try {
      this.eventCount = events.length;
      const [report, guardReport, policyPlan] = await Promise.all([
        this.invoke("compress_workflow", { events }),
        this.invoke("ghost_guard_audit_compressed", { events }).catch((err) => {
          console.warn("Ghost Guard compressed audit failed:", err);
          return null;
        }),
        this.invoke("routine_policy_plan", { events }).catch((err) => {
          console.warn("Routine policy plan failed:", err);
          return null;
        }),
      ]);
      this.report = report;
      this.guardReport = guardReport;
      this.policyPlan = policyPlan;
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
          ${
            this.policyPlan
              ? `<div class="compression-stat compression-stat--policy${this.policyPlan.can_proceed_with_approvals ? "" : " compression-stat--policy-blocked"}">
            <span class="stat-label">Policy</span>
            <span class="stat-value">${this.policySummaryText()}</span>
          </div>`
              : ""
          }
        </div>

        ${
          this.policyPlan
            ? `
          <div class="compression-policy${this.policyPlan.can_proceed_with_approvals ? "" : " compression-policy--blocked"}">
            <div class="policy-title">Routine policy</div>
            <div class="policy-summary">${escapeHtml(this.policyBlockSummary())}</div>
            ${
              !this.policyPlan.can_proceed_with_approvals
                ? `<div class="policy-blocked">Replay blocked until denied steps are removed or re-recorded.</div>`
                : ""
            }
          </div>
        `
            : ""
        }

        ${
          this.guardReport?.findings?.length
            ? `
          <div class="compression-guard">
            <div class="guard-title">Ghost Guard · ${escapeHtml(this.guardReport.risk_level)} risk</div>
            <div class="guard-summary">${escapeHtml(this.guardReport.summary)}</div>
            ${this.renderSensitiveSuppressionNote()}
            ${this.guardReport.findings
              .slice(0, 6)
              .map((f) => this.renderGuardFinding(f))
              .join("")}
          </div>
        `
            : this.report.redacted_fields > 0
              ? `
          <div class="compression-guard">
            <div class="guard-title">Ghost Guard · secrets suppressed</div>
            ${this.renderSensitiveSuppressionNote()}
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
    const policyStep = this.policyPlan?.steps?.find((s) => s.step_index === idx);
    const riskClass = this.getRiskClass(step, policyStep?.decision);
    const confidence =
      step.confidence !== undefined ? `${(step.confidence * 100).toFixed(0)}%` : "";
    const guardNotes = this.guardFindingsForStep(idx);
    const policyBadge = this.routineDecisionBadge(policyStep?.decision);
    const policyReason =
      policyStep?.decision?.decision === "deny" || policyStep?.decision?.decision === "require_confirmation"
        ? policyStep.decision.reason
        : null;
    const combinedNote = this.combinedReviewNote(idx, policyReason, guardNotes);

    return `
      <div class="compression-step ${riskClass}">
        <span class="step-icon">${icon}</span>
        <div class="step-content">
          <div class="step-text">${description}${policyBadge ? ` ${policyBadge}` : ""}</div>
          ${confidence ? `<div class="step-confidence">confidence: ${confidence}</div>` : ""}
          ${lastRunNote ? `<div class="step-confidence step-lastrun">Last run: ${escapeHtml(lastRunNote)}</div>` : ""}
          ${combinedNote ? `<div class="step-policy-note">${escapeHtml(combinedNote)}</div>` : ""}
        </div>
      </div>
    `;
  }

  combinedReviewNote(stepIdx, policyReason, guardNotes) {
    const parts = [];
    if (policyReason) parts.push(`Policy: ${policyReason}`);
    if (guardNotes.length) {
      parts.push(
        `Guard: ${guardNotes
          .map((n) => n.replace(/^[·!]+ /, ""))
          .join("; ")}`,
      );
    }
    if (!parts.length) return "";
    return parts.join(" · ");
  }

  routineDecisionBadge(decision) {
    if (!decision) return "";
    if (decision.decision === "allow") {
      return `<span class="org-badge org-badge--allow">Allowed</span>`;
    }
    if (decision.decision === "deny") {
      return `<span class="org-badge org-badge--deny" title="${escapeHtml(decision.reason || "")}">Denied</span>`;
    }
    if (decision.decision === "require_confirmation") {
      return `<span class="org-badge org-badge--confirm">Needs approval · ${escapeHtml(decision.risk || "")}</span>`;
    }
    return "";
  }

  policySummaryText() {
    if (!this.policyPlan) return "";
    const { denied_count: denied = 0, confirmation_count: confirm = 0, allow_count: allow = 0 } =
      this.policyPlan;
    if (denied > 0) return `${denied} denied`;
    if (confirm > 0) return `${confirm} confirm`;
    return `${allow} allowed`;
  }

  policyBlockSummary() {
    if (!this.policyPlan) return "";
    const { allow_count: allow = 0, confirmation_count: confirm = 0, denied_count: denied = 0 } =
      this.policyPlan;
    return `${allow} allowed · ${confirm} need approval · ${denied} denied`;
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

  /** Visible proof that password/OTP/payment keystrokes never landed in the recording. */
  renderSensitiveSuppressionNote() {
    const findings = this.guardReport?.findings || [];
    const sensitive = findings.some((f) =>
      ["sensitive_field", "credential_input", "sensitive_app"].includes(f.category),
    );
    const redacted = (this.report?.redacted_fields || 0) > 0;
    if (!sensitive && !redacted) return "";
    return `<div class="guard-suppression-note" data-guard-suppression-note>
      Password, OTP, and payment keystrokes were suppressed during recording — secrets never leave this machine.
    </div>`;
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

  getRiskClass(step, policyDecision) {
    if (policyDecision?.decision === "deny") return "step-policy-deny";
    if (policyDecision?.decision === "require_confirmation") {
      const risk = policyDecision.risk || "";
      if (risk === "high" || risk === "critical") return "step-policy-confirm step-policy-confirm-high";
      return "step-policy-confirm";
    }
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
    this.policyPlan = null;
  }
}
