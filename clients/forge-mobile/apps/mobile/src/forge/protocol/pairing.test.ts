import { describe, expect, it } from "vitest";

import { eventsUrlForPairing, parsePairingInput } from "./pairing";

const token = "0123456789abcdef".repeat(4);
const terminalQrPayload = `https://forge-mac.example-tailnet.ts.net/forge/${token}/`;

describe("Forge native pairing URL boundary", () => {
  it("accepts the exact trailing-slash HTTPS payload emitted by the terminal /rc QR", () => {
    expect(parsePairingInput(terminalQrPayload)).toBe(terminalQrPayload);
    expect(eventsUrlForPairing(terminalQrPayload)).toBe(
      `wss://forge-mac.example-tailnet.ts.net/forge/${token}/events`,
    );
  });

  it("accepts only canonical private Tailscale Serve HTTPS capabilities", () => {
    const raw = `https://forge-mac.example-tailnet.ts.net/forge/${token}`;
    expect(parsePairingInput(raw)).toBe(`${raw}/`);
    expect(
      parsePairingInput(`forge://pair?url=${encodeURIComponent(raw)}`),
    ).toBe(`${raw}/`);
    expect(eventsUrlForPairing(raw)).toBe(
      `wss://forge-mac.example-tailnet.ts.net/forge/${token}/events`,
    );
    expect(
      parsePairingInput(`HTTPS://FORGE-MAC.EXAMPLE-TAILNET.TS.NET/forge/${token}`),
    ).toBe(`https://forge-mac.example-tailnet.ts.net/forge/${token}/`);
  });

  it("rejects arbitrary HTTPS hosts, lookalike suffixes, and non-default ports", () => {
    const rejected = [
      `https://example.com/forge/${token}`,
      `https://forge.ts.net.evil.example/forge/${token}`,
      `https://notts.net/forge/${token}`,
      `https://forge.example.ts.net:8443/forge/${token}`,
      `http://forge.example.ts.net/forge/${token}`,
      `https://forge.example.ts.net/forge/${token}?client=phone`,
      `https://forge.example.ts.net/forge/${token}#credential`,
      `https://user@forge.example.ts.net/forge/${token}`,
    ];
    for (const input of rejected) expect(() => parsePairingInput(input)).toThrow();
  });
});
