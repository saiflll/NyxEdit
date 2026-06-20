import { invoke } from "@tauri-apps/api/core";
import { addToast, reviewFindings, isReviewing } from "$lib/stores.svelte";
import { err } from "./helpers";

// === TRIGGER FILE REVIEW ===
export async function triggerFileReview(filePath: string, content: string) {
  if (!filePath) return;
  isReviewing.set(true);
  try {
    const findings = await invoke<any[]>("review_file", { filePath, content });
    reviewFindings.set(findings);
    const errors = findings.filter(f => f.severity === "Error" || f.severity === "error").length;
    const warnings = findings.length - errors;
    if (findings.length > 0) {
      addToast(`Reviewed ${filePath.split(/[\\/]/).pop()}! Found ${errors} errors, ${warnings} warnings.`, "warning");
    } else {
      addToast("File reviewed. No issues found.", "success");
    }
  } catch (e) {
    err("Auto-review failed for file:", e);
  } finally {
    isReviewing.set(false);
  }
}


// === AUTO DETECT WORKSPACE ===
export async function autoDetectWorkspace(dir: string, callbacks: {
  onProjectDetected?: (framework: string) => void;
  onLog?: (msg: string) => void;
}) {
  if (!dir) return;
  
  try {
    const detected = await invoke<string>("project_detect", { root: dir }).catch(() => "");
    
    if (detected) {
      callbacks.onProjectDetected?.(detected);
      addToast(`Project detected: ${detected}`, "success");
      
      // Auto-load knowledge graph
      await invoke("graph_load_workspace", { root: dir }).catch(() => {});
      
      // Auto-run review jika enabled
      const autoReview = true; // TODO: Get from settings
      if (autoReview) {
        await invoke("review_text", { text: "", path: dir }).catch(() => {});
      }
    }
  } catch (e) {
    err("Failed to auto-detect workspace:", e);
  }
}

// === INTEL AUTO-LOAD ===
export async function autoLoadIntel(dir: string) {
  if (!dir) return;
  
  try {
    // Load symbol graph untuk intel
    await invoke("graph_load_workspace", { root: dir }).catch(() => {});
    
    // Trigger project detection jika belum
    const detected = await invoke<string>("project_detect", { root: dir }).catch(() => "");
    if (detected) {
      addToast(`Intel loaded for: ${detected}`, "info");
    }
  } catch (e) {
    err("Failed to auto-load intel:", e);
  }
}

// === REVIEW AUTO-RUN ===
export async function autoRunReview(dir: string, enabled: boolean) {
  if (!dir || !enabled) return;
  
  try {
    const result = await invoke<any>("review_text", { text: "", path: dir });
    if (result && result.issues && result.issues.length > 0) {
      addToast(`Review found ${result.issues.length} issues`, "warning");
    }
  } catch (e) {
    err("Auto-review failed:", e);
  }
}

// === HEALTH CHECK ===
export async function checkSystemHealth(onCrashDetected?: (components: string[]) => void) {
  try {
    const health = await invoke<any>("heal_check_startup");
    if (health?.crashed && onCrashDetected) {
      const crashed: string[] = health.crashed_components ?? [];
      onCrashDetected(crashed);
    }
    return health;
  } catch (e) {
    err("Health check failed:", e);
    return null;
  }
}

// === CLEAR CRASH MARKER ===
export async function clearCrashMarker() {
  try {
    await invoke("heal_clear_crash_marker");
  } catch (e) {
    err("Failed to clear crash marker:", e);
  }
}
