"use client";

import { useMemo, useState } from "react";
import { useRouter } from "next/navigation";
import { ArrowDown, ArrowUp, Plus, ShieldAlert, Trash2 } from "lucide-react";
import { classifyStep } from "@ghost/core/classifier";
import { EDITABLE_STEP_TYPES } from "@ghost/core/schema/step";
import type {
  Compensation,
  CompensationAction,
  EditableStepType,
  Selector,
  Verification,
  WorkflowStep,
} from "@ghost/core/schema/step";
import { Button } from "@/components/ui/button";
import { Card, CardBody } from "@/components/ui/card";
import { FieldLabel, Input, Select } from "@/components/ui/input";

/**
 * The typed step editor.
 *
 * Two rules this UI carries, both deliberate:
 *
 *  1. Every step shows its approval verdict live, and there is no control to
 *     turn one off. `classifyStep` is deterministic and the author is not a
 *     party to its decision — the point of showing it here is that nobody
 *     discovers mid-run that their workflow stops, not that they can argue
 *     with it.
 *  2. Only step types the worker can actually execute are offered. `apiCall`
 *     and `sendEmail` parse and gate but perform nothing, so authoring one
 *     would produce a run that skips its most important step and reports
 *     success. `EDITABLE_STEP_TYPES` is enforced server-side too.
 */

const TYPE_LABELS: Record<EditableStepType, string> = {
  navigate: "Navigate",
  click: "Click",
  fill: "Fill",
  select: "Select option",
  waitFor: "Wait for",
  extract: "Extract",
  verify: "Verify",
  approval: "Approval gate",
};

const SELECTOR_TYPES = new Set<string>(["click", "fill", "select", "extract"]);

/**
 * Which editable step types can even have an undo. Matches
 * `hasSideEffect` in `@ghost/core/compensate`: navigate/waitFor/extract/verify/
 * approval don't mutate anything outside the browser's own view, so there is
 * nothing for a compensation to reverse. `apiCall`/`sendEmail` also have side
 * effects but aren't offered by this editor at all (see the file doc comment),
 * so click/fill/select are the only step types this applies to today.
 */
const COMPENSATABLE_STEP_TYPES = new Set<string>(["click", "fill", "select"]);

type CompensationActionType = CompensationAction["type"];

const COMPENSATION_ACTION_LABELS: Record<CompensationActionType, string> = {
  navigate: "Navigate",
  click: "Click",
  fill: "Fill",
  select: "Select option",
  waitFor: "Wait for",
};
const COMPENSATION_ACTION_TYPES = Object.keys(
  COMPENSATION_ACTION_LABELS,
) as CompensationActionType[];

let seq = 0;
const nextId = (type: string) => `${type}-${Date.now().toString(36)}-${seq++}`;

function blankCompensationAction(type: CompensationActionType): CompensationAction {
  switch (type) {
    case "navigate":
      return { type, url: "" };
    case "click":
      return { type, selector: {} };
    case "fill":
      return { type, selector: {}, value: "" };
    case "select":
      return { type, selector: {}, value: "" };
    case "waitFor":
      return { type, selector: {} };
  }
}

function blankCompensation(): Compensation {
  return { description: "", actions: [blankCompensationAction("click")] };
}

function blankStep(type: EditableStepType): WorkflowStep {
  const id = nextId(type);
  switch (type) {
    case "navigate":
      return { id, type, url: "" };
    case "click":
      return { id, type, selector: {} };
    case "fill":
      return { id, type, selector: {}, value: "", sensitive: false };
    case "select":
      return { id, type, selector: {}, value: "" };
    case "waitFor":
      return { id, type, selector: {} };
    case "extract":
      return { id, type, selector: {}, name: "" };
    case "verify":
      return { id, type, assertion: { kind: "textPresent", expected: "" } };
    case "approval":
      return { id, type, reason: "" };
  }
}

/** Verdict for a step that may still be half-written; never throws. */
function verdictOf(step: WorkflowStep): { sensitive: boolean; reason?: string } {
  try {
    return classifyStep(step);
  } catch {
    return { sensitive: false };
  }
}

export interface WorkflowEditorProps {
  /** Present when editing; absent when creating. */
  workflowId?: string;
  initialName?: string;
  initialDescription?: string;
  initialSteps?: WorkflowStep[];
  /** Latest published version, shown so the author knows what they are basing on. */
  currentVersion?: number;
  /** Set when these initial steps came from a compiled Recording proposal —
   * on create, links the new workflow back to it (see `POST /api/workflows`)
   * so the recording is marked COMPILED instead of staying claimable twice. */
  recordingId?: string;
}

