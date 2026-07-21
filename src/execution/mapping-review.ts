import type { ActionPlan } from "./types.js";

// DOM Elements
let sourceFieldsContainer: HTMLDivElement | null = null;
let targetFieldsContainer: HTMLDivElement | null = null;
let mappingTableBody: HTMLTableSectionElement | null = null;
let addMappingButton: HTMLButtonElement | null = null;
let validateMappingButton: HTMLButtonElement | null = null;
let clearMappingsButton: HTMLButtonElement | null = null;
let exportMappingsButton: HTMLButtonElement | null = null;
let searchInput: HTMLInputElement | null = null;
let filterSelect: HTMLSelectElement | null = null;
let statusMessage: HTMLDivElement | null = null;

// State
interface FieldMapping {
  id: string;
  sourceField: string;
  targetField: string;
  transformation?: string | null;
  confidence?: number;
  status: "pending" | "mapped" | "verified" | "error";
  lastUpdated?: string;
}

interface MappingReviewState {
  mappings: FieldMapping[];
  selectedMappingId: string | null;
  sourceFields: string[];
  targetFields: string[];
  validationErrors: Record<string, string>;
  isDirty: boolean;
}

let currentState: MappingReviewState = {
  mappings: [],
  selectedMappingId: null,
  sourceFields: [],
  targetFields: [],
  validationErrors: {},
  isDirty: false,
};

// Initialization
export function initializeMappingReview(containerId: string): void {
  const container = document.getElementById(containerId);
  if (!container) {
    console.error(`Mapping review container #${containerId} not found`);
    return;
  }

  // Find DOM elements
  sourceFieldsContainer = container.querySelector(".source-fields-container");
  targetFieldsContainer = container.querySelector(".target-fields-container");
  mappingTableBody = container.querySelector(".mapping-table-body");
  addMappingButton = container.querySelector(".add-mapping-btn");
  validateMappingButton = container.querySelector(".validate-mapping-btn");
  clearMappingsButton = container.querySelector(".clear-mappings-btn");
  exportMappingsButton = container.querySelector(".export-mappings-btn");
  searchInput = container.querySelector(".mapping-search-input");
  filterSelect = container.querySelector(".mapping-filter-select");
  statusMessage = container.querySelector(".status-message");

  // Attach event listeners
  attachEventListeners();

  // Initial render
  renderSourceFields();
  renderTargetFields();
  renderMappingTable();
}

function attachEventListeners(): void {
  if (addMappingButton) {
    addMappingButton.addEventListener("click", handleAddMapping);
  }

  if (validateMappingButton) {
    validateMappingButton.addEventListener("click", handleValidateMappings);
  }

  if (clearMappingsButton) {
    clearMappingsButton.addEventListener("click", handleClearMappings);
  }

  if (exportMappingsButton) {
    exportMappingsButton.addEventListener("click", handleExportMappings);
  }

  if (searchInput) {
    searchInput.addEventListener("input", handleSearchFilter);
  }

  if (filterSelect) {
    filterSelect.addEventListener("change", handleStatusFilter);
  }

  // Keyboard shortcuts
  document.addEventListener("keydown", handleKeyboardShortcuts);
}

function handleKeyboardShortcuts(event: KeyboardEvent): void {
  // Ctrl/Cmd + N: New mapping
  if ((event.ctrlKey || event.metaKey) && event.key === "n") {
    event.preventDefault();
    handleAddMapping();
  }

  // Ctrl/Cmd + S: Save/Validate
  if ((event.ctrlKey || event.metaKey) && event.key === "s") {
    event.preventDefault();
    handleValidateMappings();
  }

  // Ctrl/Cmd + E: Export
  if ((event.ctrlKey || event.metaKey) && event.key === "e") {
    event.preventDefault();
    handleExportMappings();
  }

  // Delete: Remove selected mapping
  if (event.key === "Delete" || event.key === "Backspace") {
    if (currentState.selectedMappingId) {
      event.preventDefault();
      removeMapping(currentState.selectedMappingId);
    }
  }
}

