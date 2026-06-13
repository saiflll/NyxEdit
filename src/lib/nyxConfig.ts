import { invoke } from "@tauri-apps/api/core";
import { get } from "svelte/store";
import { currentDir } from "./stores.svelte";
import { appDataDir } from "@tauri-apps/api/path";

export async function getNyxDir(): Promise<string | null> {
  const dir = get(currentDir);
  if (!dir) return null;
  const pathSeparator = dir.includes("\\") ? "\\" : "/";
  return `${dir.replace(/[\\/]+$/, "")}${pathSeparator}.nyx`;
}

export async function ensureNyxDir(): Promise<string | null> {
  const nyxDir = await getNyxDir();
  if (!nyxDir) return null;
  try {
    const exists = await invoke<boolean>("fs_exists", { path: nyxDir });
    if (!exists) {
      await invoke("fs_create_dir", { path: nyxDir });
    }
    return nyxDir;
  } catch (e) {
    console.error("Failed to ensure .nyx dir:", e);
    return null;
  }
}

export async function saveNyxConfig(fileName: string, data: any): Promise<void> {
  const nyxDir = await ensureNyxDir();
  if (!nyxDir) return;
  const pathSeparator = nyxDir.includes("\\") ? "\\" : "/";
  const filePath = `${nyxDir}${pathSeparator}${fileName}`;
  try {
    await invoke("fs_write_file", { path: filePath, content: JSON.stringify(data, null, 2) });
  } catch (e) {
    console.error(`Failed to save config ${fileName}:`, e);
  }
}

export async function loadNyxConfig<T>(fileName: string, defaultValue: T): Promise<T> {
  const nyxDir = await getNyxDir();
  if (!nyxDir) return defaultValue;
  const pathSeparator = nyxDir.includes("\\") ? "\\" : "/";
  const filePath = `${nyxDir}${pathSeparator}${fileName}`;
  try {
    const exists = await invoke<boolean>("fs_exists", { path: filePath });
    if (exists) {
      const content = await invoke<string>("fs_read_file", { path: filePath });
      return JSON.parse(content) as T;
    }
  } catch (e) {
    console.error(`Failed to load config ${fileName}:`, e);
  }
  return defaultValue;
}

export interface GlobalSettings {
  globalInstructions: string;
  skillRead: boolean;
  skillWrite: boolean;
  skillTerminal: boolean;
}

// Global configuration helpers saved in persistent AppData directory (survives app updates)
export async function getGlobalSettingsPath(): Promise<string | null> {
  try {
    const dir = await appDataDir();
    if (!dir) return null;
    const pathSeparator = dir.includes("\\") ? "\\" : "/";
    return `${dir.replace(/[\\/]+$/, "")}${pathSeparator}global_settings.json`;
  } catch (e) {
    console.error("Failed to get app data directory:", e);
    return null;
  }
}

export async function getGlobalFilePath(fileName: string): Promise<string | null> {
  try {
    const dir = await appDataDir();
    if (!dir) return null;
    const pathSeparator = dir.includes("\\") ? "\\" : "/";
    return `${dir.replace(/[\\/]+$/, "")}${pathSeparator}${fileName}`;
  } catch (e) {
    console.error("Failed to get app data directory:", e);
    return null;
  }
}

export async function saveGlobalFile(fileName: string, data: any): Promise<void> {
  try {
    const filePath = await getGlobalFilePath(fileName);
    if (filePath) {
      await invoke("fs_write_file", { path: filePath, content: JSON.stringify(data, null, 2) });
    }
  } catch (e) {
    console.error(`Failed to save ${fileName}:`, e);
  }
}

export async function loadGlobalFile<T>(fileName: string, defaultValue: T): Promise<T> {
  try {
    const filePath = await getGlobalFilePath(fileName);
    if (filePath) {
      const exists = await invoke<boolean>("fs_exists", { path: filePath });
      if (exists) {
        const content = await invoke<string>("fs_read_file", { path: filePath });
        return JSON.parse(content) as T;
      }
    }
  } catch (e) {
    console.error(`Failed to load ${fileName}:`, e);
  }
  return defaultValue;
}

export async function saveGlobalNyxConfig(data: GlobalSettings): Promise<void> {
  try {
    const filePath = await getGlobalSettingsPath();
    if (filePath) {
      await invoke("fs_write_file", { path: filePath, content: JSON.stringify(data, null, 2) });
    }
  } catch (e) {
    console.error("Failed to save global settings:", e);
  }
}

export async function loadGlobalNyxConfig<T>(defaultValue: T): Promise<T> {
  try {
    const filePath = await getGlobalSettingsPath();
    if (filePath) {
      const exists = await invoke<boolean>("fs_exists", { path: filePath });
      if (exists) {
        const content = await invoke<string>("fs_read_file", { path: filePath });
        return JSON.parse(content) as T;
      }
    }
  } catch (e) {
    console.error("Failed to load global settings:", e);
  }
  return defaultValue;
}

export async function getActiveSettings(): Promise<GlobalSettings> {
  const defaultSettings: GlobalSettings = {
    globalInstructions: "",
    skillRead: true,
    skillWrite: true,
    skillTerminal: true
  };

  // 1. Try to load workspace-specific settings
  const workspaceDirVal = get(currentDir);
  if (workspaceDirVal) {
    const config = await loadNyxConfig<any>("style_coding.json", null);
    if (config) {
      return {
        globalInstructions: config.globalInstructions ?? "",
        skillRead: config.skillRead ?? true,
        skillWrite: config.skillWrite ?? true,
        skillTerminal: config.skillTerminal ?? true
      };
    }
  }

  // 2. Fallback to global settings in appDataDir
  const globalConfig = await loadGlobalNyxConfig<GlobalSettings | null>(null);
  if (globalConfig) {
    return {
      globalInstructions: globalConfig.globalInstructions ?? "",
      skillRead: globalConfig.skillRead ?? true,
      skillWrite: globalConfig.skillWrite ?? true,
      skillTerminal: globalConfig.skillTerminal ?? true
    };
  }

  // 3. Fallback to localStorage (legacy)
  try {
    const globalInstructions = localStorage.getItem("nyxedit-global-instructions") || "";
    const skillRead = localStorage.getItem("nyxedit-skill-read") !== "false";
    const skillWrite = localStorage.getItem("nyxedit-skill-write") !== "false";
    const skillTerminal = localStorage.getItem("nyxedit-skill-terminal") !== "false";
    return { globalInstructions, skillRead, skillWrite, skillTerminal };
  } catch {
    return defaultSettings;
  }
}
