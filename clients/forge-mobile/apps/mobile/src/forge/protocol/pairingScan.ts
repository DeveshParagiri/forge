export interface PairingScanGate {
  readonly tryBegin: () => boolean;
  readonly rearm: () => void;
}

/**
 * Synchronously latches the first camera or paste result while its pairing is
 * being persisted. React state alone cannot do this because iOS may deliver
 * duplicate barcode frames before the component re-renders.
 */
export function createPairingScanGate(): PairingScanGate {
  let pending = false;

  return {
    tryBegin: () => {
      if (pending) return false;
      pending = true;
      return true;
    },
    rearm: () => {
      pending = false;
    },
  };
}