// Rendering functions
export function renderSourceFields(): void {
  if (!sourceFieldsContainer) return;

  if (currentState.sourceFields.length === 0) {
    sourceFieldsContainer.innerHTML = `
      <div class="empty-state">
        <p>No source fields available</p>
        <button class="btn btn--secondary load-source-btn">Load Source Fields</button>
      </div>
    `;

    const loadBtn = sourceFieldsContainer.querySelector(".load-source-btn");
    if (loadBtn) {
      loadBtn.addEventListener("click", () => loadSourceFieldsFromPlan());
    }
    return;
  }

  const html = `
    <div class="fields-header">
      <h3>Source Fields</h3>
      <span class="field-count">${currentState.sourceFields.length}</span>
    </div>
    <ul class="fields-list">
      ${currentState.sourceFields
        .map(
          (field) => `
        <li class="field-item ${isFieldMapped(field, "source") ? "mapped" : ""}" data-field="${escapeAttr(field)}">
          <span class="field-name">${escapeHtml(field)}</span>
          ${isFieldMapped(field, "source") ? '<span class="mapped-indicator">✓</span>' : ""}
        </li>
      `,
        )
        .join("")}
    </ul>
  `;

  sourceFieldsContainer.innerHTML = html;

  // Add click handlers for source fields
  const fieldItems = sourceFieldsContainer.querySelectorAll(".field-item");
  fieldItems.forEach((item) => {
    item.addEventListener("click", () => handleFieldSelect(item, "source"));
  });
}

export function renderTargetFields(): void {
  if (!targetFieldsContainer) return;

  if (currentState.targetFields.length === 0) {
    targetFieldsContainer.innerHTML = `
      <div class="empty-state">
        <p>No target fields available</p>
        <button class="btn btn--secondary load-target-btn">Load Target Fields</button>
      </div>
    `;

    const loadBtn = targetFieldsContainer.querySelector(".load-target-btn");
    if (loadBtn) {
      loadBtn.addEventListener("click", () => loadTargetFieldsFromPlan());
    }
    return;
  }

  const html = `
    <div class="fields-header">
      <h3>Target Fields</h3>
      <span class="field-count">${currentState.targetFields.length}</span>
    </div>
    <ul class="fields-list">
      ${currentState.targetFields
        .map(
          (field) => `
        <li class="field-item ${isFieldMapped(field, "target") ? "mapped" : ""}" data-field="${escapeAttr(field)}">
          <span class="field-name">${escapeHtml(field)}</span>
          ${isFieldMapped(field, "target") ? '<span class="mapped-indicator">✓</span>' : ""}
        </li>
      `,
        )
        .join("")}
    </ul>
  `;

  targetFieldsContainer.innerHTML = html;

  // Add click handlers for target fields
  const fieldItems = targetFieldsContainer.querySelectorAll(".field-item");
  fieldItems.forEach((item) => {
    item.addEventListener("click", () => handleFieldSelect(item, "target"));
  });
}

