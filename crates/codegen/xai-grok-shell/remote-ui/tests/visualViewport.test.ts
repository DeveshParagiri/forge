import { afterEach, describe, expect, it, vi } from "vitest";
import { bindVisualViewport } from "../src/visualViewport";

const originalVisualViewport = Object.getOwnPropertyDescriptor(window, "visualViewport");
const originalInnerHeight = Object.getOwnPropertyDescriptor(window, "innerHeight");

afterEach(() => {
  vi.useRealTimers();
  if (originalVisualViewport) {
    Object.defineProperty(window, "visualViewport", originalVisualViewport);
  } else {
    Reflect.deleteProperty(window, "visualViewport");
  }
  if (originalInnerHeight) Object.defineProperty(window, "innerHeight", originalInnerHeight);
  document.documentElement.removeAttribute("style");
});

describe("iOS visual viewport binding", () => {
  it("shrinks and offsets the app with the visible viewport while the keyboard is open", () => {
    const events = new EventTarget();
    let height = 844;
    let offsetTop = 0;
    Object.defineProperties(events, {
      height: { configurable: true, get: () => height },
      offsetTop: { configurable: true, get: () => offsetTop },
    });
    const viewport = events as unknown as VisualViewport;
    Object.defineProperty(window, "visualViewport", { configurable: true, value: viewport });

    const unbind = bindVisualViewport();
    expect(document.documentElement.style.getPropertyValue("--forge-visual-viewport-height")).toBe("844px");
    expect(document.documentElement.style.getPropertyValue("--forge-visual-viewport-offset-top")).toBe("0px");

    height = 397.5;
    offsetTop = 51;
    events.dispatchEvent(new Event("resize"));
    expect(document.documentElement.style.getPropertyValue("--forge-visual-viewport-height")).toBe("397.5px");
    expect(document.documentElement.style.getPropertyValue("--forge-visual-viewport-offset-top")).toBe("51px");

    offsetTop = 68;
    events.dispatchEvent(new Event("scroll"));
    expect(document.documentElement.style.getPropertyValue("--forge-visual-viewport-offset-top")).toBe("68px");

    unbind();
    expect(document.documentElement.style.getPropertyValue("--forge-visual-viewport-height")).toBe("");
    expect(document.documentElement.style.getPropertyValue("--forge-visual-viewport-offset-top")).toBe("");
  });

  it("falls back to the window height when VisualViewport is unavailable", () => {
    Object.defineProperty(window, "visualViewport", { configurable: true, value: undefined });
    Object.defineProperty(window, "innerHeight", { configurable: true, value: 712 });

    const unbind = bindVisualViewport();
    expect(document.documentElement.style.getPropertyValue("--forge-visual-viewport-height")).toBe("712px");
    expect(document.documentElement.style.getPropertyValue("--forge-visual-viewport-offset-top")).toBe("0px");
    unbind();
  });

  it("rechecks after WebKit publishes its corrected keyboard offset late", () => {
    vi.useFakeTimers();
    const events = new EventTarget();
    let offsetTop = 0;
    Object.defineProperties(events, {
      height: { configurable: true, value: 430 },
      offsetTop: { configurable: true, get: () => offsetTop },
      pageTop: { configurable: true, get: () => offsetTop },
    });
    Object.defineProperty(window, "visualViewport", {
      configurable: true,
      value: events as unknown as VisualViewport,
    });

    const unbind = bindVisualViewport();
    events.dispatchEvent(new Event("resize"));
    expect(document.documentElement.style.getPropertyValue("--forge-visual-viewport-offset-top")).toBe("0px");

    offsetTop = 51;
    vi.advanceTimersByTime(100);
    expect(document.documentElement.style.getPropertyValue("--forge-visual-viewport-offset-top")).toBe("51px");
    unbind();
  });
});
