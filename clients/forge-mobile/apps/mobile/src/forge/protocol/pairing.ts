const FORGE_PAIR_PATH = /^\/forge\/([a-f0-9]{64})\/?$/i;
const MAX_PAIRING_URL_LENGTH = 4096;

export class PairingUrlError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "PairingUrlError";
  }
}

function extractCandidate(input: string): string {
  const trimmed = input.trim();
  if (trimmed.length === 0 || trimmed.length > MAX_PAIRING_URL_LENGTH) {
    throw new PairingUrlError("The pairing code is empty or too long.");
  }

  if (!trimmed.toLowerCase().startsWith("forge://")) {
    return trimmed;
  }

  const deepLink = new URL(trimmed);
  if (deepLink.hostname.toLowerCase() !== "pair" || deepLink.pathname !== "") {
    throw new PairingUrlError("This Forge link is not a session pairing link.");
  }
  const candidate = deepLink.searchParams.get("url");
  if (!candidate) {
    throw new PairingUrlError("The Forge pairing link has no session URL.");
  }
  return candidate;
}

/**
 * Accepts the browser handoff `forge://pair?url=<encoded HTTPS bearer URL>`
 * and the raw HTTPS URL emitted in the terminal QR. The returned value is a
 * canonical immutable gateway capability, never a session identifier.
 */
export function parsePairingInput(input: string): string {
  const candidate = extractCandidate(input);
  let gateway: URL;
  try {
    gateway = new URL(candidate);
  } catch {
    throw new PairingUrlError("The pairing code does not contain a valid URL.");
  }
  if (gateway.protocol !== "https:") {
    throw new PairingUrlError("Forge pairings must use tailnet-private HTTPS.");
  }
  const hostname = gateway.hostname.toLowerCase();
  if (hostname !== "ts.net" && !hostname.endsWith(".ts.net")) {
    throw new PairingUrlError("Forge pairings must use a Tailscale Serve hostname.");
  }
  if (gateway.port !== "") {
    throw new PairingUrlError("Forge pairings must use the default HTTPS port.");
  }
  if (gateway.username || gateway.password || gateway.search || gateway.hash) {
    throw new PairingUrlError("The pairing URL contains unsupported credentials or parameters.");
  }
  if (!FORGE_PAIR_PATH.test(gateway.pathname)) {
    throw new PairingUrlError("The URL is not a Forge remote pairing capability.");
  }
  gateway.pathname = `${gateway.pathname.replace(/\/+$/g, "")}/`;
  return gateway.toString();
}

export function eventsUrlForPairing(pairingUrl: string): string {
  const base = new URL(parsePairingInput(pairingUrl));
  const events = new URL("events", base);
  events.protocol = "wss:";
  events.search = "";
  events.hash = "";
  return events.toString();
}

export function pairingDisplayHost(pairingUrl: string): string {
  return new URL(parsePairingInput(pairingUrl)).host;
}
