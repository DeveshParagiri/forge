const VIEWPORT_HEIGHT_PROPERTY = "--forge-visual-viewport-height";
const VIEWPORT_OFFSET_PROPERTY = "--forge-visual-viewport-offset-top";

function viewportMetrics(host: Window): { height: number; offsetTop: number } {
  const viewport = host.visualViewport;
  return {
    height: Math.max(1, viewport?.height ?? host.innerHeight),
    offsetTop: Math.max(
      0,
      viewport?.offsetTop ?? 0,
      viewport ? (viewport.pageTop ?? host.scrollY) - host.scrollY : 0,
    ),
  };
}

/**
 * iOS Safari keeps the layout viewport full-height when its software keyboard
 * opens. Mirror the smaller visual viewport into CSS so the bottom composer is
 * laid out above the keyboard instead of underneath it.
 */
export function bindVisualViewport(
  root: HTMLElement = document.documentElement,
  host: Window = window,
): () => void {
  const viewport = host.visualViewport;
  let followupFrame = 0;
  let followupTimer = 0;
  const update = () => {
    const { height, offsetTop } = viewportMetrics(host);
    root.style.setProperty(VIEWPORT_HEIGHT_PROPERTY, `${height}px`);
    root.style.setProperty(VIEWPORT_OFFSET_PROPERTY, `${offsetTop}px`);
  };
  const settle = () => {
    update();
    host.cancelAnimationFrame(followupFrame);
    host.clearTimeout(followupTimer);
    followupFrame = host.requestAnimationFrame(update);
    followupTimer = host.setTimeout(update, 100);
  };

  update();
  host.addEventListener("resize", settle, { passive: true });
  host.addEventListener("focusin", settle);
  host.addEventListener("focusout", settle);
  host.addEventListener("pageshow", settle);
  host.addEventListener("orientationchange", settle);
  viewport?.addEventListener("resize", settle, { passive: true });
  viewport?.addEventListener("scroll", settle, { passive: true });

  return () => {
    host.cancelAnimationFrame(followupFrame);
    host.clearTimeout(followupTimer);
    host.removeEventListener("resize", settle);
    host.removeEventListener("focusin", settle);
    host.removeEventListener("focusout", settle);
    host.removeEventListener("pageshow", settle);
    host.removeEventListener("orientationchange", settle);
    viewport?.removeEventListener("resize", settle);
    viewport?.removeEventListener("scroll", settle);
    root.style.removeProperty(VIEWPORT_HEIGHT_PROPERTY);
    root.style.removeProperty(VIEWPORT_OFFSET_PROPERTY);
  };
}
