/**
 * SSH session utilities
 * Handles SSH connection, session management, and remote file operations
 */

import { invoke } from "@tauri-apps/api/core";
import { err } from "./helpers";

export type SshProfile = {
  id: string;
  name: string;
  host: string;
  port: number;
  username: string;
  password?: string;
  privateKey?: string;
};

export interface SshState {
  activeProfile: SshProfile | null;
  sessions: Map<string, any>;
}

export function createSshState(): SshState {
  return {
    activeProfile: null,
    sessions: new Map(),
  };
}

export function openSshSessionTab(
  profile: SshProfile,
  tabs: any[],
  setTabs: (tabs: any[]) => void,
  setActiveTabId: (id: string) => void
): void {
  const existing = tabs.find(t => t.type === "ssh_session" && t.sshProfile?.id === profile.id);
  if (existing) {
    setActiveTabId(existing.id);
    return;
  }
  
  const id = "tab-ssh-" + Date.now().toString(36);
  setTabs([...tabs, { 
    id, 
    type: "ssh_session", 
    label: `SSH: ${profile.name}`, 
    sshProfile: profile 
  }]);
  setActiveTabId(id);
}

export async function onRemoteFileOpen(
  sessionId: string,
  remotePath: string,
  name: string,
  addLog: (msg: string) => void,
  addToast: (msg: string, type?: string) => void,
  tabs: any[],
  addTab: (type: string, extra?: any) => string
): Promise<void> {
  const path = `sftp://${sessionId}${remotePath}`;
  const existing = tabs.find((t: any) => t.filePath === path);
  
  if (existing) {
    // Tab already exists, just activate it
    return;
  }
  
  try {
    addLog(`Opening remote file ${remotePath} via SFTP...`);
    const content = await invoke<string>("sftp_read_file", {
      sessionId,
      remotePath
    });
    addTab("file", { 
      label: `[Remote] ${name}`, 
      filePath: path, 
      fileContent: content 
    });
  } catch (e: any) {
    addToast(`Failed to open remote file: ${e}`, "error");
  }
}

export async function markSessionPrivate(sessionId: string): Promise<void> {
  try {
    await invoke("pty_mark_private", { sessionId });
  } catch (e) {
    err("Failed to mark private:", e);
  }
}

export const SIDEBAR_LABELS: Record<string, string> = {
  files: "Workspace",
  search: "Search",
  ssh: "SSH Tree",
  postman: "API Client",
  mqtt: "MQTT Client",
  platformio: "Platform IO",
  extensions: "Extensions",
  database: "Database Client",
  intel: "Intel",
};