export function renderMappingTable(): void {
  if (!mappingTableBody) return;

  const filteredMappings = getFilteredMappings();

  if (filteredMappings.length === 0) {
    mappingTableBody.innerHTML = `
      <tr class="empty-row">
        <td colspan="5">
          <div class="empty-state">
            <p>No mappings to display</p>
            <button class="btn btn--primary add-first-mapping-btn">Create your first mapping</button>
          </div>
        </td>
      </tr>
    `;

    const addBtn = mappingTableBody.querySelector(".add-first-mapping-btn");
    if (addBtn) {
      addBtn.addEventListener("click", handleAddMapping);
    }
    return;
  }

  const html = filteredMappings
    .map(
      (mapping) => `
    <tr class="mapping-row ${mapping.status} ${currentState.selectedMappingId === mapping.id ? "selected" : ""}" data-mapping-id="${escapeAttr(mapping.id)}">
      <td class="col-source">
        <span class="field-badge">${escapeHtml(mapping.sourceField)}</span>
      </td>
      <td class="col-arrow">
        <svg class="arrow-icon" viewBox="0 0 24 24" width="16" height="16">
          <path d="M5 12h14M12 5l7 7-7 7" stroke="currentColor" stroke-width="2" fill="none"/>
        </svg>
      </td>
      <td class="col-target">
        <span class="field-badge">${escapeHtml(mapping.targetField)}</span>
      </td>
      <td class="col-transformation">
        ${mapping.transformation ? `<code class="transform-code">${escapeHtml(mapping.transformation)}</code>` : '<span class="no-transform">—</span>'}
      </td>
      <td class="col-status">
        ${renderStatusBadge(mapping.status)}
        ${mapping.confidence !== undefined ? `<span class="confidence-score">${Math.round(mapping.confidence * 100)}%</span>` : ""}
      </td>
      <td class="col-actions">
        <button class="icon-btn edit-btn" title="Edit mapping" data-action="edit">
          <svg viewBox="0 0 24 24" width="16" height="16"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" stroke="currentColor" stroke-width="2" fill="none"/><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" stroke="currentColor" stroke-width="2" fill="none"/></svg>
        </button>
        <button class="icon-btn delete-btn" title="Remove mapping" data-action="delete">
          <svg viewBox="0 0 24 24" width="16" height="16"><polyline points="3 6 5 6 21 6" stroke="currentColor" stroke-width="2" fill="none"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" stroke="currentColor" stroke-width="2" fill="none"/></svg>
        </button>
      </td>
    </tr>
  `,
    )
    .join("");

  mappingTableBody.innerHTML = html;

  // Add event handlers for row actions
  const rows = mappingTableBody.querySelectorAll(".mapping-row");
  rows.forEach((row) => {
    row.addEventListener("click", (event) => {
      const target = event.target as HTMLElement;
      if (target.closest(".icon-btn")) return; // Don't select row when clicking action buttons

      const mappingId = row.getAttribute("data-mapping-id");
      if (mappingId) {
        selectMapping(mappingId);
      }
    });
  });

  // Add event handlers for action buttons
  const actionButtons = mappingTableBody.querySelectorAll(".icon-btn");
  actionButtons.forEach((btn) => {
    btn.addEventListener("click", (event) => {
      event.stopPropagation();
      const row = btn.closest(".mapping-row");
      const mappingId = row?.getAttribute("data-mapping-id");
      const action = btn.getAttribute("data-action");

      if (mappingId && action) {
        if (action === "edit") {
          editMapping(mappingId);
        } else if (action === "delete") {
          removeMapping(mappingId);
        }
      }
    });
  });
}

function renderStatusBadge(status: FieldMapping["status"]): string {
  const badges: Record<FieldMapping["status"], string> = {
    pending: '<span class="badge badge--pending">Pending</span>',
    mapped: '<span class="badge badge--mapped">Mapped</span>',
    verified: '<span class="badge badge--verified">Verified</span>',
    error: '<span class="badge badge--error">Error</span>',
  };
  return badges[status] || "";
}

function getFilteredMappings(): FieldMapping[] {
  let filtered = [...currentState.mappings];

  // Apply search filter
  if (searchInput?.value) {
    const searchTerm = searchInput.value.toLowerCase();
    filtered = filtered.filter(
      (m) =>
        m.sourceField.toLowerCase().includes(searchTerm) ||
        m.targetField.toLowerCase().includes(searchTerm),
    );
  }

  // Apply status filter
  if (filterSelect?.value && filterSelect.value !== "all") {
    const statusFilter = filterSelect.value;
    filtered = filtered.filter((m) => m.status === statusFilter);
  }

  return filtered;
}

// Event handlers
function handleAddMapping(): void {
  const newMapping: FieldMapping = {
    id: generateId(),
    sourceField: "",
    targetField: "",
    status: "pending",
    lastUpdated: new Date().toISOString(),
  };

  currentState.mappings.push(newMapping);
  currentState.isDirty = true;
  selectMapping(newMapping.id);
  renderMappingTable();
  showStatusMessage("New mapping created. Select source and target fields.", "info");
}

