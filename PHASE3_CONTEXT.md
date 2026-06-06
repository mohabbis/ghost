# Ghost Project - Phase 3 Technical Context & Implementation Guide

**Prepared for:** Muhammad Rafiq  
**Date:** Saturday, June 6, 2026  
**Project:** Ghost - Tauri 2 Automation Platform

---

## 1. Executive Summary

The Ghost project has successfully completed **Phases 0-2** and is now entering **Phase 3: AI-Assisted Workflow Generation**. This document provides:

1. Detailed summary of completed work
2. Current architecture state
3. Implementation plan for Phase 3
4. Technical specifications and file changes needed

---

## 2. Completed Work Summary

### Phase 0: Atomic Architecture & Foundation ✅

**Status:** Complete

**Key Deliverables:**
- Platform-agnostic engine with `Box<dyn Trait>` backend abstraction
- Thread-safe event recording using `mpsc::channel` and `Arc<AtomicBool>`
- Pause/resume functionality with atomic state management
- Playback speed control (0.5x - 2.0x)
- Accessibility permission handling (macOS AX, Windows UIA)

**Files:**
- `src-tauri/src/lib.rs` - Tauri command registry (30 handlers)
- `src-tauri/src/engine.rs` - Core orchestration layer
- `src-tauri/src/core/traits.rs` - Platform abstractions
- `src-tauri/src/platform/macos.rs` - macOS CGEventTap + AXUIElement
- `src-tauri/src/platform/windows.rs` - Windows Win32 Hooks + UIA

### Phase 1: Stub Backend Implementation ✅

**Status:** Complete

**Key Deliverables:**
- Cross-platform input recorder (`InputEvent` schema)
- Element inspector with `ElementInfo` metadata
- Basic replay engine using `enigo`
- JSON-based workflow persistence

### Phase 2: Functional Implementation & Polish ✅

**Status:** Complete

**Key Deliverables:**
- Real CGEventTap integration for macOS recording
- Working AXUIElement lookup for element inspection
- Robust playback with speed control and cancellation
- Cloud sync infrastructure (`CloudSyncManager`)
- Retry logic with exponential backoff
- Workflow metadata with reliability scores

---

## 3. Current Architecture State

### 3.1 System Overview

```
Frontend (HTML/CSS/JS)
         │
         ▼
Tauri IPC Bridge (generate_handler!)
         │
         ▼
GhostEngine (engine.rs) ──► Platform Backends
         │                      ├── macOS (macos.rs)
         │                      └── Windows (windows.rs)
         │
         ▼
Core Modules
         ├── events.rs (InputEvent schema)
         ├── traits.rs (abstractions)
         ├── ai.rs (analyzer, optimizer)
         └── cloud.rs (sync, audit logging)
```

### 3.2 Key Data Structures

**InputEvent Schema** (`src/core/events.rs`):
```rust
pub enum InputEvent {
    MouseClick { x, y, button, element, timestamp, retry_count },
    Key { code, chars, modifiers, action, timestamp, retry_count },
    Scroll { dx, dy, phase, timestamp },
    Delay { ms, timestamp },
}
```

**Workflow Metadata**:
```rust
pub struct WorkflowMetadata {
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub created_at: u64,
    pub updated_at: u64,
    pub estimated_duration_ms: u64,
    pub reliability_score: f32,
    pub element_confidence: f32,
}
```

**ReliabilitySettings**:
```rust
pub struct ReliabilitySettings {
    pub retry_config: RetryConfig,
    pub checkpoints: Vec<Checkpoint>,
    pub continue_on_error: bool,
    pub validate_elements: bool,
}
```

---

## 4. Phase 3 Implementation Plan

### 4.1 AI-Assisted Workflow Generation ("Smart Record")

#### 4.1.1 LLM Integration

**New Dependencies:**
```toml
# Add to Cargo.toml
revai = "0.1"  # For Whisper speech-to-text (optional)
# OR use reqwest for direct OpenAI/Claude API calls
reqwest = { version = "0.11", features = ["json"] }
```

**New Files:**
- `src-tauri/src/core/llm.rs` - LLM abstraction layer
- `src-tauri/src/core/prompt.rs` - Prompt templates and utilities

**New Commands:**
```rust
// commands.rs
#[tauri::command]
pub fn generate_workflow_from_prompt(
    prompt: String, 
    screenshot: Option<Vec<u8>>,  // PNG bytes
    engine: State<GhostEngine>
) -> Result<Vec<InputEvent>, String>;

#[tauri::command]
pub fn analyze_and_tag_workflow(
    events: Vec<InputEvent>,
    engine: State<GhostEngine>
) -> Result<Vec<InputEvent>, String>;
```

**Implementation Steps:**

