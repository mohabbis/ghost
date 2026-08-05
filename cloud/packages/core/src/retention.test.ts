import { describe, expect, it } from "vitest";
import {
  DEFAULT_ARTIFACT_RETENTION_DAYS,
  artifactRetentionDaysFromEnv,
  isEligibleForArtifactPurge,
  retentionCutoff,
} from "./retention.js";

describe("retentionCutoff", () => {
  it("subtracts the retention window from now", () => {
    const now = new Date("2026-08-05T00:00:00.000Z");
    expect(retentionCutoff(now, 90).toISOString()).toBe("2026-05-07T00:00:00.000Z");
  });

  it("rejects a non-positive window rather than silently purging everything", () => {
    const now = new Date();
    expect(() => retentionCutoff(now, 0)).toThrow(/positive/);
    expect(() => retentionCutoff(now, -5)).toThrow(/positive/);
    expect(() => retentionCutoff(now, NaN)).toThrow(/positive/);
  });
});

describe("isEligibleForArtifactPurge", () => {
  const cutoff = new Date("2026-08-05T00:00:00.000Z");

  it("is not eligible while the run has not ended", () => {
    expect(isEligibleForArtifactPurge({ endedAt: null, artifactsPurgedAt: null }, cutoff)).toBe(
      false,
    );
  });

  it("is not eligible once already purged, even if old enough", () => {
    expect(
      isEligibleForArtifactPurge(
        {
          endedAt: new Date("2026-01-01T00:00:00.000Z"),
          artifactsPurgedAt: new Date("2026-08-01T00:00:00.000Z"),
        },
        cutoff,
      ),
    ).toBe(false);
  });

  it("is not eligible when it ended after the cutoff", () => {
    expect(
      isEligibleForArtifactPurge(
        { endedAt: new Date("2026-08-04T23:59:59.999Z"), artifactsPurgedAt: null },
        cutoff,
      ),
    ).toBe(true);
    expect(
      isEligibleForArtifactPurge(
        { endedAt: new Date("2026-08-06T00:00:00.000Z"), artifactsPurgedAt: null },
        cutoff,
      ),
    ).toBe(false);
  });

  it("is eligible exactly at the boundary only when strictly before it", () => {
    expect(
      isEligibleForArtifactPurge({ endedAt: cutoff, artifactsPurgedAt: null }, cutoff),
    ).toBe(false);
  });

  it("is eligible for a run that ended well before the cutoff and was never purged", () => {
    expect(
      isEligibleForArtifactPurge(
        { endedAt: new Date("2020-01-01T00:00:00.000Z"), artifactsPurgedAt: null },
        cutoff,
      ),
    ).toBe(true);
  });
});

describe("artifactRetentionDaysFromEnv", () => {
  it("defaults when unset", () => {
    expect(artifactRetentionDaysFromEnv({})).toBe(DEFAULT_ARTIFACT_RETENTION_DAYS);
  });

  it("parses a configured value", () => {
    expect(artifactRetentionDaysFromEnv({ ARTIFACT_RETENTION_DAYS: "30" })).toBe(30);
  });

  it("rejects a non-positive or non-numeric override rather than defaulting silently", () => {
    expect(() => artifactRetentionDaysFromEnv({ ARTIFACT_RETENTION_DAYS: "0" })).toThrow(
      /positive/,
    );
    expect(() => artifactRetentionDaysFromEnv({ ARTIFACT_RETENTION_DAYS: "-1" })).toThrow(
      /positive/,
    );
    expect(() => artifactRetentionDaysFromEnv({ ARTIFACT_RETENTION_DAYS: "soon" })).toThrow(
      /positive/,
    );
  });
});