export function WorkflowEditor({
  workflowId,
  initialName = "",
  initialDescription = "",
  initialSteps,
  currentVersion,
  recordingId,
}: WorkflowEditorProps) {
  const router = useRouter();
  const [name, setName] = useState(initialName);
  const [description, setDescription] = useState(initialDescription);
  const [steps, setSteps] = useState<WorkflowStep[]>(
    initialSteps?.length ? initialSteps : [blankStep("navigate")],
  );
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const gated = useMemo(() => steps.filter((s) => verdictOf(s).sensitive).length, [steps]);

  function update(index: number, patch: Partial<WorkflowStep>) {
    setSteps((prev) =>
      prev.map((s, i) => (i === index ? ({ ...s, ...patch } as WorkflowStep) : s)),
    );
  }

  function updateSelector(index: number, patch: Partial<Selector>) {
    setSteps((prev) =>
      prev.map((s, i) => {
        if (i !== index || !("selector" in s)) return s;
        const selector = { ...(s.selector ?? {}), ...patch };
        // Drop emptied keys so an unused field never reaches the resolver as "".
        for (const k of Object.keys(selector) as (keyof Selector)[]) {
          if (!selector[k]) delete selector[k];
        }
        return { ...s, selector } as WorkflowStep;
      }),
    );
  }

  function changeType(index: number, type: EditableStepType) {
    setSteps((prev) =>
      prev.map((s, i) => (i === index ? { ...blankStep(type), label: s.label } : s)),
    );
  }

  /**
   * Every undo-authoring control goes through this one updater, the same way
   * every step field goes through `update`/`updateSelector` above. Absent
   * (`undefined`) is a real, meaningful state — "no known way to undo this" —
   * not just an empty draft, so callers pass it back explicitly rather than an
   * empty object.
   */
  function updateCompensate(
    index: number,
    updater: (current: Compensation | undefined) => Compensation | undefined,
  ) {
    setSteps((prev) =>
      prev.map((s, i) => (i === index ? ({ ...s, compensate: updater(s.compensate) } as WorkflowStep) : s)),
    );
  }

  function move(index: number, delta: number) {
    setSteps((prev) => {
      const to = index + delta;
      if (to < 0 || to >= prev.length) return prev;
      const next = [...prev];
      [next[index], next[to]] = [next[to]!, next[index]!];
      return next;
    });
  }

  async function save() {
    setSaving(true);
    setError(null);
    try {
      const res = workflowId
        ? await fetch(`/api/workflows/${workflowId}/versions`, {
            method: "POST",
            headers: { "content-type": "application/json" },
            body: JSON.stringify({ steps }),
          })
        : await fetch("/api/workflows", {
            method: "POST",
            headers: { "content-type": "application/json" },
            body: JSON.stringify({ name, description, steps, recordingId }),
          });
      const body = await res.json().catch(() => ({}));
      if (!res.ok) throw new Error(body?.error ?? `save failed (${res.status})`);
      router.push("/workflows");
      router.refresh();
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="space-y-5">
      <Card>
        <CardBody className="space-y-4">
          <div>
            <FieldLabel htmlFor="wf-name">Name</FieldLabel>
            <Input
              id="wf-name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="Submit weekly order"
              disabled={Boolean(workflowId)}
            />
          </div>
          <div>
            <FieldLabel htmlFor="wf-desc">Description</FieldLabel>
            <Input
              id="wf-desc"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="What this workflow does, in one line"
              disabled={Boolean(workflowId)}
            />
          </div>
          {currentVersion !== undefined && (
            <p className="text-xs text-[var(--color-muted)]">
              Editing version {currentVersion}. Saving publishes version {currentVersion + 1};
              runs already in flight keep executing the version they started with.
            </p>
          )}
        </CardBody>
      </Card>

      <div className="space-y-3">
        {steps.map((step, i) => {
          const verdict = verdictOf(step);
          return (
            <Card key={step.id}>
              <CardBody className="space-y-3">
                <div className="flex items-center gap-2">
                  <span className="w-6 shrink-0 text-xs text-[var(--color-muted)]">{i + 1}</span>
                  <Select
                    aria-label={`Step ${i + 1} type`}
                    value={step.type}
                    onChange={(e) => changeType(i, e.target.value as EditableStepType)}
                    className="w-44"
                  >
                    {EDITABLE_STEP_TYPES.map((t) => (
                      <option key={t} value={t}>
                        {TYPE_LABELS[t]}
                      </option>
                    ))}
                  </Select>
                  <Input
                    aria-label={`Step ${i + 1} label`}
                    value={step.label ?? ""}
                    onChange={(e) => update(i, { label: e.target.value || undefined })}
                    placeholder="Label (optional)"
                  />
                  <Button
                    variant="ghost"
                    size="sm"
                    aria-label={`Move step ${i + 1} up`}
                    onClick={() => move(i, -1)}
                    disabled={i === 0}
                  >
                    <ArrowUp className="h-4 w-4" />
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    aria-label={`Move step ${i + 1} down`}
                    onClick={() => move(i, 1)}
                    disabled={i === steps.length - 1}
                  >
                    <ArrowDown className="h-4 w-4" />
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    aria-label={`Delete step ${i + 1}`}
                    onClick={() => setSteps((prev) => prev.filter((_, j) => j !== i))}
                    disabled={steps.length === 1}
                  >
                    <Trash2 className="h-4 w-4" />
                  </Button>
                </div>

                <StepFields
                  step={step}
                  index={i}
                  update={update}
                  updateSelector={updateSelector}
                  updateCompensate={updateCompensate}
                />

                {verdict.sensitive && (
                  <div
                    data-testid={`gate-${i}`}
                    className="flex items-start gap-2 rounded-lg border border-[var(--color-warning,#a16207)]/40 bg-[var(--color-warning,#a16207)]/10 px-3 py-2 text-xs"
                  >
                    <ShieldAlert className="mt-0.5 h-3.5 w-3.5 shrink-0" />
                    <span>
                      <strong>Stops for approval.</strong> {verdict.reason}
                    </span>
                  </div>
                )}
              </CardBody>
            </Card>
          );
        })}
      </div>

      <div className="flex flex-wrap items-center gap-2">
        {EDITABLE_STEP_TYPES.map((t) => (
          <Button
            key={t}
            variant="secondary"
            size="sm"
            onClick={() => setSteps((prev) => [...prev, blankStep(t)])}
          >
            <Plus className="h-3.5 w-3.5" />
            {TYPE_LABELS[t]}
          </Button>
        ))}
      </div>

      {error && (
        <p role="alert" className="text-sm text-[var(--color-danger)]">
          {error}
        </p>
      )}

      <div className="flex items-center gap-3">
        <Button onClick={save} disabled={saving || (!workflowId && !name.trim())}>
          {saving ? "Saving…" : workflowId ? "Publish new version" : "Create workflow"}
        </Button>
        <span className="text-xs text-[var(--color-muted)]">
          {steps.length} step{steps.length === 1 ? "" : "s"} ·{" "}
          {gated === 0
            ? "none stop for approval"
            : `${gated} step${gated === 1 ? "" : "s"} stop for approval`}
        </span>
      </div>
    </div>
  );
}

/**
 * `ariaPrefix`/`onChange` rather than `(index, updateSelector)` so this one
 * component serves both a step's own selector and a selector nested inside
 * one of its undo actions — the two have different addressing (a step index
 * vs. a step index *and* an action index), and a plain callback sidesteps
 * that instead of this component needing to know which case it's in.
 */
function SelectorFields({
  selector,
  ariaPrefix,
  onChange,
}: {
  selector: Selector;
  ariaPrefix: string;
  onChange: (patch: Partial<Selector>) => void;
}) {
  return (
    <div>
      <FieldLabel>Target</FieldLabel>
      <div className="grid grid-cols-2 gap-2 sm:grid-cols-3">
        {(["role", "name", "testId", "text", "css"] as const).map((key) => (
          <Input
            key={key}
            aria-label={`${ariaPrefix} ${key}`}
            value={selector[key] ?? ""}
            onChange={(e) => onChange({ [key]: e.target.value })}
            placeholder={key === "css" ? "css (last resort)" : key}
          />
        ))}
      </div>
    </div>
  );
}

function StepFields({
  step,
  index,
  update,
  updateSelector,
  updateCompensate,
}: {
  step: WorkflowStep;
  index: number;
  update: (i: number, patch: Partial<WorkflowStep>) => void;
  updateSelector: (i: number, patch: Partial<Selector>) => void;
  updateCompensate: (
    i: number,
    updater: (current: Compensation | undefined) => Compensation | undefined,
  ) => void;
}) {
  const selector = "selector" in step ? (step.selector ?? {}) : undefined;

  return (
    <div className="space-y-3">
      {step.type === "navigate" && (
        <div>
          <FieldLabel>URL</FieldLabel>
          <Input
            aria-label={`Step ${index + 1} url`}
            value={step.url}
            onChange={(e) => update(index, { url: e.target.value } as Partial<WorkflowStep>)}
            placeholder="https://example.com/orders"
          />
        </div>
      )}

      {selector && SELECTOR_TYPES.has(step.type) && (
        <SelectorFields
          selector={selector}
          ariaPrefix={`Step ${index + 1} selector`}
          onChange={(patch) => updateSelector(index, patch)}
        />
      )}

      {(step.type === "fill" || step.type === "select") && (
        <div>
          <FieldLabel>Value</FieldLabel>
          <Input
            aria-label={`Step ${index + 1} value`}
            value={step.value}
            onChange={(e) => update(index, { value: e.target.value } as Partial<WorkflowStep>)}
            placeholder={step.type === "fill" ? "Ada Lovelace" : "Option label"}
          />
        </div>
      )}

      {step.type === "fill" && (
        <label className="flex items-center gap-2 text-xs text-[var(--color-muted)]">
          <input
            type="checkbox"
            checked={step.sensitive}
            onChange={(e) => update(index, { sensitive: e.target.checked } as Partial<WorkflowStep>)}
          />
          {/* Only ever *adds* a gate — the classifier ORs this with its own
              field-word list, so ticking it cannot remove an approval. */}
          Secret or high-risk input (password, OTP, card) — always stops for approval
        </label>
      )}

      {step.type === "extract" && (
        <div>
          <FieldLabel>Store as</FieldLabel>
          <Input
            aria-label={`Step ${index + 1} name`}
            value={step.name}
            onChange={(e) => update(index, { name: e.target.value } as Partial<WorkflowStep>)}
            placeholder="orderNumber"
          />
        </div>
      )}

      {step.type === "waitFor" && (
        <div className="grid gap-2 sm:grid-cols-2">
          <SelectorFields
            selector={step.selector ?? {}}
            ariaPrefix={`Step ${index + 1} selector`}
            onChange={(patch) => updateSelector(index, patch)}
          />
          <div>
            <FieldLabel>…or milliseconds</FieldLabel>
            <Input
              aria-label={`Step ${index + 1} ms`}
              type="number"
              min={1}
              value={step.ms ?? ""}
              onChange={(e) =>
                update(index, {
                  ms: e.target.value ? Number(e.target.value) : undefined,
                } as Partial<WorkflowStep>)
              }
              placeholder="1000"
            />
          </div>
        </div>
      )}

      {step.type === "verify" && (
        <div className="grid gap-2 sm:grid-cols-2">
          <div>
            <FieldLabel>Assertion</FieldLabel>
            <Select
              aria-label={`Step ${index + 1} assertion kind`}
              value={step.assertion.kind}
              onChange={(e) => {
                const kind = e.target.value as "textPresent" | "url" | "selectorVisible";
                update(index, {
                  assertion:
                    kind === "selectorVisible"
                      ? { kind, selector: {} }
                      : { kind, expected: "" },
                } as Partial<WorkflowStep>);
              }}
            >
              <option value="textPresent">Text is present</option>
              <option value="url">URL matches</option>
              <option value="selectorVisible">Element is visible</option>
            </Select>
          </div>
          {step.assertion.kind !== "selectorVisible" && (
            <div>
              <FieldLabel>Expected</FieldLabel>
              <Input
                aria-label={`Step ${index + 1} expected`}
                value={step.assertion.expected}
                onChange={(e) =>
                  update(index, {
                    assertion: { ...step.assertion, expected: e.target.value },
                  } as Partial<WorkflowStep>)
                }
                placeholder="Order submitted"
              />
            </div>
          )}
        </div>
      )}

      {step.type === "approval" && (
        <div>
          <FieldLabel>Why a human must approve</FieldLabel>
          <Input
            aria-label={`Step ${index + 1} reason`}
            value={step.reason}
            onChange={(e) => update(index, { reason: e.target.value } as Partial<WorkflowStep>)}
            placeholder="Confirm the totals before sending"
          />
        </div>
      )}

      {COMPENSATABLE_STEP_TYPES.has(step.type) && (
        <CompensationFields
          stepIndex={index}
          compensation={step.compensate}
          updateCompensate={updateCompensate}
        />
      )}
    </div>
  );
}

/**
 * Author an optional undo for a step — a `Compensation`: a description shown
 * to whoever approves the reversal, an ordered list of actions, and an
 * optional assertion that the reversal actually took effect. The reversal
 * engine (`compensateRunJob`) already walks the run journal backwards and
 * executes whatever is defined here; this is the only place that plan can be
 * authored, so a step with nothing entered here is honestly reported as
 * irreversible rather than silently skipped.
 */
function CompensationFields({
  stepIndex,
  compensation,
  updateCompensate,
}: {
  stepIndex: number;
  compensation?: Compensation;
  updateCompensate: (
    i: number,
    updater: (current: Compensation | undefined) => Compensation | undefined,
  ) => void;
}) {
  if (!compensation) {
    return (
      <div className="rounded-lg border border-dashed border-[var(--color-border)] p-3">
        <Button
          type="button"
          variant="ghost"
          size="sm"
          onClick={() => updateCompensate(stepIndex, () => blankCompensation())}
        >
          <Plus className="h-3.5 w-3.5" />
          Add undo for this step
        </Button>
        <p className="mt-1 text-xs text-[var(--color-muted)]">
          No undo defined. If someone requests an undo of this run, this step
          will be reported as unable to be reversed rather than skipped.
        </p>
      </div>
    );
  }

  function setActions(actions: CompensationAction[]) {
    updateCompensate(stepIndex, (c) => (c ? { ...c, actions } : c));
  }

  return (
    <div className="space-y-3 rounded-lg border border-[var(--color-border)] p-3">
      <div className="flex items-center justify-between">
        <FieldLabel className="mb-0">Undo this step</FieldLabel>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          onClick={() => updateCompensate(stepIndex, () => undefined)}
        >
          <Trash2 className="h-3.5 w-3.5" />
          Remove undo
        </Button>
      </div>

      <div>
        <FieldLabel>What happens (shown to whoever approves the undo)</FieldLabel>
        <Input
          aria-label={`Step ${stepIndex + 1} undo description`}
          value={compensation.description}
          onChange={(e) =>
            updateCompensate(stepIndex, (c) => (c ? { ...c, description: e.target.value } : c))
          }
          placeholder='Open the order and click "Cancel order"'
        />
      </div>

      <div className="space-y-2">
        {compensation.actions.map((action, j) => (
          <div
            key={j}
            className="space-y-2 rounded-md border border-[var(--color-border)] p-2"
          >
            <div className="flex items-center gap-2">
              <Select
                aria-label={`Step ${stepIndex + 1} undo action ${j + 1} type`}
                value={action.type}
                onChange={(e) =>
                  setActions(
                    compensation.actions.map((a, k) =>
                      k === j
                        ? blankCompensationAction(e.target.value as CompensationActionType)
                        : a,
                    ),
                  )
                }
                className="w-40"
              >
                {COMPENSATION_ACTION_TYPES.map((t) => (
                  <option key={t} value={t}>
                    {COMPENSATION_ACTION_LABELS[t]}
                  </option>
                ))}
              </Select>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                aria-label={`Remove undo action ${j + 1}`}
                onClick={() => setActions(compensation.actions.filter((_, k) => k !== j))}
                disabled={compensation.actions.length === 1}
              >
                <Trash2 className="h-3.5 w-3.5" />
              </Button>
            </div>
            <CompensationActionFields
              action={action}
              stepIndex={stepIndex}
              actionIndex={j}
              onChange={(patch) =>
                setActions(
                  compensation.actions.map((a, k) =>
                    k === j ? ({ ...a, ...patch } as CompensationAction) : a,
                  ),
                )
              }
              onSelectorChange={(patch) =>
                setActions(
                  compensation.actions.map((a, k) => {
                    if (k !== j || !("selector" in a) || !a.selector) return a;
                    const selector = { ...a.selector, ...patch };
                    for (const key of Object.keys(selector) as (keyof Selector)[]) {
                      if (!selector[key]) delete selector[key];
                    }
                    return { ...a, selector } as CompensationAction;
                  }),
                )
              }
            />
          </div>
        ))}
      </div>

      <Button
        type="button"
        variant="secondary"
        size="sm"
        onClick={() => setActions([...compensation.actions, blankCompensationAction("click")])}
      >
        <Plus className="h-3.5 w-3.5" />
        Add undo action
      </Button>

      <div>
        <label className="flex items-center gap-2 text-xs text-[var(--color-muted)]">
          <input
            type="checkbox"
            checked={Boolean(compensation.verify)}
            onChange={(e) =>
              updateCompensate(stepIndex, (c) =>
                c
                  ? {
                      ...c,
                      verify: e.target.checked ? { kind: "textPresent", expected: "" } : undefined,
                    }
                  : c,
              )
            }
          />
          Verify the undo took effect
        </label>
        {compensation.verify && (
          <div className="mt-2 grid gap-2 sm:grid-cols-2">
            <div>
              <FieldLabel>Assertion</FieldLabel>
              <Select
                aria-label={`Step ${stepIndex + 1} undo assertion kind`}
                value={compensation.verify.kind}
                onChange={(e) => {
                  const kind = e.target.value as Verification["kind"];
                  updateCompensate(stepIndex, (c) =>
                    c
                      ? {
                          ...c,
                          verify:
                            kind === "selectorVisible" ? { kind, selector: {} } : { kind, expected: "" },
                        }
                      : c,
                  );
                }}
              >
                <option value="textPresent">Text is present</option>
                <option value="url">URL matches</option>
                <option value="selectorVisible">Element is visible</option>
              </Select>
            </div>
            {compensation.verify.kind !== "selectorVisible" && (
              <div>
                <FieldLabel>Expected</FieldLabel>
                <Input
                  aria-label={`Step ${stepIndex + 1} undo expected`}
                  value={compensation.verify.expected}
                  onChange={(e) => {
                    const expected = e.target.value;
                    updateCompensate(stepIndex, (c) =>
                      c && c.verify && c.verify.kind !== "selectorVisible"
                        ? { ...c, verify: { ...c.verify, expected } }
                        : c,
                    );
                  }}
                  placeholder="Order canceled"
                />
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

function CompensationActionFields({
  action,
  stepIndex,
  actionIndex,
  onChange,
  onSelectorChange,
}: {
  action: CompensationAction;
  stepIndex: number;
  actionIndex: number;
  onChange: (patch: Partial<CompensationAction>) => void;
  onSelectorChange: (patch: Partial<Selector>) => void;
}) {
  const ariaPrefix = `Step ${stepIndex + 1} undo action ${actionIndex + 1}`;

  return (
    <div className="space-y-2">
      {action.type === "navigate" && (
        <div>
          <FieldLabel>URL</FieldLabel>
          <Input
            aria-label={`${ariaPrefix} url`}
            value={action.url}
            onChange={(e) => onChange({ url: e.target.value })}
            placeholder="https://example.com/orders"
          />
        </div>
      )}

      {"selector" in action && action.selector !== undefined && (
        <SelectorFields
          selector={action.selector}
          ariaPrefix={`${ariaPrefix} selector`}
          onChange={onSelectorChange}
        />
      )}

      {(action.type === "fill" || action.type === "select") && (
        <div>
          <FieldLabel>Value</FieldLabel>
          <Input
            aria-label={`${ariaPrefix} value`}
            value={action.value}
            onChange={(e) => onChange({ value: e.target.value })}
            placeholder={action.type === "fill" ? "0" : "Option label"}
          />
        </div>
      )}

      {action.type === "click" && (
        <div>
          <FieldLabel>Description (optional, shown to the approver)</FieldLabel>
          <Input
            aria-label={`${ariaPrefix} description`}
            value={action.description ?? ""}
            onChange={(e) => onChange({ description: e.target.value || undefined })}
            placeholder="Cancel order button"
          />
        </div>
      )}

      {action.type === "waitFor" && (
        <div>
          <FieldLabel>…or milliseconds</FieldLabel>
          <Input
            aria-label={`${ariaPrefix} ms`}
            type="number"
            min={1}
            value={action.ms ?? ""}
            onChange={(e) => onChange({ ms: e.target.value ? Number(e.target.value) : undefined })}
            placeholder="1000"
          />
        </div>
      )}
    </div>
  );
}