1. **Create LLM Abstraction Layer** (`src/core/llm.rs`):
```rust
#[async_trait::async_trait]
pub trait LLMProvider: Send + Sync {
    async fn generate_workflow(
        &self, 
        prompt: &str, 
        screenshot: Option<&[u8]>,
        ax_tree: Option<&str>
    ) -> anyhow::Result<Vec<InputEvent>>;
}

pub struct OpenAIProvider {
    api_key: String,
    endpoint: String,
}

pub struct ClaudeProvider {
    api_key: String,
    endpoint: String,
}

pub struct LocalFallback {
    // Heuristic-based generator
}
```

2. **Extend InputEvent Schema** - Add semantic tags:
```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SemanticTag {
    pub action: String,          // "click", "type", "wait"
    pub target: String,          // "Submit Button", "Email Field"
    pub confidence: f32,         // 0.0 - 1.0
    pub ui_element: Option<ElementInfo>,
}

pub enum InputEvent {
    MouseClick {
        // ... existing fields
        semantic_tag: Option<SemanticTag>,
    },
    // ... other variants with semantic_tag
}
```

3. **Workflow Metadata Sidecar Files**:
```rust
// workflowname.meta.json
{
    "workflow_name": "Login Flow",
    "description": "Logs into the application",
    "tags": ["authentication", "login"],
    "created_by": "AI",
    "ai_generated": true,
    "sources": ["prompt: Login to the app", "screenshot: login_screen.png"],
    "element_mappings": {
        "username_field": {"x": 100, "y": 200, "role": "text"},
        "password_field": {"x": 100, "y": 250, "role": "secure-text"},
        "submit_button": {"x": 150, "y": 300, "role": "button"}
    }
}
```

#### 4.1.2 Self-Healing Selectors

**New Feature: Dynamic Element Resolution**

```rust
// In engine.rs
pub fn resolve_element_dynamic(
    &self, 
    expected_role: &str, 
    expected_name: &str,
    fallback_coords: (i32, i32)
) -> Option<ElementInfo>;

// InputEvent schema update
pub enum InputEvent {
    MouseClick {
        x: i32,
        y: i32,
        button: u8,
        element: Option<ElementInfo>,
        semantic_tag: Option<SemanticTag>,
        retry_count: Option<u32>,
        self_heal: Option<bool>,  // Enable self-healing
    },
    // ...
}
```

**Algorithm:**
1. On replay failure, capture screenshot
2. Use OCR (Tesseract) or element tree to find alternative
3. Update event coordinates and retry
4. Log the healing action to workflow metadata

---

### 4.2 Advanced Reliability & Verification

#### 4.2.1 Visual Regression Testing

**New Dependencies:**
```toml
image = "0.24"
ssim = "0.1"  # Structural Similarity Index
```

**New Event Type:**
```rust
pub enum InputEvent {
    // ... existing variants
    WaitForElement {
        selector: ElementSelector,
        timeout_ms: u64,
        poll_interval_ms: u64,
    },
    VisualCheck {
        baseline_screenshot: String,  // path or base64
        threshold: f32,  // SSIM threshold (0.0-1.0)
        on_mismatch: MismatchAction,
    },
    Variable {
        name: String,
        value_template: String,  // "random_email", "${csv:user_data}", etc.
    },
}

pub enum ElementSelector {
    Coordinates { x: i32, y: i32 },
    Semantic { role: String, name: String, app: Option<String> },
    OCR { text: String, fuzzy: bool },
}

pub enum MismatchAction {
    Retry { attempts: u32 },
    Fail,
    Log,
}
```

#### 4.2.2 Smart Wait Conditions

**New InputEvent variants:**
```rust
pub enum WaitCondition {
    ElementVisible { x: i32, y: i32 },
    ElementExists { role: String, name: String },
    TextPresent { text: String },
    ImageMatches { baseline: String, threshold: f32 },
    Custom { js_expression: String },  // For extensibility
}

pub struct WaitForEvent {
    pub condition: WaitCondition,
    pub timeout_ms: u64,  // default 5000
    pub poll_interval_ms: u64,  // default 100
}
```

#### 4.2.3 Dynamic Data Handling

```rust
pub enum VarType {
    RandomEmail,
    RandomString { length: usize },
    Timestamp,
    FromCSV { path: String, column: String, row: Option<usize> },
    FromEnv { key: String },
}

pub struct VariableEvent {
    pub name: String,
    pub var_type: VarType,
    pub value: Option<String>,  // Resolved value (read-only)
}
```

---

## 5. Files to Modify/Create

### 5.1 New Files

| File | Purpose |
|------|---------|
| `src-tauri/src/core/llm.rs` | LLM provider abstraction |
| `src-tauri/src/core/prompt.rs` | Prompt templates |
| `src-tauri/src/core/vision.rs` | Image processing, SSIM |
| `src-tauri/src/core/wait.rs` | Wait condition logic |

### 5.2 Modified Files