function handleValidateMappings(): void {
  const errors: Record<string, string> = {};
  let validCount = 0;
  let errorCount = 0;

  currentState.mappings = currentState.mappings.map((mapping) => {
    if (!mapping.sourceField || !mapping.targetField) {
      errors[mapping.id] = "Both source and target fields are required";
      errorCount++;
      return { ...mapping, status: "error" as const };
    }

    // Check for duplicate mappings
    const duplicates = currentState.mappings.filter(
      (m) =>
        m.id !== mapping.id &&
        (m.sourceField === mapping.sourceField || m.targetField === mapping.targetField),
    );

    if (duplicates.length > 0) {
      errors[mapping.id] = "Duplicate field mapping detected";
      errorCount++;
      return { ...mapping, status: "error" as const };
    }

    validCount++;
    return { ...mapping, status: "verified" as const, lastUpdated: new Date().toISOString() };
  });

  currentState.validationErrors = errors;
  currentState.isDirty = Object.keys(errors).length > 0;

  renderMappingTable();
  renderSourceFields();
  renderTargetFields();

  if (errorCount === 0) {
    showStatusMessage(`All ${validCount} mappings validated successfully!`, "success");
  } else {
    showStatusMessage(`${errorCount} error(s) found. Please fix before proceeding.`, "error");
  }
}

function handleClearMappings(): void {
  if (confirm("Are you sure you want to clear all mappings? This action cannot be undone.")) {
    currentState.mappings = [];
    currentState.validationErrors = {};
    currentState.isDirty = true;
    currentState.selectedMappingId = null;
    renderMappingTable();
    renderSourceFields();
    renderTargetFields();
    showStatusMessage("All mappings cleared.", "info");
  }
}

function handleExportMappings(): void {
  const exportData = {
    exportedAt: new Date().toISOString(),
    totalMappings: currentState.mappings.length,
    verifiedMappings: currentState.mappings.filter((m) => m.status === "verified").length,
    mappings: currentState.mappings,
  };

  const jsonString = JSON.stringify(exportData, null, 2);
  const blob = new Blob([jsonString], { type: "application/json" });
  const url = URL.createObjectURL(blob);

  const link = document.createElement("a");
  link.href = url;
  link.download = `field-mappings-${new Date().toISOString().split("T")[0]}.json`;
  document.body.appendChild(link);
  link.click();
  document.body.removeChild(link);
  URL.revokeObjectURL(url);

  showStatusMessage("Mappings exported successfully!", "success");
}

function handleSearchFilter(): void {
  renderMappingTable();
}

function handleStatusFilter(): void {
  renderMappingTable();
}

function handleFieldSelect(element: Element, side: "source" | "target"): void {
  const field = element.getAttribute("data-field");
  if (!field) return;

  if (!currentState.selectedMappingId) {
    // Create a new mapping with this field
    const newMapping: FieldMapping = {
      id: generateId(),
      sourceField: side === "source" ? field : "",
      targetField: side === "target" ? field : "",
      status: "pending",
      lastUpdated: new Date().toISOString(),
    };
    currentState.mappings.push(newMapping);
    currentState.isDirty = true;
    selectMapping(newMapping.id);
  } else {
    // Update the selected mapping
    const mapping = currentState.mappings.find((m) => m.id === currentState.selectedMappingId);
    if (mapping) {
      if (side === "source") {
        mapping.sourceField = field;
      } else {
        mapping.targetField = field;
      }
      mapping.status = "mapped";
      mapping.lastUpdated = new Date().toISOString();
      currentState.isDirty = true;
    }
  }

  renderMappingTable();
  renderSourceFields();
  renderTargetFields();
}

function selectMapping(mappingId: string): void {
  currentState.selectedMappingId = mappingId;
  renderMappingTable();
}

function editMapping(mappingId: string): void {
  const mapping = currentState.mappings.find((m) => m.id === mappingId);
  if (!mapping) return;

  selectMapping(mappingId);

  // Open edit dialog or inline editor
  const transformation = prompt("Enter transformation (optional):", mapping.transformation || "");
  if (transformation !== null) {
    mapping.transformation = transformation || null;
    mapping.status = "mapped";
    mapping.lastUpdated = new Date().toISOString();
    currentState.isDirty = true;
    renderMappingTable();
  }
}

