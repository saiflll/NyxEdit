import { get } from "svelte/store";
import { activeTerminalSessionId } from "$lib/stores.svelte";

const sessionCounter = new Map<string, number>();
let globalCounter = 0;

export function generateSessionId(prefix = "term"): string {
  const count = (sessionCounter.get(prefix) || 0) + 1;
  sessionCounter.set(prefix, count);
  return `${prefix}-${count}`;
}

export function getActiveTerminalSession(): string {
  const current = get(activeTerminalSessionId);
  if (current) return current;

  const newId = generateSessionId("term");
  activeTerminalSessionId.set(newId);
  return newId;
}

export function resetSessionCounter(prefix?: string) {
  if (prefix) {
    sessionCounter.delete(prefix);
  } else {
    sessionCounter.clear();
    globalCounter = 0;
  }
}
