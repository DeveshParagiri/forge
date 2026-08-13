export function openForgeHomeThread(
  pairingId: string,
  reconnect: (pairingId: string) => void,
  navigate: (pairingId: string) => void,
): void {
  reconnect(pairingId);
  navigate(pairingId);
}
