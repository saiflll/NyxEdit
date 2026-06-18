import { invoke } from "@tauri-apps/api/core";
import { addToast } from "$lib/stores.svelte";
import { getActiveTerminalSession } from "./terminal";
import { BINARY_EXTS } from "./helpers";

// === TAB LABELS & ICONS ===
export const TAB_LABELS = {
  file: "Untitled",
  settings: "Settings",
  setup: "Setup",
  terminal: "Terminal",
  preview: "Preview",
  ssh_session: "SSH",
  private: "Private",
  api_client: "API Client",
  db_query: "DB Query",
  diff: "AI Diff",
  ai_chat: "AI Chat",
} as const;

export const TAB_ICONS = {
  file: `<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5 2z"/><polyline points="14 2 14 8 20 8"/></svg>`,
  settings: `<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="3"/><path d="M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42"/></svg>`,
  setup: `<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 2L2 7l10 5 10-5-10-5z"/><path d="M2 17l10 5 10-5"/><path d="M2 12l10 5 10-5"/></svg>`,
  terminal: `<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/></svg>`,
  preview: `<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="5"/><line x1="12" y1="1" x2="12" y2="3"/><line x1="12" y1="21" x2="12" y2="23"/><line x1="4.22" y1="4.22" x2="5.64" y2="5.64"/><line x1="18.36" y1="18.36" x2="19.78" y2="19.78"/><line x1="1" y1="12" x2="3" y2="12"/><line x1="21" y1="12" x2="23" y2="12"/><line x1="4.22" y1="19.78" x2="5.64" y2="18.36"/><line x1="18.36" y1="5.64" x2="19.78" y2="4.22"/></svg>`,
  ssh_session: `<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="2" width="20" height="20" rx="4"/><line x1="6" y1="6" x2="18" y2="6"/><line x1="6" y1="12" x2="18" y2="12"/><line x1="6" y1="18" x2="18" y2="18"/></svg>`,
  private: `<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="11" width="18" height="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>`,
  api_client: `<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/></svg>`,
  db_query: `<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><ellipse cx="12" cy="5" rx="9" ry="3"/><path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"/><path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"/></svg>`,
  diff: `<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="7" height="18" rx="1"/><rect x="14" y="3" width="7" height="18" rx="1"/></svg>`,
  ai_chat: `<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>`,
} as const;

// === TAB MANAGEMENT ===
export function createTabId(prefix = "tab") {
  return `${prefix}-${Date.now().toString(36)}${Math.random().toString(36).slice(2, 4)}`;
}

export function generateTabLabel(type: keyof typeof TAB_LABELS, count: number) {
  const base = TAB_LABELS[type];
  return (type === "terminal" || type === "file") ? `${base} ${count + 1}` : base;
}

// === FILE OPERATIONS ===
export async function openFileInTab(
  path: string,
  tabs: any[],
  setActiveTab: (id: string) => void,
  activeFile: any,
  fileContent: any,
  addTab: (type: string, extra?: any) => string
) {
  const existing = tabs.find((t) => t.filePath === path);
  if (existing) {
    setActiveTab(existing.id);
    activeFile.set(path);
    if (existing.fileContent !== undefined) fileContent.set(existing.fileContent);
    return;
  }

  const name = path.split(/[\\/]/).pop() || "Untitled";
  const ext = name.split(".").pop()?.toLowerCase() || "";

  if (BINARY_EXTS.has(ext)) {
    const tabId = addTab("file", { label: name, filePath: path, fileContent: "" });
    activeFile.set(path);
    fileContent.set("");
    return;
  }

  activeFile.set(path);
  try {
    const content = await invoke<string>("fs_read_file", { path });
    fileContent.set(content);
    addTab("file", { label: name, filePath: path, fileContent: content });
  } catch {
    addTab("file", { label: name, filePath: path, fileContent: "" });
  }
}

// === LOGGING ===
export function createLogger(addLogFn: (msg: string, type: string) => void) {
  const origLog = console.log;
  const origError = console.error;

  console.log = (...args: any[]) => {
    origLog(...args);
    addLogFn(args.map(a => typeof a === "string" ? a : JSON.stringify(a)).join(" "), "info");
  };

  console.error = (...args: any[]) => {
    origError(...args);
    addLogFn(args.map(a => typeof a === "string" ? a : JSON.stringify(a)).join(" "), "error");
  };

  return () => {
    console.log = origLog;
    console.error = origError;
  };
}

// === PLATFORM IO HELPERS ===
export async function runPioCommand(
  command: string,
  args: Record<string, any>,
  successMsg: string,
  errorMsg: string
) {
  try {
    const res = await invoke(command, args);
    addToast(successMsg, "success");
    return res;
  } catch (e: any) {
    addToast(`${errorMsg}: ${e.message}`, "error");
    throw e;
  }
}

// === EXTENSION HELPERS ===
export function applyExtensionTheme(ext: { theme?: Record<string, string>; installed: boolean }) {
  if (!ext.theme || !ext.installed) return;
  const root = document.documentElement;
  for (const [key, val] of Object.entries(ext.theme)) {
    root.style.setProperty(key, val);
  }
}

export function removeExtensionTheme(ext: { theme?: Record<string, string> }) {
  if (!ext.theme) return;
  const root = document.documentElement;
  for (const key of Object.keys(ext.theme)) {
    root.style.removeProperty(key);
  }
}

// === SCRIPT RUNNER ===
export async function runScriptInTerminal(
  script: string,
  sessionId?: string
) {
  const sid = sessionId || getActiveTerminalSession();
  const isWin = navigator.userAgent.toLowerCase().includes("win");
  await invoke("pty_write", { sessionId: sid, data: script + (isWin ? "\r" : "\n") });
}
