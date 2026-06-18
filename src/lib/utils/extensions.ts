/**
 * Extension management utilities
 * Handles loading, saving, installing, and removing extensions
 */

import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
import { invoke } from "@tauri-apps/api/core";
import { getActiveTerminalSession } from "./terminal";
import { getStoredTheme, applyTheme } from "../themes";
import { setExtensionIcons, getExtensionIcons } from "../icon-overrides";

export type Extension = {
  id: string;
  name: string;
  version: string;
  description: string;
  type: string;
  url?: string;
  installed: boolean;
  theme?: Record<string, string>;
  icons?: Record<string, string>;
  scripts?: { install?: string; uninstall?: string };
};

const EXT_STORAGE_KEY = "nyxedit-extensions";

export function loadExtensions(): Extension[] {
  try {
    const raw = localStorage.getItem(EXT_STORAGE_KEY);
    if (raw) return JSON.parse(raw);
  } catch {}
  return [];
}

export function saveExtensions(extensions: Extension[]): void {
  try {
    localStorage.setItem(EXT_STORAGE_KEY, JSON.stringify(extensions));
  } catch {}
}

export function applyExtensionTheme(ext: Extension): void {
  if (!ext.theme || !ext.installed) return;
  const root = document.documentElement;
  for (const [key, val] of Object.entries(ext.theme)) {
    root.style.setProperty(key, val);
  }
}

export function removeExtensionTheme(ext: Extension): void {
  if (!ext.theme) return;
  const root = document.documentElement;
  for (const key of Object.keys(ext.theme)) {
    root.style.removeProperty(key);
  }
  applyTheme(getStoredTheme());
}

export async function addExtensionFromUrl(
  url: string,
  extensions: Extension[],
  setExtensions: (exts: Extension[]) => void,
  addLog: (msg: string) => void,
  tabs: any[],
  setActiveTabId: (id: string) => void
): Promise<{ success: boolean; msg: string }> {
  if (!url.trim()) return { success: false, msg: "Empty URL" };
  
  try {
    const res = await tauriFetch(url.trim());
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    
    const data = await res.json();
    if (!data.name) throw new Error("Invalid extension format: missing 'name'");
    
    const id = "ext-" + Date.now().toString(36);
    const ext: Extension = {
      id,
      name: data.name,
      version: data.version || "1.0",
      description: data.description || "",
      type: data.type || "misc",
      url: url.trim(),
      installed: true,
      theme: data.theme,
      icons: data.icons,
      scripts: data.scripts,
    };
    
    setExtensions([...extensions, ext]);
    saveExtensions([...extensions, ext]);
    
    if (ext.theme) applyExtensionTheme(ext);
    if (ext.icons) {
      const all = { ...getExtensionIcons(), ...ext.icons };
      setExtensionIcons(all);
    }
    
    if (ext.scripts?.install) {
      const termTab = tabs.find(t => t.type === "terminal");
      if (termTab) setActiveTabId(termTab.id);
      const isWin = navigator.userAgent.toLowerCase().includes("win");
      invoke("pty_write", { 
        sessionId: getActiveTerminalSession(), 
        data: ext.scripts.install + (isWin ? "\r" : "\n") 
      }).catch(() => {});
    }
    
    addLog(`Installed extension: ${ext.name}`);
    return { success: true, msg: `Installed: ${ext.name}` };
  } catch (e: any) {
    return { success: false, msg: `Error: ${e.message}` };
  }
}

export function removeExtension(
  id: string,
  extensions: Extension[],
  setExtensions: (exts: Extension[]) => void,
  addLog: (msg: string) => void
): string {
  const ext = extensions.find(e => e.id === id);
  if (!ext) return "Extension not found";
  
  if (ext.scripts?.uninstall) {
    const isWin = navigator.userAgent.toLowerCase().includes("win");
    invoke("pty_write", { 
      sessionId: getActiveTerminalSession(), 
      data: ext.scripts.uninstall + (isWin ? "\r" : "\n") 
    }).catch(() => {});
  }
  
  removeExtensionTheme(ext);
  
  if (ext.icons) {
    const all = getExtensionIcons();
    for (const k of Object.keys(ext.icons)) delete all[k];
    setExtensionIcons(all);
  }
  
  const filtered = extensions.filter(e => e.id !== id);
  setExtensions(filtered);
  saveExtensions(filtered);
  
  addLog(`Removed extension: ${ext.name}`);
  return `Removed: ${ext.name}`;
}

export function toggleExtension(
  id: string,
  extensions: Extension[],
  setExtensions: (exts: Extension[]) => void,
  addLog: (msg: string) => void,
  tabs: any[],
  setActiveTabId: (id: string) => void
): Promise<{ success: boolean; msg: string }> | null {
  const ext = extensions.find(e => e.id === id);
  if (!ext) return null;
  
  if (ext.installed) {
    const msg = removeExtension(id, extensions, setExtensions, addLog);
    return Promise.resolve({ success: true, msg });
  } else {
    if (ext.url) {
      return addExtensionFromUrl(ext.url, extensions, setExtensions, addLog, tabs, setActiveTabId);
    }
  }
  return null;
}
