import { describe, expect, it } from "vite-plus/test";

import {
  remoteNewSessionHeaderPresentation,
  remoteNewSessionNavigationTarget,
  remoteSessionCreatedRegistrationInput,
} from "./remoteNewSessionPresentation";

describe("remoteNewSessionHeaderPresentation", () => {
  it("hides the pencil until an executable newSession command exists", () => {
    expect(
      remoteNewSessionHeaderPresentation({
        supported: false,
        pending: false,
        hasExecutableHandler: true,
      }),
    ).toBeNull();
    expect(
      remoteNewSessionHeaderPresentation({
        supported: true,
        pending: false,
        hasExecutableHandler: false,
      }),
    ).toBeNull();
  });

  it("labels the current-directory action and disables it while pending", () => {
    expect(
      remoteNewSessionHeaderPresentation({
        supported: true,
        pending: false,
        hasExecutableHandler: true,
      }),
    ).toEqual({
      accessibilityLabel: "Create new session in current directory",
      disabled: false,
      systemImage: "square.and.pencil",
    });
    expect(
      remoteNewSessionHeaderPresentation({
        supported: true,
        pending: true,
        hasExecutableHandler: true,
      }),
    ).toEqual({
      accessibilityLabel: "Creating new session in current directory",
      disabled: true,
      systemImage: "square.and.pencil",
    });
  });
});

describe("sessionCreated handoff", () => {
  it("registers the returned pairing and retains the remote session identity for navigation", () => {
    const outcome = {
      sessionId: "new-session",
      pairingUrl: "https://mac.example/forge/new-token/",
      expiresAt: "2026-08-14T18:00:00.000Z",
    };
    expect(remoteSessionCreatedRegistrationInput(outcome)).toBe(outcome.pairingUrl);
    expect(remoteNewSessionNavigationTarget(outcome, "local-pairing-id")).toEqual({
      pairingId: "local-pairing-id",
      expectedSessionId: "new-session",
      expiresAt: "2026-08-14T18:00:00.000Z",
    });
  });
});