function removeMapping(mappingId: string): void {
  if (confirm("Are you sure you want to remove this mapping?")) {
    currentState.mappings = currentState.mappings.filter((m) => m.id !== mappingId);
    if (currentState.selectedMappingId === mappingId) {
      currentState.selectedMappingId = null;
    }
    currentState.isDirty = true;
    renderMappingTable();
    renderSourceFields();
    renderTargetFields();
    showStatusMessage("Mapping removed.", "info");
  }
}

// Helper functions
function isFieldMapped(field: string, side: "source" | "target"): boolean {
  return currentState.mappings.some((m) => {
    if (side === "source") {
      return m.sourceField === field;
    }
    return m.targetField === field;
  });
}

function generateId(): string {
  return `mapping_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
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

function showStatusMessage(message: string, type: "info" | "success" | "error" | "warning"): void {
  if (!statusMessage) {
    console.log(`[${type.toUpperCase()}] ${message}`);
    return;
  }

  const typeClasses = {
    info: "status--info",
    success: "status--success",
    error: "status--error",
    warning: "status--warning",
  };

  const messageEl = statusMessage;
  messageEl.className = `status-message ${typeClasses[type]}`;
  messageEl.textContent = message;
  messageEl.style.display = "block";

  // Auto-hide after 5 seconds for non-error messages
  if (type !== "error") {
    setTimeout(() => {
      messageEl.style.display = "none";
    }, 5000);
  }
}

// Loading functions
async function loadSourceFieldsFromPlan(): Promise<void> {
  // Placeholder - implement based on your data source
  showStatusMessage("Loading source fields...", "info");
  // Example: fetch from API or extract from ActionPlan
}

async function loadTargetFieldsFromPlan(): Promise<void> {
  // Placeholder - implement based on your data source
  showStatusMessage("Loading target fields...", "info");
  // Example: fetch from API or extract from ActionPlan
}

// Public API
export function setSourceFields(fields: string[]): void {
  currentState.sourceFields = fields;
  renderSourceFields();
}

export function setTargetFields(fields: string[]): void {
  currentState.targetFields = fields;
  renderTargetFields();
}

export function setMappings(mappings: FieldMapping[]): void {
  currentState.mappings = mappings;
  currentState.isDirty = false;
  renderMappingTable();
  renderSourceFields();
  renderTargetFields();
}

export function getMappings(): FieldMapping[] {
  return [...currentState.mappings];
}

export function isDirty(): boolean {
  return currentState.isDirty;
}

export function resetState(): void {
  currentState = {
    mappings: [],
    selectedMappingId: null,
    sourceFields: [],
    targetFields: [],
    validationErrors: {},
    isDirty: false,
  };
  renderMappingTable();
  renderSourceFields();
  renderTargetFields();
}

export function destroy(): void {
  // Clean up event listeners
  if (addMappingButton) {
    addMappingButton.removeEventListener("click", handleAddMapping);
  }
  if (validateMappingButton) {
    validateMappingButton.removeEventListener("click", handleValidateMappings);
  }
  if (clearMappingsButton) {
    clearMappingsButton.removeEventListener("click", handleClearMappings);
  }
  if (exportMappingsButton) {
    exportMappingsButton.removeEventListener("click", handleExportMappings);
  }
  if (searchInput) {
    searchInput.removeEventListener("input", handleSearchFilter);
  }
  if (filterSelect) {
    filterSelect.removeEventListener("change", handleStatusFilter);
  }

  // Reset DOM references
  sourceFieldsContainer = null;
  targetFieldsContainer = null;
  mappingTableBody = null;
  addMappingButton = null;
  validateMappingButton = null;
  clearMappingsButton = null;
  exportMappingsButton = null;
  searchInput = null;
  filterSelect = null;
  statusMessage = null;

  resetState();
}
