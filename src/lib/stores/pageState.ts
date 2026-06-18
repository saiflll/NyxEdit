import { writable, type Writable } from "svelte/store";

// === TAB TYPES ===
export type TabType = 
  | "file" | "settings" | "setup" | "terminal" | "preview" 
  | "ssh_session" | "private" | "api_client" | "db_query" 
  | "diff" | "ai_chat";

export type SidebarView = 
  | "files" | "search" | "ssh" | "postman" | "mqtt" 
  | "platformio" | "extensions" | "database" | "intel" | null;

export type WorkspaceMode = "explorer" | "git" | "ssh_explorer";

// === TAB STATE ===
export interface Tab {
  id: string;
  type: TabType;
  label: string;
  filePath?: string;
  fileContent?: string;
  previewUrl?: string;
  isNew?: boolean;
  isDirty?: boolean;
  sshProfile?: any;
  requestId?: string;
  initialCommand?: string;
  connectionId?: string;
  diffFiles?: Array<{ path: string; oldContent: string; newContent: string }>;
}

export const tabs: Writable<Tab[]> = writable([]);
export const activeTabId: Writable<string> = writable("");
export const privateSessionIds: Writable<Set<string>> = writable(new Set());

// === SIDEBAR STATE ===
export const sidebarView: Writable<SidebarView> = writable("files");
export const workspaceMode: Writable<WorkspaceMode> = writable("explorer");
export const sidebarWidth: Writable<number> = writable(220);

export const activityViews: Writable<Array<"files" | "search" | "ssh" | "postman" | "mqtt" | "platformio" | "extensions" | "database" | "intel">> = writable([
  "files", "search", "intel", "platformio", "database", "postman", "mqtt", "ssh", "extensions"
]);

// === FLOATING PANELS ===
export const showFloatingAi: Writable<boolean> = writable(false);
export const showFloatingRunner: Writable<boolean> = writable(false);
export const showLogs: Writable<boolean> = writable(false);

// === PRIMARY CWD & PROJECT ===
export const primaryCwd: Writable<string> = writable("");
export const activeFilePath: Writable<string> = writable("");
export const autoReviewEnabled: Writable<boolean> = writable(true);
export const projectDetected: Writable<string | null> = writable(null);

// === LOGS ===
export interface LogEntry {
  time: string;
  msg: string;
  type: string;
}
export const logs: Writable<LogEntry[]> = writable([]);

// === CLOCK ===
export const now: Writable<Date> = writable(new Date());

// === PLATFORM IO ===
export interface PioResult {
  success: boolean;
  output: string;
  error: string | null;
}

export interface PioStatus {
  installed: boolean;
  version: string | null;
  python: string | null;
  error?: string;
}

export const pioStatus: Writable<PioStatus> = writable({ installed: false, version: null, python: null });
export const pioBoards: Writable<string[]> = writable([]);
export const pioBusy: Writable<boolean> = writable(false);
export const pioInstalling: Writable<boolean> = writable(false);
export const pioInitPath: Writable<string> = writable("");
export const pioStatusMsg: Writable<string> = writable("");
export const pioBoardSearch: Writable<string> = writable("");
export const pioBoardLimit: Writable<number> = writable(50);

// === EXTENSIONS ===
export interface Extension {
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
}

export const extensions: Writable<Extension[]> = writable([]);
export const extUrl: Writable<string> = writable("");
export const extBusy: Writable<boolean> = writable(false);
export const extMsg: Writable<string> = writable("");

// === SYSTEM HEALTH ===
export const proxyPort: Writable<number> = writable(0);
export const degradedCount: Writable<number> = writable(0);

// === DRAG & DROP ===
export const dragIconIndex: Writable<number | null> = writable(null);

// === ADD MENU ===
export const addMenuOpen: Writable<boolean> = writable(false);

// === BINARY EXTENSIONS ===
export const BINARY_EXTS = new Set([
  "png","jpg","jpeg","gif","webp","bmp","ico","tiff","avif",
  "svg",
  "mp4","webm","ogg","mkv","mov","avi","mp3","wav","flac","aac","m4a","opus",
]);
