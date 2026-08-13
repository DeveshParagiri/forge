import { afterEach, describe, expect, it, vi } from "vitest";
import { beginNativeHandoff, buildNativePairingUrl, NATIVE_HANDOFF_DELAY_MS } from "../src/nativeHandoff";

const BEARER = `https://forge.tail.example/forge/${"a".repeat(64)}/`;

class VisibilityFixture {
  hidden = false;
  private listeners = new Set<() => void>();
  addEventListener(_type: "visibilitychange", listener: () => void) { this.listeners.add(listener); }
  removeEventListener(_type: "visibilitychange", listener: () => void) { this.listeners.delete(listener); }
  change(hidden: boolean) {
    this.hidden = hidden;
    for (const listener of this.listeners) listener();
  }
}

afterEach(() => vi.useRealTimers());

describe("native-first bearer handoff", () => {
  it("constructs the exact Forge deep link without placing the bearer in text", () => {
    expect(buildNativePairingUrl(BEARER)).toBe(`forge://pair?url=${encodeURIComponent(BEARER)}`);
  });

  it("does not let the browser claim the socket before the native window expires", async () => {
    vi.useFakeTimers();
    const fallback = vi.fn();
    const attempt = vi.fn();
    const controller = beginNativeHandoff({
      bearerUrl: BEARER,
      visibilityTarget: new VisibilityFixture(),
      attemptNative: attempt,
      onBrowserFallback: fallback,
    });
    await Promise.resolve();
    expect(attempt).toHaveBeenCalledWith(BEARER);
    expect(fallback).not.toHaveBeenCalled();
    vi.advanceTimersByTime(NATIVE_HANDOFF_DELAY_MS - 1);
    expect(fallback).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(fallback).toHaveBeenCalledOnce();
    controller.dispose();
  });

  it("cancels browser fallback permanently when opening the app hides Safari", () => {
    vi.useFakeTimers();
    const visibility = new VisibilityFixture();
    const fallback = vi.fn();
    const claimed = vi.fn();
    const controller = beginNativeHandoff({
      bearerUrl: BEARER,
      visibilityTarget: visibility,
      attemptNative: vi.fn(),
      onBrowserFallback: fallback,
      onNativeClaimed: claimed,
    });
    visibility.change(true);
    vi.advanceTimersByTime(NATIVE_HANDOFF_DELAY_MS * 10);
    expect(claimed).toHaveBeenCalledOnce();
    expect(fallback).not.toHaveBeenCalled();
    controller.dispose();
  });

  it("mounts browser mode exactly once when Safari returns after native handoff", () => {
    vi.useFakeTimers();
    const visibility = new VisibilityFixture();
    const fallback = vi.fn();
    const claimed = vi.fn();
    const controller = beginNativeHandoff({
      bearerUrl: BEARER,
      visibilityTarget: visibility,
      attemptNative: vi.fn(),
      onBrowserFallback: fallback,
      onNativeClaimed: claimed,
    });

    visibility.change(true);
    expect(claimed).toHaveBeenCalledOnce();
    visibility.change(false);
    expect(fallback).toHaveBeenCalledOnce();
    vi.advanceTimersByTime(NATIVE_HANDOFF_DELAY_MS * 10);
    visibility.change(true);
    visibility.change(false);
    expect(fallback).toHaveBeenCalledOnce();
    controller.dispose();
  });

  it("lets an explicit Continue in browser bypass the remaining delay", () => {
    vi.useFakeTimers();
    const fallback = vi.fn();
    const controller = beginNativeHandoff({
      bearerUrl: BEARER,
      visibilityTarget: new VisibilityFixture(),
      attemptNative: vi.fn(),
      onBrowserFallback: fallback,
    });
    controller.continueInBrowser();
    expect(fallback).toHaveBeenCalledOnce();
    vi.advanceTimersByTime(NATIVE_HANDOFF_DELAY_MS * 2);
    expect(fallback).toHaveBeenCalledOnce();
  });

  it("relinquishes the browser socket before opening the native scheme", async () => {
    vi.useFakeTimers();
    let finishRelease: (() => void) | undefined;
    const order: string[] = [];
    const attempt = vi.fn(() => order.push("scheme"));
    const controller = beginNativeHandoff({
      bearerUrl: BEARER,
      visibilityTarget: new VisibilityFixture(),
      attemptNative: attempt,
      attemptOnStart: false,
      beforeNativeAttempt: () => new Promise<void>((resolve) => {
        order.push("release-start");
        finishRelease = () => {
          order.push("release-finished");
          resolve();
        };
      }),
      onBrowserFallback: vi.fn(),
    });

    const retry = controller.retryNative();
    await Promise.resolve();
    expect(order).toEqual(["release-start"]);
    expect(attempt).not.toHaveBeenCalled();

    finishRelease?.();
    await retry;
    expect(order).toEqual(["release-start", "release-finished", "scheme"]);
    controller.dispose();
  });

  it("does not open the native scheme when Continue wins a pending release race", async () => {
    vi.useFakeTimers();
    let finishRelease: (() => void) | undefined;
    const attempt = vi.fn();
    const fallback = vi.fn();
    const controller = beginNativeHandoff({
      bearerUrl: BEARER,
      visibilityTarget: new VisibilityFixture(),
      attemptNative: attempt,
      attemptOnStart: false,
      beforeNativeAttempt: () => new Promise<void>((resolve) => {
        finishRelease = resolve;
      }),
      onBrowserFallback: fallback,
    });

    const retry = controller.retryNative();
    await Promise.resolve();
    controller.continueInBrowser();
    finishRelease?.();
    await retry;

    expect(fallback).toHaveBeenCalledOnce();
    expect(attempt).not.toHaveBeenCalled();
  });
});
