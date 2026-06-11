import { invoke } from "@tauri-apps/api/core";
import { get, writable } from "svelte/store";
import { currentDir } from "./stores.svelte";

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
