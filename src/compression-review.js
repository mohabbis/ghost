/**
 * Compressed-Step Review Timeline UI
 * 
 * Displays semantic workflow steps (Click, TypeText, Shortcut, Scroll, Wait)
 * with confidence scores, warnings, and risk indicators.
 * 
 * This is Ghost's trust model in action: deterministic workflow review before execution.
 */

export class CompressionReview {
  constructor(containerId) {
    this.container = document.getElementById(containerId);
    this.report = null;
  }

  async compress(events) {
    try {
      this.eventCount = events.length;
      this.report = await window.__TAURI__.invoke('compress_workflow', { events });
      this.render();
      return this.report;
    } catch (err) {
      console.error('Compression failed:', err);
      this.renderError(err.message);
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
        </div>

        ${this.report.warnings.length > 0 ? `
          <div class="compression-warnings">
            <div class="warning-title">⚠️ Review flags</div>
            ${this.report.warnings.map(w => this.renderWarning(w)).join('')}
          </div>
        ` : ''}

        <div class="compression-steps">
          ${this.report.steps.map((step, idx) => this.renderStep(step, idx, lastRun.get(idx))).join('')}
        </div>
      </div>
    `;

    this.container.innerHTML = html;
  }

  // Map the last replay's per-click resolution outcomes onto compressed steps
  // via the report's raw-event spans (spans are non-contiguous — dropped
  // delays and standalone releases consume events without emitting a step,
  // so never prefix-sum raw_event_count). Empty unless a replay of this exact
  // event list ran this session (handoff set by main.js after each run).
  lastRunOutcomes() {
    const outcomes = new Map();
    const handoff = window.__ghostLastReplayTrace;
    const spans = this.report?.raw_spans;
    if (!handoff || !Array.isArray(spans) || spans.length === 0) return outcomes;
    if (handoff.eventCount !== this.eventCount) return outcomes;
    for (const t of handoff.trace || []) {
      if (t.kind !== 'CoordinateFallback' && t.kind !== 'NoDescriptor') continue;
      const stepIdx = spans.findIndex(
        ([start, len]) => start <= t.step_index && t.step_index < start + len
      );
      if (stepIdx >= 0 && !outcomes.has(stepIdx)) {
        outcomes.set(
          stepIdx,
          t.kind === 'CoordinateFallback'
            ? 'lost its element last run — clicked recorded coordinates'
            : 'ran on raw coordinates last run'
        );
      }
    }
    return outcomes;
  }

  renderStep(step, idx, lastRunNote) {
    const icon = this.getStepIcon(step.kind);
    const description = this.getStepDescription(step);
    const riskClass = this.getRiskClass(step);
    const confidence = step.confidence !== undefined ?
      (step.confidence * 100).toFixed(0) + '%' : '';

    return `
      <div class="compression-step ${riskClass}">
        <span class="step-icon">${icon}</span>
        <div class="step-content">
          <div class="step-text">${description}</div>
          ${confidence ? `<div class="step-confidence">confidence: ${confidence}</div>` : ''}
          ${lastRunNote ? `<div class="step-confidence step-lastrun">⚠ ${lastRunNote}</div>` : ''}
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
      click: '🖱️',
      type_text: '⌨️',
      shortcut: '⌘',
      scroll: '↕️',
      wait: '⏳',
      unknown: '❓'
    };
    return icons[kind] || '•';
  }

  getStepDescription(step) {
    switch (step.kind) {
      case 'click': {
        const button = step.button === 'left' ? 'left' : 'right';
        if (step.target) {
          return `Clicked <strong>"${step.target.name}"</strong> (${step.target.role})`;
        } else if (step.fallback_coords) {
          return `Clicked at (${step.fallback_coords[0]}, ${step.fallback_coords[1]})`;
        }
        return 'Clicked';
      }
      case 'type_text': {
        if (step.secure_field) {
          return `Typed <em>[redacted: ${step.char_count} chars in secure field]</em>`;
        } else if (step.redacted) {
          return `Typed <em>[redacted: ${step.char_count} chars]</em>`;
        } else if (step.text) {
          return `Typed <strong>"${this.truncate(step.text, 40)}"</strong>`;
        }
        return `Typed (${step.char_count} chars)`;
      }
      case 'shortcut':
        return `Pressed <strong>${step.combo}</strong>`;
      case 'scroll': {
        const dir = `${step.direction} ${step.magnitude}`;
        return `Scrolled ${dir}`;
      }
      case 'wait':
        return `Waited ${step.ms}ms`;
      case 'unknown':
        return `Unknown: ${step.description}`;
      default:
        return JSON.stringify(step);
    }
  }

  getWarningText(warning) {
    switch (warning.kind) {
      case 'coordinate_only_target':
        return `⚠️ Step ${warning.step_index + 1}: coordinate-only target (may break if window moves)`;
      case 'low_confidence':
        return `⚠️ Step ${warning.step_index + 1}: low confidence (${(warning.confidence * 100).toFixed(0)}%)`;
      case 'secure_field_typing':
        return `🔒 Step ${warning.step_index + 1}: typing in secure field (redacted)`;
      default:
        return JSON.stringify(warning);
    }
  }

  getRiskClass(step) {
    if (step.kind === 'unknown') return 'step-unknown';
    if (step.confidence !== undefined && step.confidence < 0.5) return 'step-low-confidence';
    if (step.kind === 'type_text' && step.secure_field) return 'step-secure';
    return 'step-normal';
  }

  truncate(str, len) {
    return str.length > len ? str.slice(0, len) + '…' : str;
  }

  renderError(msg) {
    if (this.container) {
      this.container.innerHTML = `
        <div class="compression-error">
          <p>${msg}</p>
        </div>
      `;
    }
  }

  clear() {
    if (this.container) {
      this.container.innerHTML = '';
    }
    this.report = null;
  }
}
