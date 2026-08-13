import { describe, expect, it } from "vitest";

import { createPairingScanGate } from "./pairingScan";

describe("Forge pairing scan gate", () => {
  it("accepts one camera frame and synchronously deduplicates later frames", () => {
    const gate = createPairingScanGate();

    expect(gate.tryBegin()).toBe(true);
    expect(gate.tryBegin()).toBe(false);
    expect(gate.tryBegin()).toBe(false);
  });

  it("re-arms after an invalid or failed pairing attempt", () => {
    const gate = createPairingScanGate();

    expect(gate.tryBegin()).toBe(true);
    gate.rearm();
    expect(gate.tryBegin()).toBe(true);
  });
});
