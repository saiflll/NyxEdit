import { writable } from "svelte/store";

// === DEBUG MODE ===
const DEBUG_ENABLED = import.meta.env.VITE_DEBUG_MODE !== "0";

export function log(...args: any[]) {
  if (!DEBUG_ENABLED) return;
  console.log(...args);
}

export function err(...args: any[]) {
  if (!DEBUG_ENABLED) return;
  console.error(...args);
}

// === ERROR HANDLER ===
export interface ApiError {
  message: string;
  code?: string;
}

export function hndlErr(err: unknown, fallbackMsg = "Terjadi kesalahan"): ApiError {
  const message = err instanceof Error ? err.message : String(err);
  return { message: fallbackMsg, code: "UNKNOWN" };
}

// === TO RESULT PATTERN ===
export async function to<T>(promise: Promise<T>): Promise<[T | null, ApiError | null]> {
  try {
    const result = await promise;
    return [result, null];
  } catch (err) {
    return [null, hndlErr(err)];
  }
}

// === DEBOUNCE ===
export function debounce<T extends (...args: any[]) => any>(fn: T, delay: number) {
  let timeoutId: ReturnType<typeof setTimeout>;
  return (...args: Parameters<T>) => {
    clearTimeout(timeoutId);
    timeoutId = setTimeout(() => fn(...args), delay);
  };
}

// === THROTTLE ===
export function throttle<T extends (...args: any[]) => any>(fn: T, limit: number) {
  let inThrottle = false;
  return (...args: Parameters<T>) => {
    if (!inThrottle) {
      fn(...args);
      inThrottle = true;
      setTimeout(() => (inThrottle = false), limit);
    }
  };
}

// === FORMAT TIME ===
export function fmtTime(d: Date): string {
  return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

// === FORMAT DATETIME ===
export function fmtDateTime(d: Date): string {
  return d.toLocaleString([], { 
    hour: "2-digit", 
    minute: "2-digit", 
    second: "2-digit",
    year: "numeric",
    month: "short",
    day: "numeric"
  });
}

// === TRUNCATE TEXT ===
export function truncate(str: string, len: number): string {
  if (str.length <= len) return str;
  return str.slice(0, len - 3) + "...";
}

// === SANITIZE PATH ===
export function sanitizePath(path: string): string {
  return path.replace(/[<>:"|?*]/g, "_");
}

// === IS BINARY EXT ===
export const BINARY_EXTS = new Set([
  "png","jpg","jpeg","gif","webp","bmp","ico","tiff","avif",
  "svg",
  "mp4","webm","ogg","mkv","mov","avi","mp3","wav","flac","aac","m4a","opus",
]);

export function isBinaryExt(ext: string): boolean {
  return BINARY_EXTS.has(ext.toLowerCase());
}
