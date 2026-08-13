export const NATIVE_HANDOFF_DELAY_MS = 800;

export function buildNativePairingUrl(bearerUrl: string): string {
  return `forge://pair?url=${encodeURIComponent(bearerUrl)}`;
}

export function attemptNativePairing(
  bearerUrl: string,
  documentTarget: Pick<Document, "body" | "createElement"> = document,
): void {
  const anchor = documentTarget.createElement("a");
  anchor.href = buildNativePairingUrl(bearerUrl);
  anchor.hidden = true;
  anchor.tabIndex = -1;
  documentTarget.body.append(anchor);
  anchor.click();
  anchor.remove();
}

interface VisibilityTarget {
  readonly hidden: boolean;
  addEventListener(type: "visibilitychange", listener: () => void): void;
  removeEventListener(type: "visibilitychange", listener: () => void): void;
}

export interface NativeHandoffController {
  continueInBrowser(): void;
  retryNative(): Promise<void>;
  dispose(): void;
}

export function beginNativeHandoff(options: {
  bearerUrl: string;
  visibilityTarget?: VisibilityTarget;
  attemptNative?: (bearerUrl: string) => void;
  beforeNativeAttempt?: () => void | Promise<void>;
  onBrowserFallback(): void;
  onNativeClaimed?(): void;
  delayMs?: number;
  attemptOnStart?: boolean;
}): NativeHandoffController {
  const visibilityTarget = options.visibilityTarget ?? document;
  const attempt = options.attemptNative ?? attemptNativePairing;
  const delayMs = options.delayMs ?? NATIVE_HANDOFF_DELAY_MS;
  let timer: number | null = null;
  let disposed = false;
  let browserChosen = false;
  let nativeClaimed = false;
  let attemptGeneration = 0;

  const cancelTimer = () => {
    if (timer !== null) window.clearTimeout(timer);
    timer = null;
  };
  const chooseBrowser = () => {
    if (disposed || browserChosen) return;
    browserChosen = true;
    cancelTimer();
    visibilityTarget.removeEventListener("visibilitychange", onVisibilityChange);
    options.onBrowserFallback();
  };
  const scheduleFallback = () => {
    cancelTimer();
    timer = window.setTimeout(chooseBrowser, delayMs);
  };
  const onVisibilityChange = () => {
    if (disposed || browserChosen) return;
    if (visibilityTarget.hidden) {
      nativeClaimed = true;
      attemptGeneration += 1;
      cancelTimer();
      options.onNativeClaimed?.();
      return;
    }
    if (nativeClaimed) chooseBrowser();
  };
  const retryNative = async () => {
    if (disposed || browserChosen) return;
    nativeClaimed = false;
    const generation = ++attemptGeneration;
    cancelTimer();
    await options.beforeNativeAttempt?.();
    if (disposed || browserChosen || nativeClaimed || generation !== attemptGeneration) return;
    attempt(options.bearerUrl);
    scheduleFallback();
  };

  visibilityTarget.addEventListener("visibilitychange", onVisibilityChange);
  if (options.attemptOnStart === false) {
    scheduleFallback();
  } else {
    void retryNative();
  }

  return {
    continueInBrowser: chooseBrowser,
    retryNative,
    dispose: () => {
      disposed = true;
      attemptGeneration += 1;
      cancelTimer();
      visibilityTarget.removeEventListener("visibilitychange", onVisibilityChange);
    },
  };
}
