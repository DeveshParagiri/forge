export interface RemoteSessionCreatedOutcome {
  readonly sessionId: string;
  readonly pairingUrl: string;
  readonly expiresAt: string;
}

export interface RemoteNewSessionHeaderPresentation {
  readonly accessibilityLabel: string;
  readonly disabled: boolean;
  readonly systemImage: "square.and.pencil";
}

export interface RemoteNewSessionNavigationTarget {
  readonly pairingId: string;
  readonly expectedSessionId: string;
  readonly expiresAt: string;
}

export function remoteNewSessionHeaderPresentation(input: {
  readonly supported: boolean;
  readonly pending: boolean;
  readonly hasExecutableHandler: boolean;
}): RemoteNewSessionHeaderPresentation | null {
  if (!input.supported || !input.hasExecutableHandler) return null;
  return {
    accessibilityLabel: input.pending
      ? "Creating new session in current directory"
      : "Create new session in current directory",
    disabled: input.pending,
    systemImage: "square.and.pencil",
  };
}

export function remoteSessionCreatedRegistrationInput(
  outcome: RemoteSessionCreatedOutcome,
): string {
  return outcome.pairingUrl;
}

export function remoteNewSessionNavigationTarget(
  outcome: RemoteSessionCreatedOutcome,
  registeredPairingId: string,
): RemoteNewSessionNavigationTarget {
  return {
    pairingId: registeredPairingId,
    expectedSessionId: outcome.sessionId,
    expiresAt: outcome.expiresAt,
  };
}
