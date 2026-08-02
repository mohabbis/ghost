import { describe, expect, it } from "vitest";
import { EDITABLE_STEP_TYPES } from "@ghost/core/schema/step";
import { UNIMPLEMENTED_ACTION_TYPES } from "./driver.js";

/**
 * Contract between what the editor offers and what the worker can actually do.
 *
 * The editor and the executor live in different packages and are edited by
 * different changes, so nothing stops someone adding a step type to the
 * authoring UI months before its executor exists. The result would not be a
 * crash — `applyStep` returns `{}` for an unimplemented type, so the run would
 * sail past it and report SUCCEEDED having done nothing. A workflow that
 * silently skips its most important step is worse than one that fails.
 *
 * Written as a test rather than a comment on purpose: this session already
 * found a resolver that four test files opted into and the worker's entrypoint
 * did not, which passed CI for exactly as long as nobody encoded the contract.
 */
describe("editable step types", () => {
  it("offers nothing the executor cannot perform", () => {
    const unimplemented = new Set<string>(UNIMPLEMENTED_ACTION_TYPES);
    const offered = EDITABLE_STEP_TYPES.filter((t) => unimplemented.has(t));

    expect(offered).toEqual([]);
  });

  it("lists the connector steps as unimplemented while they are no-ops", () => {
    // Guards the other direction: if someone empties UNIMPLEMENTED_ACTION_TYPES
    // to quiet the test above rather than building the executor, this fails.
    // Delete this assertion when apiCall/sendEmail genuinely execute.
    expect([...UNIMPLEMENTED_ACTION_TYPES].sort()).toEqual(["apiCall", "sendEmail"]);
  });
});