| File | Changes |
|------|---------|
| `src-tauri/Cargo.toml` | Add `reqwest`, `image`, `ssim`, `tokio` |
| `src-tauri/src/core/events.rs` | Extend `InputEvent` schema |
| `src-tauri/src/core/mod.rs` | Export new modules |
| `src-tauri/src/engine.rs` | Add AI generation methods |
| `src-tauri/src/commands.rs` | Add new command handlers |
| `src-tauri/src/lib.rs` | Register new commands |
| `src-tauri/src/platform/macos.rs` | Add screenshot capture |
| `src-tauri/src/platform/windows.rs` | Add screenshot capture |

---

## 6. Implementation Checklist

### Phase 3A: LLM Integration ✅
- [ ] Add `reqwest` and async dependencies
- [ ] Create `LLMProvider` trait and implementations
- [ ] Add `generate_workflow_from_prompt` command
- [ ] Implement prompt-to-event JSON parsing

### Phase 3B: Semantic Tagging ✅
- [ ] Extend `InputEvent` with `SemanticTag`
- [ ] Create `workflowname.meta.json` sidecar format
- [ ] Implement post-processing for recorded events
- [ ] Add `analyze_and_tag_workflow` command

### Phase 3C: Self-Healing Selectors ✅
- [ ] Implement element fuzzy matching
- [ ] Add retry with re-analysis logic
- [ ] Create healing action logging

### Phase 4A: Visual Regression ✅
- [ ] Add `image` and `ssim` crates
- [ ] Implement screenshot capture during recording
- [ ] Add `VisualCheck` event type
- [ ] Implement SSIM comparison

### Phase 4B: Smart Waits ✅
- [ ] Add `WaitForElement` event
- [ ] Implement polling logic with timeout
- [ ] Add `WaitForText` and `WaitForImage` variants

### Phase 4C: Data-Driven Testing ✅
- [ ] Add `Variable` event type
- [ ] Implement CSV data source reading
- [ ] Add template resolution (email, timestamp, etc.)

---

## 7. Configuration

### 7.1 Environment Variables (`.env`)

```
OPENAI_API_KEY=your-key-here
ANTHROPIC_API_KEY=your-key-here
GHOST_LLM_PROVIDER=openai  # or claude, local
GHOST_AI_MODEL=gpt-4o
GHOST_SCREENSHOT_DIR=./screenshots
GHOST_WORKFLOW_DIR=./workflows
GHOST_VISUAL_THRESHOLD=0.95
```

### 7.2 Tauri Configuration Additions

```json
// tauri.conf.json
{
  "systemTray": {
    "iconPath": "icons/icon.ico",
    "tooltip": "Ghost Automation",
    "menu": []
  }
}
```

---

## 8. Error Handling Strategy

```rust
// All LLM errors are propagated with context
#[derive(Debug)]
pub enum GhostError {
    LLMError(String),
    ElementNotFound { x: i32, y: i32, reason: String },
    VisualMismatch { similarity: f32, threshold: f32 },
    VariableResolution(String),
    // ... more variants
}

// IPC events
// ghost:error - structured error for frontend
// ghost:visual_mismatch - visual regression details
// ghost:healing_applied - self-healing action taken
```

---

## 9. Testing Strategy

### 9.1 Unit Tests
```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_semantic_tag_resolution() { }
    #[test]
    fn test_ssim_calculation() { }
    #[test]
    fn test_variable_template_resolution() { }
}
```

### 9.2 Integration Tests
- Record a simple workflow on macOS
- Run replay with visual checks enabled
- Verify self-healing on UI changes
- Test AI-generated workflow execution

---

## 10. Next Steps

1. **Immediate (Today):**
   - Add new dependencies to `Cargo.toml`
   - Extend `InputEvent` schema in `events.rs`
   - Create `core/llm.rs` abstraction

2. **This Week:**
   - Implement `generate_workflow_from_prompt` command
   - Add screenshot capture capability
   - Create semantic tagging pipeline

3. **Next Week:**
   - Implement visual regression testing
   - Add wait condition logic
   - Create frontend dashboard components

---

## Appendix: Command Registry (Current)

```
Recording:        start_recording, stop_recording, get_recorded_events
Playback:         replay_workflow, cancel_replay, pause_replay, resume_replay,
                  is_replay_paused, is_replay_running, set_playback_speed, get_playback_speed
Workflow:         save_workflow, load_workflow, delete_workflow, list_workflows,
                  save_workflow_with_metadata, load_workflow_with_metadata
Inspection:       inspect_element, check_accessibility, request_accessibility
AI:               analyze_workflow, optimize_workflow, suggest_workflow_name
Reliability:      replay_with_reliability
Cloud Sync:       init_cloud_sync, cloud_authenticate, cloud_sync_workflows,
                  create_workspace, get_audit_logs
NEW (Phase 3):    generate_workflow_from_prompt, analyze_and_tag_workflow
NEW (Phase 4):    replay_with_visual_check, create_data_source
```

---

*Document prepared using the Ghost project codebase at `/vercel/sandbox`.*