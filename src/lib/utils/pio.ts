/**
 * PlatformIO utilities
 * Handles detection, installation, initialization, and running targets
 */

import { invoke } from "@tauri-apps/api/core";
import { err } from "./helpers";

export type PioResult = { success: boolean; output: string; error: string | null };
export type PioStatus = { installed: boolean; version: string | null; python: string | null; error?: string };

const MAX_PIO_BOARDS = 50;

export interface PioState {
  status: PioStatus;
  boards: string[];
  busy: boolean;
  installing: boolean;
  initPath: string;
  statusMsg: string;
  boardSearch: string;
  boardLimit: number;
}

export function createPioState(): PioState {
  return {
    status: { installed: false, version: null, python: null },
    boards: [],
    busy: false,
    installing: false,
    initPath: "",
    statusMsg: "",
    boardSearch: "",
    boardLimit: MAX_PIO_BOARDS,
  };
}

export async function checkPio(force = false, state: PioState, addLog: (msg: string) => void): Promise<void> {
  const hasCheckedRef = { value: false };
  
  if (!force && hasCheckedRef.value) return;
  
  try {
    const s = await invoke<PioStatus>("pio_detect");
    state.status = s;
    hasCheckedRef.value = true;
    
    if (s.installed) {
      addLog(`PlatformIO detected: ${s.version}`);
      if (state.boards.length === 0) {
        const boards = await invoke<string[]>("pio_list_boards", { search: null });
        state.boards = boards.slice(0, 20);
      }
    }
  } catch (e: any) {
    state.status = { installed: false, version: null, python: null, error: String(e) };
  }
}

export async function installPio(state: PioState, addLog: (msg: string) => void): Promise<void> {
  state.installing = true;
  state.statusMsg = "Installing PlatformIO...";
  
  try {
    const res = await invoke<PioResult>("pio_install");
    if (res.success) {
      state.statusMsg = "PlatformIO installed successfully!";
      addLog("PlatformIO installed");
      await checkPio(true, state, addLog);
    } else {
      state.statusMsg = "Installation failed";
      err(res.error);
    }
  } catch (e: any) {
    state.statusMsg = "Installation error";
    err(e);
  }
  
  state.installing = false;
}

export async function initPioProject(board: string | undefined, primaryCwd: string, state: PioState, addLog: (msg: string) => void): Promise<void> {
  const path = state.initPath || primaryCwd;
  if (!path) return;
  
  state.busy = true;
  state.statusMsg = "Initializing project...";
  
  try {
    const res = await invoke<PioResult>("pio_init", { path, board: board || null });
    if (res.success) {
      state.statusMsg = board ? `Project initialized with ${board}!` : "Project initialized!";
      addLog(`PIO project initialized at ${path}${board ? ` (${board})` : ""}`);
    } else {
      state.statusMsg = "Init failed";
      err(res.error);
    }
  } catch (e: any) {
    state.statusMsg = "Init error";
    err(e);
  }
  
  state.busy = false;
}

export async function runPioTarget(target: string, directory: string, state: PioState, addLog: (msg: string) => void): Promise<void> {
  try {
    const res = await invoke<PioResult>("pio_run", { target, directory });
    state.statusMsg = `pio ${target}: ${res.success ? "done" : "failed"}`;
    addLog(`PIO ${target}: ${res.success ? "OK" : "FAIL"}`);
    if (!res.success) err(res.error);
  } catch (e: any) {
    state.statusMsg = `pio ${target} error`;
    err(e);
  }
}

export function getFilteredBoards(state: PioState): string[] {
  if (!state.boardSearch) {
    return state.boards.slice(0, MAX_PIO_BOARDS);
  }
  return state.boards
    .filter(b => b.toLowerCase().includes(state.boardSearch.toLowerCase()))
    .slice(0, MAX_PIO_BOARDS);
}

export const PIO_SCRIPTS = {
  antigravity: { cmd: "powershell", args: ["-c", "Invoke-Expression (Invoke-RestMethod https://raw.githubusercontent.com/antigravity/cli/main/install.ps1)"] },
  opencode: { cmd: "powershell", args: ["-c", "Invoke-Expression (Invoke-RestMethod https://raw.githubusercontent.com/opencode-ai/cli/main/install.ps1)"] },
};

export function initPioProjectWithBoard(board: string, primaryCwd: string, state: PioState, addLog: (msg: string) => void): void {
  initPioProject(board, primaryCwd, state, addLog);
}
