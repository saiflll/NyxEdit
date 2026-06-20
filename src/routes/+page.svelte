<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { open as openShell } from "@tauri-apps/plugin-shell";
  import SplitTerminal from "$lib/components/SplitTerminal.svelte";
  import AIChat from "$lib/components/AIChat.svelte";
  import AIDiffTab from "$lib/components/AIDiffTab.svelte";
  import GitStatus from "$lib/components/GitStatus.svelte";
  import FileManager from "$lib/components/FileManager.svelte";
  import CodeEditor from "$lib/components/CodeEditor.svelte";
  import ViewerRouter from "$lib/components/viewers/ViewerRouter.svelte";
  import Settings from "$lib/components/Settings.svelte";
  import IntelPanel from "$lib/components/IntelPanel.svelte";
  import Runner from "$lib/components/Runner.svelte";
  import Toast from "$lib/components/Toast.svelte";
  import ContextMenu from "$lib/components/ContextMenu.svelte";
  import SearchInFiles from "$lib/components/SearchInFiles.svelte";
  import CommandPalette from "$lib/components/CommandPalette.svelte";
  import AIFloatingBar from "$lib/components/AIFloatingBar.svelte";
  import SSHTree from "$lib/components/SSHTree.svelte";
  import SSHSession from "$lib/components/SSHSession.svelte";
  import PostmanClient from "$lib/components/PostmanClient.svelte";
  import MQTTClient from "$lib/components/MQTTClient.svelte";
  import DatabaseClient from "$lib/components/DatabaseClient.svelte";
  import SSHExplorer from "$lib/components/SSHExplorer.svelte";
  import { 
    currentDir, addToast, aiSendRequest, activeSshProfile, activeFile, fileContent, type Agent,
    reviewFindings, isReviewing, graphStatus, systemHealth, costSummary, loadWorkspace, workspaceFolders
  } from "$lib/stores.svelte";
  import { get } from "svelte/store";
  import { onMount } from "svelte";
  import { getStoredTheme, getStoredFont, applyTheme, applyFont } from "$lib/themes";
  import { setExtensionIcons, getExtensionIcons } from "$lib/icon-overrides";
  import { initIdle } from "$lib/idle.svelte";
  import { getActiveTerminalSession } from "$lib/utils/terminal";

  type TabType = "file" | "settings" | "setup" | "terminal" | "preview" | "ssh_session" | "private" | "api_client" | "db_query" | "diff" | "ai_chat";
  type SidebarView = "files" | "search" | "ssh" | "postman" | "mqtt" | "platformio" | "extensions" | "database" | "intel" | null;

  let proxyPort = $state(0);
  let degradedCount = $state(0);

  let tabs = $state<Tab[]>([
    { id: "tab-term", type: "terminal", label: "Terminal" },
  ]);
  let activeTabId = $state("tab-term");

  // Track private terminal session IDs (AI cannot write to these)
  let privateSessionIds = $state<Set<string>>(new Set());

  type Tab = {
    id: string; type: TabType; label: string;
    filePath?: string; fileContent?: string; previewUrl?: string;
    isNew?: boolean; isDirty?: boolean;
    sshProfile?: any; // for ssh_session tabs
    requestId?: string; // for api_client tabs
    initialCommand?: string; // for terminal tabs
    connectionId?: string; // for db_query tabs
    diffFiles?: { path: string; oldContent: string; newContent: string }[]; // for diff tabs
  };

  // ─── Sidebar ───────────────────────────────────
  let sidebarView = $state<SidebarView>("files");
  let workspaceMode = $state<"explorer" | "git" | "ssh_explorer">("explorer");
  let sidebarWidth = $state(220);

  let activityViews = $state<("files" | "search" | "ssh" | "postman" | "mqtt" | "platformio" | "extensions" | "database" | "intel")[]>([
    "files",
    "search",
    "intel",
    "platformio",
    "database",
    "postman",
    "mqtt",
    "ssh",
    "extensions"
  ]);
  let dragIconIndex = $state<number | null>(null);

  function handleIconDragStart(e: DragEvent, index: number) {
    dragIconIndex = index;
    if (e.dataTransfer) {
      e.dataTransfer.effectAllowed = "move";
      e.dataTransfer.setData("text/plain", index.toString());
    }
  }
  function handleIconDrop(index: number) {
    if (dragIconIndex === null || dragIconIndex === index) return;
    const views = [...activityViews];
    const draggedItem = views.splice(dragIconIndex, 1)[0];
    views.splice(index, 0, draggedItem);
    activityViews = views;
    dragIconIndex = null;
    try {
      localStorage.setItem("codlib-activity-order", JSON.stringify(views));
    } catch {}
  }
  function handleIconDragOver(e: DragEvent) {
    e.preventDefault();
  }

  // ─── Floating Panels ──────────────────────────
  let showFloatingAi = $state(false);
  let aiChatRef = $state<any>();
  function closeAiChat() {
    showFloatingAi = false;
    if (aiChatRef) {
      aiChatRef.clearChat();
    }
  }
  function minimizeAiChat() {
    showFloatingAi = false;
  }
  function openAiChatTab() {
    showFloatingAi = false;
    const existing = tabs.find(t => t.type === "ai_chat");
    if (existing) {
      activeTabId = existing.id;
      return;
    }
    const id = "tab-ai-" + Date.now().toString(36);
    tabs = [...tabs, { id, type: "ai_chat", label: "AI Chat" }];
    activeTabId = id;
  }
  function handleOpenAiInTab(content: string, label: string) {
    const id = "tab-ai-plan-" + Date.now().toString(36);
    tabs = [...tabs, { id, type: "file", label: label || "AI Plan", filePath: "", fileContent: content }];
    activeTabId = id;
  }
  function handleOpenDiffTab(msgIdx: number, changes: { path: string; oldContent: string; newContent: string }[]) {
    const id = `diff-msg-${msgIdx}`;
    const existing = tabs.find(t => t.id === id);
    if (existing) {
      activeTabId = existing.id;
      return;
    }
    tabs = [...tabs, { id, type: "diff", label: `Diff: AI Msg #${msgIdx + 1}`, diffFiles: changes }];
    activeTabId = id;
  }
  let showFloatingRunner = $state(false);
  let showLogs = $state(false);

  // CMMO Global Reactive States
  let activeFramework = $state("Unknown");
  let isIndexingGraph = $state(false);
  let indexProgress = $state(0);
  let indexCurrentIndex = $state(0);
  let indexTotalFiles = $state(0);
  let indexCurrentFile = $state("");
  let agentSwarmStatus = $state("Idle");

  // ─── Primary CWD (from terminal) ──────────────
  let primaryCwd = $state("");
  let activeFilePath = $state("");
  let autoReviewEnabled = $state(true);
  let projectDetected = $state<string | null>(null);
  
  // Import auto-detect functions
  import { autoDetectWorkspace, autoLoadIntel, autoRunReview, triggerFileReview } from "$lib/utils/workspace";

  // ─── App Logs ─────────────────────────────────
  let logs = $state<{ time: string; msg: string; type: string }[]>([]);
  function addLog(msg: string, type = "info") {
    const t = new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
    logs = [...logs, { time: t, msg, type }];
  }
  
  // Import logger dengan cleanup otomatis
  import { createLogger } from "$lib/utils/tabHelpers";
  
  // ─── Network & All AI Cost ────────────────────
  let pingGithub = $state<number | null>(null);
  let isOnline = $state(true);
  let totalAllAiCost = $state(0.0);
  let registeredAgentsCount = $state(0);
  let signalBars = $derived(!isOnline || pingGithub === null ? 0 : pingGithub < 150 ? 4 : pingGithub < 300 ? 3 : pingGithub < 600 ? 2 : 1);

  async function checkGithubLatency() {
    isOnline = navigator.onLine;
    if (!isOnline) {
      pingGithub = null;
      return;
    }
    const start = performance.now();
    try {
      const controller = new AbortController();
      const timeoutId = setTimeout(() => controller.abort(), 3000);
      await fetch("https://api.github.com/meta", { 
        method: "HEAD", 
        mode: "no-cors",
        cache: "no-store",
        signal: controller.signal
      });
      clearTimeout(timeoutId);
      pingGithub = Math.round(performance.now() - start);
      isOnline = true;
    } catch (e) {
      pingGithub = null;
      isOnline = false;
    }
  }

  async function updateAllAiCost() {
    try {
      const list = await invoke<any[]>("ai_get_usage");
      totalAllAiCost = list.reduce((sum, item) => sum + (item.total_cost || 0), 0);
    } catch (e) {
      console.warn("Failed to get all AI usage:", e);
    }
  }

  async function updateRegisteredAgents() {
    try {
      const agentsList = await invoke<any[]>("ai_list_agents");
      registeredAgentsCount = agentsList.length;
    } catch {}
  }

  let cleanupConsole: (() => void) | null = null;
  onMount(() => {
    cleanupConsole = createLogger(addLog);
    addLog("App started");
    showLogs = false; // thin by default
    
    checkGithubLatency();
    const netInterval = setInterval(checkGithubLatency, 15000);

    updateAllAiCost();
    updateRegisteredAgents();

    const unsubDonePromise = listen("ai:done", () => {
      updateAllAiCost();
    });
    
    return () => {
      if (cleanupConsole) cleanupConsole();
      clearInterval(netInterval);
      unsubDonePromise.then(unsub => unsub());
    };
  });

  // ─── Clock ────────────────────────────────────
  let now = $state(new Date());
  $effect(() => {
    const id = setInterval(() => { now = new Date(); }, 1000);
    return () => clearInterval(id);
  });
  import { fmtTime, log, err } from "$lib/utils/helpers";

  // ─── Platform IO ──────────────────────────────
  import { createPioState, checkPio, installPio, initPioProject, runPioTarget, getFilteredBoards, PIO_SCRIPTS, initPioProjectWithBoard } from "$lib/utils/pio";
  
  let pioState = $state(createPioState());
  let hasCheckedPio = false;
  let pioFilteredBoards = $derived(getFilteredBoards(pioState));

  // ─── Extensions ──────────────────────────────
  import { loadExtensions, saveExtensions, applyExtensionTheme, removeExtensionTheme, addExtensionFromUrl, removeExtension, toggleExtension, type Extension } from "$lib/utils/extensions";
  
  let extensions = $state<Extension[]>([]);
  let extUrl = $state("");
  let extBusy = $state(false);
  let extMsg = $state("");
  
  // Load extensions on init
  extensions = loadExtensions();

  // ─── Tab labels & icons (moved to utils) ──────
  import { TAB_LABELS, TAB_ICONS, createTabId, generateTabLabel } from "$lib/utils/tabHelpers";

  function normalizeUrl(url: string): string {
    if (!url || url === "undefined") return url;
    if (url.startsWith("http://") || url.startsWith("https://") || url.startsWith("file://")) return url;
    return "https://" + url;
  }

  // ─── Tab management ───────────────────────────
  let addMenuOpen = $state(false);
  let nyxMenuOpen = $state(false);

  async function handleOpenWorkspace() {
    try {
      const selected = await openDialog({
        directory: false,
        multiple: false,
        filters: [{
          name: "NyxEdit Workspace",
          extensions: ["workspace"]
        }]
      });
      if (selected && typeof selected === "string") {
        const content = await invoke<string>("fs_read_file", { path: selected });
        const data = JSON.parse(content);
        if (data && Array.isArray(data.folders)) {
          const folders = data.folders.map((f: any) => {
            if (f && typeof f === "object" && typeof f.path === "string") {
              return f.path;
            }
            return String(f);
          });
          loadWorkspace(folders);
          addToast("Workspace loaded successfully", "success");
        }
      }
    } catch (e) {
      console.error("Load workspace error:", e);
      addToast("Failed to load workspace file", "error");
    }
  }

  async function handleSaveWorkspace() {
    try {
      const { save } = await import("@tauri-apps/plugin-dialog");
      const selected = await save({
        filters: [{
          name: "NyxEdit Workspace",
          extensions: ["workspace"]
        }],
        defaultPath: "project.workspace"
      });
      if (selected && typeof selected === "string") {
        const folders = get(workspaceFolders);
        const workspaceData = {
          folders
        };
        const content = JSON.stringify(workspaceData, null, 2);
        await invoke("fs_write_file", { path: selected, content });
        addToast("Workspace saved successfully", "success");
      }
    } catch (e) {
      console.error("Save workspace error:", e);
      addToast("Failed to save workspace", "error");
    }
  }

  async function handleAddFolderToWorkspace() {
    try {
      const selected = await openDialog({
        directory: true,
        multiple: false,
      });
      if (selected && typeof selected === "string") {
        const folders = get(workspaceFolders);
        if (!folders.includes(selected)) {
          loadWorkspace([...folders, selected]);
          addToast("Folder added to workspace", "success");
        }
      }
    } catch (e) {
      console.error("Add folder error:", e);
    }
  }

  function addTab(type: TabType, extra?: Partial<Tab>) {
    const base = TAB_LABELS[type];
    const count = tabs.filter((t) => t.type === type).length;
    const label = generateTabLabel(type, count);
    const id = createTabId("tab");
    tabs = [...tabs, { id, type, label, ...extra }];
    activeTabId = id;
    addMenuOpen = false;
    return id;
  }

  function closeTab(id: string) {
    if (tabs.length <= 1) return;
    const tab = tabs.find((t) => t.id === id);
    if (tab?.isDirty && !confirm(`"${tab.label}" has unsaved changes. Close anyway?`)) return;
    const idx = tabs.findIndex((t) => t.id === id);
    tabs = tabs.filter((t) => t.id !== id);
    if (activeTabId === id) {
      activeTabId = tabs[Math.min(idx, tabs.length - 1)]?.id || tabs[0]?.id || "";
    }
  }

  function closeAllTabs() {
    const dirty = tabs.filter((t) => t.isDirty);
    if (dirty.length > 0 && !confirm(`Close all tabs? ${dirty.length} tab(s) have unsaved changes.`)) return;
    tabs = [];
    activeTabId = "";
  }

  function closeOtherTabs(id: string) {
    const tab = tabs.find((t) => t.id === id);
    if (!tab) return;
    const others = tabs.filter((t) => t.id !== id && t.isDirty);
    if (others.length > 0 && !confirm(`Close other tabs? ${others.length} tab(s) have unsaved changes.`)) return;
    tabs = [tab];
    activeTabId = id;
  }

  function setActiveTab(id: string) {
    activeTabId = id;
    const tab = tabs.find(t => t.id === id);
    if (tab?.type === "file" && tab.filePath) {
      activeFile.set(tab.filePath);
      if (tab.fileContent !== undefined) fileContent.set(tab.fileContent);
    }
  }

  const activeTab = $derived(tabs.find((t) => t.id === activeTabId));

  // ─── File open → create tab ──────────────────
  const BINARY_EXTS = new Set([
    "png","jpg","jpeg","gif","webp","bmp","ico","tiff","avif",
    "svg",
    "mp4","webm","ogg","mkv","mov","avi","mp3","wav","flac","aac","m4a","opus",
  ]);

  function onFileOpen(path: string) {
    const existing = tabs.find((t) => t.filePath === path);
    if (existing) {
      activeTabId = existing.id;
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
    invoke<string>("fs_read_file", { path }).then((content) => {
      fileContent.set(content);
      addTab("file", { label: name, filePath: path, fileContent: content });
    }).catch(() => {
      addTab("file", { label: name, filePath: path, fileContent: "" });
    });
    
    // Auto-detect workspace saat buka file/folder
    const dir = path.includes(".") ? path.substring(0, path.lastIndexOf("/")) : path;
    autoLoadIntel(dir);
  }

  function handleFileSave(path: string, content: string) {
    const tab = tabs.find((t) => t.filePath === path);
    if (tab) {
      tab.fileContent = content;
      tab.isDirty = false;
    }
  }

  function markTabDirty(id: string) {
    const tab = tabs.find((t) => t.id === id);
    if (tab && tab.type === "file") {
      tab.isDirty = true;
    }
  }

  function makeOnSaveForTab(tabId: string) {
    return (_path: string, content: string) => {
      const t = tabs.find((tt) => tt.id === tabId);
      if (t) {
        t.fileContent = content;
        t.isDirty = false;
      }
    };
  }

  async function pollSystemHealth() {
    try {
      const components = await invoke<any[]>("heal_get_status");
      degradedCount = components.filter(c => c.status !== "Healthy").length;
    } catch (e) {
      console.warn("Failed to check health:", e);
    }
  }

  onMount(() => {
    applyTheme(getStoredTheme());
    applyFont(getStoredFont());
    initIdle();
    invoke<number>("get_proxy_port")
      .then(port => proxyPort = port)
      .catch(() => {});
    try {
      const saved = localStorage.getItem("codlib-activity-order");
      if (saved) {
        activityViews = JSON.parse(saved);
      }
    } catch {}

    // Poll health status
    pollSystemHealth();
    const healthInterval = setInterval(pollSystemHealth, 30000);

    return () => {
      clearInterval(healthInterval);
    };
  });

  $effect(() => {
    const unsub = aiSendRequest.subscribe(req => {
      if (req) {
        showFloatingAi = true;
      }
    });
    return unsub;
  });

  $effect(() => {
    const unsub = activeSshProfile.subscribe(profile => {
      if (profile) {
        sidebarView = "files";
        workspaceMode = "ssh_explorer";
      }
    });
    return unsub;
  });

  function throttle<T extends (...args: any[]) => void>(func: T, limit: number): T {
    let inThrottle = false;
    return function(this: any, ...args: any[]) {
      if (!inThrottle) {
        func.apply(this, args);
        inThrottle = true;
        setTimeout(() => inThrottle = false, limit);
      }
    } as any;
  }

  onMount(() => {
    const throttledProgress = throttle((data: any) => {
      indexProgress = data.progress;
      indexCurrentFile = data.current_file;
      indexTotalFiles = data.total_files;
      indexCurrentIndex = data.current_index;
      
      if (data.current_index === 1 || data.current_index === data.total_files || Math.round(data.progress) % 20 === 0) {
        const fn = data.current_file.split(/[/\\]/).pop() || data.current_file;
        addLog(`[Graph Sync] Indexing progress: ${Math.round(data.progress)}% [${fn}] (${data.current_index}/${data.total_files})`, "info");
      }
    }, 200);

    const cleanups: (() => void)[] = [];

    Promise.all([
      listen<number>("graph:index_start", (event) => {
        isIndexingGraph = true;
        indexTotalFiles = event.payload;
        indexProgress = 0;
        indexCurrentIndex = 0;
        indexCurrentFile = "Analyzing files...";
        graphStatus.set("Syncing");
        addLog(`Started indexing workspace: ${event.payload} files found.`, "info");
      }),
      listen<any>("graph:index_progress", (event) => {
        isIndexingGraph = true;
        throttledProgress(event.payload);
      }),
      listen<number>("graph:index_end", (event) => {
        isIndexingGraph = false;
        indexProgress = 100;
        graphStatus.set("Synced");
        addLog(`Workspace indexing finished. Indexed ${event.payload} symbols.`, "success");
        addToast(`Workspace indexed: ${event.payload} symbols found`, "success");
      }),
      listen<string>("graph:status", (event) => {
        graphStatus.set(event.payload);
        addLog(`[Graph Sync] Knowledge graph status: ${event.payload}`, "info");
      }),
      listen<any>("ai:budget_warning", (event) => {
        addToast(event.payload.message, "warning");
        costSummary.set(event.payload.cost || { total_cost: 0.0, limit: 0.0 });
        addLog(`[Budget Warning] ${event.payload.message}`, "warning");
      }),
      listen<any>("ai:agent_swarm_status", (event) => {
        agentSwarmStatus = event.payload.status || "Idle";
        addLog(`[Agent Swarm] ${event.payload.status}: ${event.payload.detail || ""}`, "info");
      }),
    ]).then((fns) => {
      cleanups.push(...fns);
    });

    return () => {
      for (const fn of cleanups) fn();
    };
  });

  // Proactive background diagnostics polling
  $effect(() => {
    const updateStats = async () => {
      try {
        const health = await invoke<any>("heal_check_startup");
        systemHealth.set(health || { ok: true, status: "Healthy" });
      } catch (e) {
        systemHealth.set({ ok: false, status: "Check failed" });
      }
      try {
        const summary = await invoke<any>("cost_get_summary");
        costSummary.set(summary || { total_cost: 0.0, limit: 0.0 });
      } catch (e) {
        console.warn("Failed to get cost summary:", e);
      }
      try {
        await updateAllAiCost();
        await updateRegisteredAgents();
      } catch {}
    };
    
    updateStats();
    const interval = setInterval(updateStats, 30000); // Poll every 30s
    return () => clearInterval(interval);
  });

  import { ensureNyxDir } from "$lib/nyxConfig";
  $effect(() => {
    const unsub = currentDir.subscribe((val) => {
      if (val) {
        ensureNyxDir().catch(() => {});

        invoke("ai_set_workspace", { root: val }).catch(() => {});

        addLog(`Detecting framework for: ${val}...`, "info");
        invoke<any>("project_detect", { root: val }).then((detected) => {
          if (detected && detected.framework) {
            let label = detected.framework;
            if (detected.framework === "RustCargo") label = "Rust/Cargo";
            else if (detected.framework === "NodeNpm") label = "Node.js (npm)";
            else if (detected.framework === "NodeYarn") label = "Node.js (yarn)";
            else if (detected.framework === "PythonPoetry") label = "Python (Poetry)";
            else if (detected.framework === "PythonPip") label = "Python (pip)";
            else if (detected.framework === "GoMod") label = "Go";
            else if (detected.framework === "PlatformIO") label = "PlatformIO";
            else if (detected.framework === "Docker") label = "Docker";
            activeFramework = label;
            addLog(`Project detected: ${label}`, "success");
            addToast(`Project detected: ${label}`, "success");
          } else {
            activeFramework = "Unknown";
            addLog("No specific project framework detected.", "info");
          }
        }).catch((e) => {
          err("Framework detection failed:", e);
          activeFramework = "Unknown";
        });

        addLog("Loading workspace symbol graph...", "info");
        graphStatus.set("Syncing");
        invoke<boolean>("graph_load_workspace", { root: val }).then((exists) => {
          if (exists) {
            graphStatus.set("Synced");
            addLog("Loaded existing symbol graph successfully.", "success");
            addToast("Symbol graph loaded", "info");
          } else {
            addLog("No existing symbol graph found. Auto-indexing in progress...", "info");
            invoke("graph_index_workspace", { root: val }).catch((e) => {
              err("Auto-indexing failed to start:", e);
              addLog(`Auto-indexing failed: ${e}`, "error");
              graphStatus.set("Error");
            });
          }
        }).catch((e) => {
          err("Workspace load/indexing check failed:", e);
          graphStatus.set("Error");
        });

        invoke("graph_watch", { root: val }).then(() => {
          addLog("File watcher started. Symbols will sync live on file edits.", "info");
        }).catch((e) => {
          err("Failed to start file watcher:", e);
        });

        addLog("Running initial static code review...", "info");
        invoke<any>("review_text", { text: "" }).then((res) => {
          if (res && res.findings) {
            reviewFindings.set(res.findings);
          }
        }).catch((e) => {
          err("Initial review check failed:", e);
        });
      }
    });
    return unsub;
  });

  // Open SSH session as a dedicated tab
  import { openSshSessionTab, onRemoteFileOpen, markSessionPrivate } from "$lib/utils/ssh";
  
  function openDbQueryTab(connId: string, label: string) {
    const existing = tabs.find(t => t.type === "db_query" && t.connectionId === connId);
    if (existing) { activeTabId = existing.id; return; }
    const id = "tab-db-" + Date.now().toString(36);
    tabs = [...tabs, { id, type: "db_query", label: `DB: ${label}`, connectionId: connId }];
    activeTabId = id;
  }

  // ─── Custom Window Controls ───────────────────
  const appWindow = getCurrentWindow();
  function minimizeWindow() {
    log("Minimizing window...");
    appWindow.minimize().catch(err => {
      err("Minimize error:", err);
      alert("Minimize error: " + err);
    });
  }
  async function toggleMaximizeWindow() {
    log("Toggling maximize window...");
    try {
      if (await appWindow.isMaximized()) {
        await appWindow.unmaximize();
      } else {
        await appWindow.maximize();
      }
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      log("Maximize error:", msg);
      alert("Maximize error: " + msg);
    }
  }
  async function openNewWindow() {
    try {
      const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
      const label = "nyxedit-" + Math.random().toString(36).slice(2, 9);
      new WebviewWindow(label, {
        title: "NyxEdit",
        width: 1400,
        height: 900,
        minWidth: 900,
        minHeight: 600,
        decorations: false
      });
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      log("Failed to open new window:", msg);
    }
  }

  async function triggerOpenFile() {
    try {
      const selected = await openDialog({
        multiple: false,
        directory: false,
      });
      if (selected && typeof selected === "string") {
        onFileOpen(selected);
      }
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      log("Open file error:", msg);
    }
  }

  async function triggerOpenFolder() {
    try {
      const selected = await openDialog({
        multiple: false,
        directory: true,
      });
      if (selected && typeof selected === "string") {
        primaryCwd = selected;
        currentDir.set(selected);
      }
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      log("Open folder error:", msg);
    }
  }

  function openSettingsTab() {
    const existing = tabs.find(t => t.type === "settings");
    if (existing) {
      activeTabId = existing.id;
    } else {
      addTab("settings");
    }
  }

  function closeWindow() {
    log("Closing window...");
    appWindow.close().catch(err => {
      err("Close error:", err);
      alert("Close error: " + err);
    });
  }

  function handleHeaderMousedown(e: MouseEvent) {
    if (e.button !== 0) return;
    const target = e.target as HTMLElement;
    if (target.closest(".mac-controls, .tab-add-wrap, button, input, select, textarea, .add-menu")) {
      return;
    }
    appWindow.startDragging().catch(err => {
      log("Manual drag failed:", err);
    });
  }

  // ─── CWD from terminal ────────────────────────
  function onTerminalCwdChange(cwd: string) {
    primaryCwd = cwd;
    currentDir.set(cwd);
    autoDetectWorkspace(cwd, {
      onProjectDetected: (fw) => log(`Project detected: ${fw}`),
      onLog: (msg) => log(msg)
    });
  }

  // ─── Sidebar ──────────────────────────────────
  import { SIDEBAR_LABELS } from "$lib/utils/ssh";
  
  function toggleSidebar(view: SidebarView) {
    sidebarView = sidebarView === view ? null : view;
  }

  // ─── Command Palette ──────────────────────────
  let showCommandPalette = $state(false);

  type Cmd = { id: string; label: string; desc?: string; icon?: string; action: () => void };
  let commandItems = $derived<Cmd[]>([
    { id: "palette", label: "Command Palette", desc: "Show all commands", action: () => { showCommandPalette = true; } },
    { id: "term", label: "Toggle Terminal", desc: "Open or focus terminal tab", action: () => { const t = tabs.find(t => t.type === "terminal"); if (t) activeTabId = t.id; else addTab("terminal"); } },
    { id: "sidebar-explorer", label: "Toggle Explorer", desc: "Show/hide file explorer", action: () => toggleSidebar(sidebarView === "files" ? null : "files") },
    { id: "sidebar-search", label: "Toggle Search", desc: "Show/hide search panel", action: () => toggleSidebar(sidebarView === "search" ? null : "search") },
    { id: "sidebar-git", label: "Toggle Source Control", desc: "Show/hide git panel", action: () => { if (sidebarView === "files" && workspaceMode === "git") { sidebarView = null; } else { sidebarView = "files"; workspaceMode = "git"; } } },
    { id: "ai", label: "Toggle AI Chat", desc: "Show/hide floating AI chat panel", action: () => { showFloatingAi = !showFloatingAi; if (showFloatingAi) showFloatingRunner = false; } },
    { id: "runner", label: "Toggle Runner Panel", desc: "Show/hide runner panel", action: () => { showFloatingRunner = !showFloatingRunner; if (showFloatingRunner) showFloatingAi = false; } },
    { id: "open-file", label: "Open File...", desc: "Open a file dialog", action: () => triggerOpenFile() },
    { id: "open-folder", label: "Open Folder...", desc: "Open a folder dialog", action: () => triggerOpenFolder() },
    { id: "settings", label: "Settings", desc: "Open settings tab", action: () => openSettingsTab() },
    { id: "new-term", label: "New Terminal Tab", desc: "Add a new terminal tab", action: () => addTab("terminal") },
    { id: "new-editor", label: "New Editor Tab", desc: "Add a new editor tab", action: () => addTab("file") },
    { id: "close-all", label: "Close All Tabs", desc: "Close all open tabs", action: () => closeAllTabs() },
    { id: "minimize", label: "Minimize Window", action: () => minimizeWindow() },
    { id: "maximize", label: "Toggle Maximize Window", action: () => toggleMaximizeWindow() },
    { id: "close-window", label: "Close Window", action: () => closeWindow() },
  ]);

  // ─── Tab Context Menu ─────────────────────────
  let ctxTabId = $state<string | null>(null);
  let ctxX = $state(0);
  let ctxY = $state(0);
  let ctxOpen = $state(false);
  let contextMenuItems = $derived<{ label: string; icon?: string; danger?: boolean; action: () => void }[]>(ctxTabId ? [
    { label: "Close", icon: `<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>`, action: () => { if (ctxTabId) closeTab(ctxTabId); } },
    { label: "Close Others", action: () => { if (ctxTabId) closeOtherTabs(ctxTabId); } },
    { label: "Close All", action: () => closeAllTabs() },
    { label: "Close to the Right", action: () => { if (!ctxTabId) return; const idx = tabs.findIndex(t => t.id === ctxTabId); const right = tabs.filter((t, i) => i > idx); const dirty = right.filter(t => t.isDirty); if (dirty.length > 0 && !confirm(`Close ${dirty.length} tab(s) with unsaved changes?`)) return; tabs = tabs.filter((t, i) => i <= idx); activeTabId = ctxTabId!; } },
  ] : []);

  function onTabContextMenu(e: MouseEvent, tabId: string) {
    e.preventDefault();
    ctxTabId = tabId;
    ctxX = e.clientX;
    ctxY = e.clientY;
    ctxOpen = true;
  }

  // ─── File Context Menu ─────────────────────────
  let ctxFilePath = $state<string | null>(null);
  let ctxFileX = $state(0);
  let ctxFileY = $state(0);
  let ctxFileOpen = $state(false);

  let fileContextItems = $derived<{ label: string; icon?: string; action: () => void }[]>(ctxFilePath ? [
    {
      label: "Copy Path",
      icon: `<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>`,
      action: () => { navigator.clipboard.writeText(ctxFilePath!).catch(() => {}); addToast("Path copied"); },
    },
    {
      label: "Open in Terminal",
      icon: `<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/></svg>`,
      action: () => {
        const entry = ctxFilePath!;
        const dir = entry.includes(".") ? entry.substring(0, entry.lastIndexOf("\\")) : entry;
        const isWin = navigator.userAgent.toLowerCase().includes("win");
        invoke("pty_write", { sessionId: getActiveTerminalSession(), data: `cd "${dir}"${isWin ? "\r" : "\n"}` }).catch(() => {});
        primaryCwd = dir;
        currentDir.set(dir);
      },
    },
  ] : []);

  function onFileContextMenu(path: string, x: number, y: number) {
    ctxFilePath = path;
    ctxFileX = x;
    ctxFileY = y;
    ctxFileOpen = true;
  }

  // ─── Keyboard Shortcuts ────────────────────────
  function handleGlobalKeydown(e: KeyboardEvent) {
    let shortcuts = {
      sidebar: { ctrl: true, alt: false, shift: false, key: "b" },
      terminal: { ctrl: true, alt: false, shift: false, key: "j" },
      ai: { ctrl: true, alt: true, shift: false, key: "a" },
      runner: { ctrl: true, alt: true, shift: false, key: "n" },
    };
    try {
      const stored = localStorage.getItem("nyxedit-shortcuts");
      if (stored) {
        Object.assign(shortcuts, JSON.parse(stored));
      }
    } catch {}

    const match = (binding: any) => {
      return (
        e.ctrlKey === binding.ctrl &&
        e.altKey === binding.alt &&
        e.shiftKey === binding.shift &&
        e.key.toLowerCase() === binding.key.toLowerCase()
      );
    };

    if (match(shortcuts.sidebar)) {
      e.preventDefault();
      toggleSidebar(sidebarView === "files" ? null : "files");
    } else if (match(shortcuts.terminal)) {
      e.preventDefault();
      const termTab = tabs.find(t => t.type === "terminal");
      if (termTab) {
        activeTabId = termTab.id;
      } else {
        addTab("terminal");
      }
    } else if (match(shortcuts.ai)) {
      e.preventDefault();
      showFloatingAi = !showFloatingAi;
      if (showFloatingAi) showFloatingRunner = false;
    } else if (match(shortcuts.runner)) {
      e.preventDefault();
      showFloatingRunner = !showFloatingRunner;
      if (showFloatingRunner) showFloatingAi = false;
    } else if (e.ctrlKey && e.shiftKey && e.key.toLowerCase() === "p") {
      e.preventDefault();
      showCommandPalette = true;
    } else if (e.ctrlKey && e.shiftKey && e.key.toLowerCase() === "f") {
      e.preventDefault();
      toggleSidebar(sidebarView === "search" ? null : "search");
    } else if (e.ctrlKey && e.key === "Tab") {
      e.preventDefault();
      if (tabs.length < 2) return;
      const idx = tabs.findIndex((t) => t.id === activeTabId);
      const next = e.shiftKey
        ? (idx - 1 + tabs.length) % tabs.length
        : (idx + 1) % tabs.length;
      activeTabId = tabs[next].id;
    }
  }

  // ─── Resize ───────────────────────────────────
  let isResizing = $state(false);
  let resizeStartX = $state(0);
  let resizeStartWidth = $state(260);

  // AI Chat resizability
  let aiWidth = $state(380);
  let aiHeight = $state(440);
  let isAiResizing = $state(false);
  let aiResizeMode = $state<"w" | "n" | "nw" | null>(null);
  let aiResizeStartX = 0;
  let aiResizeStartY = 0;
  let aiResizeStartWidth = 0;
  let aiResizeStartHeight = 0;

  function onResizeStart(e: MouseEvent) {
    isResizing = true;
    resizeStartX = e.clientX;
    resizeStartWidth = sidebarWidth;
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
  }

  function onAiResizeStart(e: MouseEvent, mode: "w" | "n" | "nw") {
    e.preventDefault();
    e.stopPropagation();
    isAiResizing = true;
    aiResizeMode = mode;
    aiResizeStartX = e.clientX;
    aiResizeStartY = e.clientY;
    aiResizeStartWidth = aiWidth;
    aiResizeStartHeight = aiHeight;
    document.body.style.cursor = mode === "nw" ? "nwse-resize" : mode === "w" ? "ew-resize" : "ns-resize";
    document.body.style.userSelect = "none";
  }

  function onResizeMove(e: MouseEvent) {
    if (isResizing) {
      sidebarWidth = Math.max(180, Math.min(480, resizeStartWidth + (e.clientX - resizeStartX)));
    } else if (isAiResizing && aiResizeMode) {
      if (aiResizeMode.includes("w")) {
        const deltaX = aiResizeStartX - e.clientX;
        aiWidth = Math.max(300, Math.min(800, aiResizeStartWidth + deltaX));
      }
      if (aiResizeMode.includes("n")) {
        const deltaY = aiResizeStartY - e.clientY;
        aiHeight = Math.max(350, Math.min(800, aiResizeStartHeight + deltaY));
      }
    }
  }

  function onResizeEnd() {
    isResizing = false;
    isAiResizing = false;
    aiResizeMode = null;
    document.body.style.cursor = "";
    document.body.style.userSelect = "";
  }
</script>

<svelte:window onkeydown={handleGlobalKeydown} onmousemove={onResizeMove} onmouseup={onResizeEnd} onclick={() => { addMenuOpen = false; nyxMenuOpen = false; }} />

<div class="workspace">
  <!-- Custom Background Backdrop -->
  <div class="workspace-backdrop"></div>
  <!-- ═══ TAB BAR ═══ -->
  <header class="tab-bar" onmousedown={handleHeaderMousedown}>
    <!-- Left tools -->
    <div class="tab-bar-left">
      <div class="nyx-menu-container" style="position: relative; display: inline-block; -webkit-app-region: no-drag;">
        <button class="tool-btn nyx-logo-btn" onclick={(e) => { e.stopPropagation(); nyxMenuOpen = !nyxMenuOpen; }} title="Nyx Actions" style="padding: 2px; display: flex; align-items: center; justify-content: center; width: 22px; height: 22px;">
          <img src="/logo.png" alt="Nyx Logo" style="width: 14px; height: 14px; display: inline-block; vertical-align: middle; border-radius: 3px;" />
        </button>
        
        {#if nyxMenuOpen}
          <div class="nyx-dropdown-menu" onclick={(e) => e.stopPropagation()} role="presentation">
            <button class="nyx-dropdown-item" onclick={() => { nyxMenuOpen = false; triggerOpenFile(); }}>
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="margin-right: 6px;"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/></svg>
              <span>Open File</span>
            </button>
            <button class="nyx-dropdown-item" onclick={() => { nyxMenuOpen = false; triggerOpenFolder(); }}>
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="margin-right: 6px;"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>
              <span>Open Folder</span>
            </button>
            <button class="nyx-dropdown-item" onclick={() => { nyxMenuOpen = false; handleOpenWorkspace(); }}>
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="margin-right: 6px;"><path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z"/><polyline points="17 21 17 13 7 13 7 21"/><polyline points="7 3 7 8 15 8"/></svg>
              <span>Open Workspace</span>
            </button>
            <button class="nyx-dropdown-item" onclick={() => { nyxMenuOpen = false; handleSaveWorkspace(); }}>
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="margin-right: 6px;"><path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z"/><polyline points="17 21 17 13 7 13 7 21"/><polyline points="7 3 7 8 15 8"/><line x1="12" y1="13" x2="12" y2="17"/><line x1="10" y1="15" x2="14" y2="15"/></svg>
              <span>Save Workspace</span>
            </button>
            <button class="nyx-dropdown-item" onclick={() => { nyxMenuOpen = false; handleAddFolderToWorkspace(); }}>
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="margin-right: 6px;"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/><line x1="12" y1="11" x2="12" y2="17"/><line x1="9" y1="14" x2="15" y2="14"/></svg>
              <span>Add Folder to Workspace</span>
            </button>
            <hr class="nyx-dropdown-divider"/>
            <button class="nyx-dropdown-item" onclick={() => { nyxMenuOpen = false; openSettingsTab(); }}>
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="margin-right: 6px;"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>
              <span>Settings</span>
            </button>
          </div>
        {/if}
      </div>
      <span class="bar-divider"></span>
      <button class="tool-btn" class:active={sidebarView !== null} onclick={() => toggleSidebar(sidebarView === null ? "files" : null)} title="Explorer">
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"/><line x1="9" y1="3" x2="9" y2="21"/></svg>
      </button>
      <button class="tool-btn" class:active={showFloatingRunner} onclick={() => { showFloatingRunner = !showFloatingRunner; if (showFloatingRunner) showFloatingAi = false; }} title="Runner">
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="5 3 19 12 5 21 5 3"/></svg>
      </button>
      <button class="tool-btn" class:active={showFloatingAi} onclick={() => { showFloatingAi = !showFloatingAi; if (showFloatingAi) showFloatingRunner = false; }} title="AI Chat">
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>
      </button>
    </div>

    <!-- Tabs (wrap) -->
    <div class="tab-wrap">
      {#each tabs as tab (tab.id)}
        <button
          class="tab"
          class:tab-active={activeTabId === tab.id}
          onclick={() => setActiveTab(tab.id)}
          oncontextmenu={(e) => onTabContextMenu(e, tab.id)}
          role="tab"
          aria-selected={activeTabId === tab.id}
        >
          <span class="tab-icon">{@html TAB_ICONS[tab.type]}</span>
          <span class="tab-label">{tab.label}</span>
          {#if tab.isDirty}
            <span class="tab-dirty">&#8226;</span>
          {/if}
          <span class="tab-close" onclick={(e) => { e.stopPropagation(); closeTab(tab.id); }} onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.stopPropagation(); closeTab(tab.id); } }} role="button" tabindex="-1" aria-label="Close">&times;</span>
        </button>
      {/each}
    </div>

    <!-- Right controls -->
    <div class="tab-bar-right">
      <button class="tool-btn" onclick={(e) => { e.stopPropagation(); addMenuOpen = !addMenuOpen; }} aria-label="Add tab" title="New Tab">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
      </button>
      <button class="tool-btn" onclick={closeAllTabs} title="Close All Tabs">
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2"/><line x1="9" y1="9" x2="15" y2="15"/><line x1="15" y1="9" x2="9" y2="15"/></svg>
      </button>
      <span class="bar-divider"></span>
      <div class="win-controls">
        <button class="win-ctrl" onclick={minimizeWindow} aria-label="Minimize" title="Minimize">
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="5" y1="12" x2="19" y2="12"/></svg>
        </button>
        <button class="win-ctrl" onclick={toggleMaximizeWindow} aria-label="Maximize" title="Maximize">
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="5" y="5" width="14" height="14" rx="2"/></svg>
        </button>
        <button class="win-ctrl win-close" onclick={closeWindow} aria-label="Close" title="Close">
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="6" y1="6" x2="18" y2="18"/><line x1="6" y1="18" x2="18" y2="6"/></svg>
        </button>
      </div>
    </div>

  </header>

  <!-- Add menu (outside header to avoid backdrop-filter stacking context) -->
  {#if addMenuOpen}
    <div class="add-menu" onclick={(e) => e.stopPropagation()} role="presentation">
      {#each [
        { type: "terminal", icon: `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/></svg>`, label: "Terminal" },
        { type: "preview", icon: `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="5"/><line x1="12" y1="1" x2="12" y2="3"/><line x1="12" y1="21" x2="12" y2="23"/><line x1="4.22" y1="4.22" x2="5.64" y2="5.64"/><line x1="18.36" y1="18.36" x2="19.78" y2="19.78"/><line x1="1" y1="12" x2="3" y2="12"/><line x1="21" y1="12" x2="23" y2="12"/><line x1="4.22" y1="19.78" x2="5.64" y2="18.36"/><line x1="18.36" y1="5.64" x2="19.78" y2="4.22"/></svg>`, label: "Preview" },
        { type: "file", icon: `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5 2z"/><polyline points="14 2 14 8 20 8"/></svg>`, label: "Editor" },
        { type: "private", icon: `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="11" width="18" height="11" rx="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>`, label: "Private" },
      ] as item}
        <button class="add-menu-item" class:private-item={item.type === "private"} onclick={() => addTab(item.type as TabType)}>
          <span class="add-menu-icon">{@html item.icon}</span>
          <span>{item.label}</span>
          {#if item.type === "private"}
            <span class="private-tag">AI Restricted</span>
          {/if}
        </button>
      {/each}
    </div>
  {/if}

  <!-- ═══ BODY ═══ -->
  <div class="body">
    <!-- Activity Bar -->
    <nav class="activity-bar">
      <div class="activity-top">
        {#each activityViews as view, i}
          <button
            class="activity-btn"
            class:active={sidebarView === view}
            onclick={() => toggleSidebar(view as SidebarView)}
            title={SIDEBAR_LABELS[view]}
            draggable="true"
            ondragstart={(e) => handleIconDragStart(e, i)}
            ondragover={handleIconDragOver}
            ondrop={() => handleIconDrop(i)}
            style="cursor: grab;"
          >
            {#if view === "files"}
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>
            {:else if view === "search"}
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
            {:else if view === "ssh"}
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="2" width="20" height="20" rx="4"/><line x1="6" y1="6" x2="18" y2="6"/><line x1="6" y1="12" x2="18" y2="12"/><line x1="6" y1="18" x2="18" y2="18"/></svg>
            {:else if view === "postman"}
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/></svg>
            {:else if view === "mqtt"}
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="9"/><line x1="12" y1="2" x2="12" y2="22"/><line x1="2" y1="12" x2="22" y2="12"/></svg>
            {:else if view === "platformio"}
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="9"/><path d="M9 12h6M12 9v6"/><path d="M7.5 7.5l9 9M7.5 16.5l9-9"/></svg>
            {:else if view === "extensions"}
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2"/><line x1="9" y1="9" x2="15" y2="15"/><line x1="15" y1="9" x2="9" y2="15"/></svg>
            {:else if view === "database"}
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><ellipse cx="12" cy="5" rx="9" ry="3"/><path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"/><path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"/></svg>
            {:else if view === "intel"}
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2L2 7l10 5 10-5-10-5z"/><path d="M2 17l10 5 10-5"/><path d="M2 12l10 5 10-5"/></svg>
            {/if}
          </button>
        {/each}
      </div>

    </nav>

    <!-- Sidebar -->
    {#if sidebarView !== null}
      <aside class="sidebar" style="width: {sidebarWidth}px">
        <div class="sidebar-header">
          <span class="sidebar-title">{SIDEBAR_LABELS[sidebarView]}</span>
          <button class="sidebar-close" onclick={() => (sidebarView = null)} aria-label="Close">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
          </button>
        </div>
        <div class="sidebar-body">
          {#if sidebarView === "files"}
            <div class="sidebar-workspace-content">
              {#if workspaceMode === "explorer"}
                <FileManager onFileOpen={onFileOpen} revealPath={primaryCwd} onDirChange={(dir: string) => { primaryCwd = dir; currentDir.set(dir); }} onFileContext={onFileContextMenu} />
              {:else if workspaceMode === "git"}
                <GitStatus />
              {:else}
                <SSHExplorer />
              {/if}
            </div>
            
            <!-- Workspace / Git / SSH Toggle Footer with Action Buttons -->
            <div class="sidebar-toggle-footer">
              <div class="toggle-switch">
                <button class:active={workspaceMode === "explorer"} onclick={() => workspaceMode = "explorer"}>Workspace</button>
                <button class:active={workspaceMode === "git"} onclick={() => workspaceMode = "git"}>Git Track</button>
                <button class:active={workspaceMode === "ssh_explorer"} onclick={() => workspaceMode = "ssh_explorer"}>SSH Explorer</button>
              </div>
            </div>
          {:else if sidebarView === "search"}
            <SearchInFiles searchPath={primaryCwd} onFileOpen={onFileOpen} onDirChange={(dir: string) => { primaryCwd = dir; currentDir.set(dir); }} />
          {:else if sidebarView === "ssh"}
            <SSHTree onConnect={openSshSessionTab} />
          {:else if sidebarView === "postman"}
            <PostmanClient isSidebar={true} onOpenRequest={(id: string) => {
              const existing = tabs.find(t => t.type === "api_client" && t.requestId === id);
              if (existing) { activeTabId = existing.id; return; }
              const tabId = "tab-api-" + Date.now().toString(36);
              tabs = [...tabs, { id: tabId, type: "api_client", label: "API Client", requestId: id }];
              activeTabId = tabId;
            }} />
          {:else if sidebarView === "mqtt"}
            <MQTTClient />
          {:else if sidebarView === "platformio"}
            <div class="pio-sidebar">
              <div class="pio-status-bar" style="display: flex; justify-content: space-between; align-items: center; width: 100%; box-sizing: border-box;">
                <div style="display: flex; align-items: center; gap: 8px;">
                  {#if pioState.busy || pioState.installing}
                    <span class="pio-spinner"></span>
                  {/if}
                  <span class="pio-status-text" class:pio-installed={pioState.status.installed} class:pio-not-installed={!pioState.status.installed}>
                    {#if pioState.statusMsg}
                      {pioState.statusMsg}
                    {:else if pioState.status.installed}
                      {pioState.status.version || "PlatformIO ready"}
                    {:else if pioState.status.error}
                      Error checking PlatformIO
                    {:else}
                      Checking...
                    {/if}
                  </span>
                </div>
                <button class="tool-btn" onclick={() => checkPio(true, pioState, addLog)} title="Recheck PlatformIO" style="padding: 2px;">
                  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21.5 2v6h-6M21.34 15.57a10 10 0 1 1-.57-8.38l5.67-5.67"/></svg>
                </button>
              </div>

              {#if !pioState.status.installed && !pioState.busy}
                <div class="pio-section pio-install-section">
                  <div class="pio-install-info">
                    <svg width="32" height="32" viewBox="0 0 48 48" fill="none">
                      <circle cx="24" cy="24" r="22" stroke="currentColor" stroke-width="2" fill="none"/>
                      <path d="M16 28l8-12 8 12" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" fill="none"/>
                      <line x1="24" y1="16" x2="24" y2="36" stroke="currentColor" stroke-width="2"/>
                      <circle cx="24" cy="24" r="6" stroke="currentColor" stroke-width="1.5" fill="none" stroke-dasharray="3 3"/>
                    </svg>
                    <span>PlatformIO is not installed</span>
                    <span class="pio-detail">Python required. Will install via pip.</span>
                    {#if pioState.status.python}
                      <span class="pio-detail pio-py-ok">&#10003; {pioState.status.python}</span>
                    {:else}
                      <span class="pio-detail pio-py-missing">&#10007; Python not found</span>
                    {/if}
                  </div>
                  <button class="pio-install-btn" onclick={() => installPio(pioState, addLog)} disabled={pioState.installing || !pioState.status.python}>
                    {pioState.installing ? "Installing..." : "Install PlatformIO"}
                  </button>
                </div>
              {/if}

              {#if pioState.status.installed}
                <div class="pio-section">
                  <div class="pio-section-title">QUICK ACCESS</div>
                  <button class="pio-item" onclick={() => runPioTarget("build", primaryCwd, pioState, addLog)}>
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polygon points="5 3 19 12 5 21 5 3"/></svg>
                    <span>Build</span>
                  </button>
                  <button class="pio-item" onclick={() => runPioTarget("upload", primaryCwd, pioState, addLog)}>
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="17 8 12 3 7 8"/><line x1="12" y1="3" x2="12" y2="15"/></svg>
                    <span>Upload</span>
                  </button>
                  <button class="pio-item" onclick={() => runPioTarget("clean", primaryCwd, pioState, addLog)}>
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="18" height="18" rx="2"/><line x1="9" y1="9" x2="15" y2="15"/><line x1="15" y1="9" x2="9" y2="15"/></svg>
                    <span>Clean</span>
                  </button>
                  <button class="pio-item" onclick={() => addTab("terminal", { label: "Serial Monitor", initialCommand: "pio device monitor" })}>
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/></svg>
                    <span>Serial Monitor</span>
                  </button>
                </div>

                <div class="pio-section">
                  <div class="pio-section-title">INIT PROJECT</div>
                  <div class="pio-init-row">
                    <input class="pio-init-input" bind:value={pioState.initPath} placeholder={primaryCwd || "Project path..."} />
                    <button class="pio-init-btn" onclick={() => initPioProject(undefined, primaryCwd, pioState, addLog)} disabled={pioState.busy}>Init</button>
                  </div>
                </div>

                <div class="pio-section">
                  <div class="pio-section-title">BOARDS ({pioState.boards.length})</div>
                  <div class="pio-board-search">
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
                    <input type="text" bind:value={pioState.boardSearch} placeholder="Search boards..." class="pio-search-input" />
                  </div>
                  <div class="pio-boards-list">
                    {#each pioFilteredBoards as board}
                      <button class="pio-item pio-board" onclick={() => initPioProjectWithBoard(board, primaryCwd, pioState, addLog)}>\n                        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="4" y="2" width="16" height="20" rx="2"/><line x1="9" y1="6" x2="15" y2="6"/><line x1="12" y1="2" x2="12" y2="6"/><circle cx="9" cy="12" r="1"/><circle cx="15" cy="12" r="1"/><circle cx="9" cy="17" r="1"/><circle cx="15" cy="17" r="1"/></svg>
                        <span>{board}</span>
                      </button>
                    {:else}
                      {#if pioState.boardSearch}
                        <div class="pio-no-results">No boards match "{pioState.boardSearch}"</div>
                      {/if}
                    {/each}
                    {#if !pioState.boardSearch && pioState.boards.length > pioState.boardLimit}
                      <div class="pio-no-results">Showing {pioState.boardLimit} of {pioState.boards.length} boards</div>
                    {/if}
                  </div>
                </div>
              {/if}
            </div>
          {:else if sidebarView === "extensions"}
            <div class="ext-sidebar">
              <div class="ext-input-row">
                <input class="ext-input" bind:value={extUrl} placeholder="Paste URL to extension.json..." disabled={extBusy}
                  onkeydown={(e) => { if (e.key === "Enter") addExtensionFromUrl(extUrl, extensions, (exts) => extensions = exts, addLog, tabs, (id) => activeTabId = id); }} />
                <button class="ext-add-btn" onclick={() => addExtensionFromUrl(extUrl, extensions, (exts) => extensions = exts, addLog, tabs, (id) => activeTabId = id)} disabled={extBusy || !extUrl.trim()}>
                  {extBusy ? "..." : "+"}
                </button>
              </div>
              {#if extMsg}
                <div class="ext-msg">{extMsg}</div>
              {/if}
              <div class="ext-section">
                <div class="ext-section-title">INSTALLED</div>
                {#each extensions.filter(e => e.installed) as ext}
                  <div class="ext-item">
                    <div class="ext-info">
                      <span class="ext-name">{ext.name} <span class="ext-ver">v{ext.version}</span></span>
                      <span class="ext-desc">{ext.description || ext.type}</span>
                    </div>
                    <button class="ext-btn ext-installed" onclick={() => { const r = removeExtension(ext.id, extensions, (exts) => extensions = exts, addLog); extMsg = r; }}>Remove</button>
                  </div>
                {:else}
                  <div class="ext-empty">No extensions installed.<br/>Paste a URL above.</div>
                {/each}
              </div>
            </div>
          {:else if sidebarView === "database"}
            <DatabaseClient isSidebar={true} onOpenQuery={(connId: string, label: string) => openDbQueryTab(connId, label)} />
          {:else if sidebarView === "intel"}
            <IntelPanel />
          {/if}
        </div>
        <div class="sidebar-resize" onmousedown={onResizeStart} role="presentation"></div>
      </aside>
    {/if}

    <!-- Workspace -->
    <main class="workspace-area">
      {#each tabs as tab (tab.id)}
        <div class="tab-panel" class:hidden={activeTabId !== tab.id} class:terminal-tab={tab.type === 'terminal' || tab.type === 'private'}>
          {#if tab.type === "terminal"}
            <SplitTerminal cwd={primaryCwd} onCwdChange={onTerminalCwdChange} initialCommand={tab.initialCommand} />
          {:else if tab.type === "file"}
            {#if tab.filePath && tab.fileContent !== undefined}
              <ViewerRouter
                filePath={tab.filePath}
                fileContent={tab.fileContent ?? ""}
                onSave={makeOnSaveForTab(tab.id)}
                onDirtyChange={(d: boolean) => { if (d) markTabDirty(tab.id); else { const t = tabs.find(tt => tt.id === tab.id); if (t) t.isDirty = false; }}}
              />
            {:else if tab.filePath}
              <ViewerRouter filePath={tab.filePath} fileContent="" onSave={makeOnSaveForTab(tab.id)} />
            {:else}
              <div class="placeholder"><p>Open a file from <strong>Explorer</strong></p></div>
            {/if}
          {:else if tab.type === "preview"}
            <div class="preview-wrap">
              {#if tab.previewUrl}
                {@const safeUrl = normalizeUrl(tab.previewUrl)}
                {@const isLocal = safeUrl.includes("localhost") || safeUrl.includes("127.0.0.1") || safeUrl.includes("192.168.") || safeUrl.includes("10.")}
                {@const proxyUrl = proxyPort > 0 ? `http://localhost:${proxyPort}/proxy?url=${encodeURIComponent(safeUrl)}` : ""}
                <div class="preview-toolbar">
                  <input class="preview-input" bind:value={tab.previewUrl} placeholder="https://..." />
                  <button class="preview-open-btn" onclick={() => openShell(safeUrl)} title="Open in Browser">
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="2" y1="12" x2="22" y2="12"/><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/></svg>
                  </button>
                </div>
                <iframe class="preview-frame" src={isLocal ? safeUrl : proxyUrl} title="Preview" sandbox="allow-scripts allow-same-origin allow-forms"></iframe>
              {:else}
                <div class="placeholder">
                  <p>Enter a URL to preview</p>
                  <input class="preview-start-input" placeholder="https://..." onkeydown={(e) => { if (e.key === "Enter") { tab.previewUrl = normalizeUrl((e.target as HTMLInputElement).value); } }} />
                </div>
              {/if}
            </div>
          {:else if tab.type === "settings"}
            <Settings />
          {:else if tab.type === "ssh_session"}
            <SSHSession
              profile={tab.sshProfile}
              onOpenFile={onRemoteFileOpen}
              onClose={() => { tabs = tabs.filter(t => t.id !== tab.id); if (activeTabId === tab.id) activeTabId = tabs[0]?.id ?? ""; }}
            />
          {:else if tab.type === "private"}
            <div class="private-terminal-wrap">
              <div class="private-badge">
                <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="11" width="18" height="11" rx="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>
                AI Restricted — AI cannot access this terminal
              </div>
              <SplitTerminal
                cwd={primaryCwd}
                onCwdChange={onTerminalCwdChange}
                onSessionCreated={(sid: string) => markSessionPrivate(sid)}
              />
            </div>
          {:else if tab.type === "api_client"}
            <PostmanClient isSidebar={false} activeRequestId={tab.requestId ?? null} onOpenRequest={() => {}} />
          {:else if tab.type === "db_query"}
            <DatabaseClient isSidebar={false} activeConnectionId={tab.connectionId ?? null} onOpenQuery={() => {}} />
          {:else if tab.type === "diff"}
            {#if tab.diffFiles}
              <AIDiffTab diffFiles={tab.diffFiles} onCloseTab={() => { tabs = tabs.filter(t => t.id !== tab.id); if (activeTabId === tab.id) activeTabId = tabs[0]?.id ?? ""; }} />
            {/if}
          {:else if tab.type === "ai_chat"}
            <AIChat
              onOpenDiff={handleOpenDiffTab}
              onMinimize={() => { tabs = tabs.filter(t => t.id !== tab.id); if (activeTabId === tab.id) activeTabId = tabs[0]?.id ?? ""; }}
              onClose={() => { tabs = tabs.filter(t => t.id !== tab.id); if (activeTabId === tab.id) activeTabId = tabs[0]?.id ?? ""; }}
              onOpenInTab={handleOpenAiInTab}
            />
          {/if}
        </div>
      {/each}
      {#if tabs.length === 0}
        <div class="placeholder"><p>Select a tab</p></div>
      {/if}
    </main>
  </div>

  <!-- ═══ STATUS BAR ═══ -->
  <footer class="status-bar">
    <div class="sb-left">
      <button class="sb-btn" class:active={showFloatingRunner} onclick={() => { showFloatingRunner = !showFloatingRunner; if (showFloatingRunner) showFloatingAi = false; }} title="Runner Panel" style="margin-right: 6px;">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polygon points="6 3 20 12 6 21 6 3"/></svg>
      </button>

      {#if activeFramework && activeFramework !== "Unknown"}
        <span class="sb-framework-tag" title="Project Framework: {activeFramework}">
          <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="margin-right: 3px;"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>
          {activeFramework}
        </span>
        <span class="path-sep" style="margin: 0 4px; color: var(--text-muted); opacity: 0.5;">|</span>
      {/if}

      {#if primaryCwd}
        <div class="path-breadcrumb">
          {#each primaryCwd.split("\\") as seg, i}
            {#if i > 0}<span class="path-sep">&gt;</span>{/if}
            <span class="path-seg" class:path-seg-last={i === primaryCwd.split("\\").length - 1}>{seg}</span>
          {/each}
        </div>
      {:else}
        <span class="sb-muted">No directory</span>
      {/if}

      {#if primaryCwd}
        <span class="path-sep" style="margin: 0 4px; color: var(--text-muted); opacity: 0.5;">|</span>
        <span class="sb-graph-status" class:sb-graph-synced={$graphStatus === 'Synced'} class:sb-graph-syncing={$graphStatus === 'Syncing'} class:sb-graph-error={$graphStatus === 'Error'} title="Knowledge Graph Status: {$graphStatus}">
          <span class="status-dot"></span>
          Graph: {$graphStatus}
        </span>
      {/if}

      {#if isIndexingGraph}
        <div class="sb-indexing-bar" title="Indexing file: {indexCurrentFile}">
          <span class="sb-spinner"></span>
          <span>Indexing: {Math.round(indexProgress)}% ({indexCurrentIndex}/{indexTotalFiles})</span>
        </div>
      {/if}
    </div>

    <div class="sb-center" onclick={() => (showLogs = !showLogs)} role="button" tabindex="0" onkeydown={(e) => e.key === "Enter" && (showLogs = !showLogs)} title="Toggle logs" style="max-width: 40%; display: flex; align-items: center; gap: 8px;">
      <span class="sb-clock" style="font-weight: 700;">{fmtTime(now)}</span>
      <span class="sb-log-badge" class:sb-log-has-error={logs.some(l => l.type === "error")}>{logs.length}</span>
      {#if logs.length > 0}
        <span class="sb-log-thin-preview" style="font-size: var(--fs-9-5); color: var(--text-muted); opacity: 0.6; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 250px;" title="Latest log: {logs[logs.length - 1].msg}">
          | {logs[logs.length - 1].msg}
        </span>
      {/if}
    </div>

    <div class="sb-right">
      {#if agentSwarmStatus && agentSwarmStatus !== "Idle"}
        <span class="sb-swarm-badge" title="Agent Swarm Status: {agentSwarmStatus}">
          <span class="swarm-pulse"></span>
          <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="display: inline-block; vertical-align: middle; margin-right: 4px; animation: spin 4s linear infinite;"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>
          Swarm: {agentSwarmStatus}
        </span>
      {/if}

      <!-- Internet Connection and GitHub Latency Indicator -->
      <span class="sb-net-badge" style="display: inline-flex; align-items: center; gap: 4px; padding: 2px 6px; border-radius: 4px; font-size: var(--fs-9-5); background: rgba(255, 255, 255, 0.03); border: 1px solid var(--border-subtle); color: var(--text-muted);" title={isOnline ? `Online (Latency to GitHub: ${pingGithub !== null ? pingGithub + 'ms' : 'calculating...'})` : 'Offline'}>
        <svg width="12" height="9" viewBox="0 0 16 12" style="display: inline-block; vertical-align: middle;">
          <rect x="1" y="9" width="2" height="3" rx="0.5" fill={signalBars >= 1 ? 'var(--accent-green)' : 'var(--text-muted)'} opacity={signalBars >= 1 ? 1 : 0.35} />
          <rect x="5" y="6" width="2" height="6" rx="0.5" fill={signalBars >= 2 ? 'var(--accent-green)' : 'var(--text-muted)'} opacity={signalBars >= 2 ? 1 : 0.35} />
          <rect x="9" y="3" width="2" height="9" rx="0.5" fill={signalBars >= 3 ? 'var(--accent-green)' : 'var(--text-muted)'} opacity={signalBars >= 3 ? 1 : 0.35} />
          <rect x="13" y="0" width="2" height="12" rx="0.5" fill={signalBars >= 4 ? 'var(--accent-green)' : 'var(--text-muted)'} opacity={signalBars >= 4 ? 1 : 0.35} />
        </svg>
        {#if isOnline}
          <span style="color: var(--text-muted); font-family: monospace; font-size: var(--fs-9-5);">{pingGithub !== null ? `${pingGithub}ms` : 'checking...'}</span>
        {:else}
          <span style="color: var(--accent-red); font-weight: bold; font-size: var(--fs-9-5);">Offline</span>
        {/if}
      </span>

      <!-- Total Cost Badge for all Registered AI agents -->
      {#if totalAllAiCost > 0}
        <span class="sb-budget-badge" style="background: color-mix(in srgb, var(--accent-purple) 15%, transparent); color: var(--accent-purple); border: 1px solid color-mix(in srgb, var(--accent-purple) 25%, transparent);" title="Total Cost (All Registered AIs: {registeredAgentsCount} agents): ${totalAllAiCost.toFixed(5)}">
          <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="display: inline-block; vertical-align: middle; margin-right: 3px;"><rect x="3" y="11" width="18" height="10" rx="2"/><circle cx="12" cy="5" r="2"/><path d="M12 7v4"/><line x1="8" y1="15" x2="8" y2="15"/><line x1="16" y1="15" x2="16" y2="15"/></svg>
          ${totalAllAiCost.toFixed(4)}
        </span>
      {/if}

      {#if $costSummary && $costSummary.limit > 0}
        <span class="sb-budget-badge" class:sb-budget-high={$costSummary.total_cost >= $costSummary.limit * 0.9} title="Total Cost: ${$costSummary.total_cost.toFixed(4)} / Limit: ${$costSummary.limit.toFixed(2)}">
          <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="display: inline-block; vertical-align: middle; margin-right: 3px;"><line x1="12" y1="1" x2="12" y2="23"/><path d="M17 5H9.5a3.5 3.5 0 0 0 0 7h5a3.5 3.5 0 0 1 0 7H6"/></svg>
          ${$costSummary.total_cost.toFixed(3)}
        </span>
      {/if}

      <span class="sb-health-badge" class:sb-health-degraded={!$systemHealth.ok} onclick={() => { sidebarView = "files"; workspaceMode = "explorer"; activeTabId = "tab-settings"; let hasSettings = tabs.some(t => t.id === "tab-settings"); if (!hasSettings) { tabs = [...tabs, { id: "tab-settings", type: "settings", label: "Settings" }]; } }} role="button" tabindex="0" onkeydown={(e) => e.key === "Enter" && (activeTabId = "tab-settings")} title="System Health: {$systemHealth.status || 'Healthy'}">
        <svg width="11" height="11" viewBox="0 0 24 24" fill={ $systemHealth.ok ? "var(--accent-green)" : "var(--accent-red)" } stroke={ $systemHealth.ok ? "var(--accent-green)" : "var(--accent-red)" } stroke-width="2" style="display: inline-block; vertical-align: middle; margin-right: 3px;"><path d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z"/></svg>
        Health
      </span>

      {#if degradedCount > 0}
        <button type="button" class="sb-degraded-badge" onclick={() => { sidebarView = "files"; workspaceMode = "explorer"; activeTabId = "tab-settings"; let hasSettings = tabs.some(t => t.id === "tab-settings"); if (!hasSettings) { tabs = [...tabs, { id: "tab-settings", type: "settings", label: "Settings" }]; } }} style="background: var(--accent-red); color: white; padding: 2px 6px; border: none; border-radius: 4px; font-weight: bold; margin-right: 8px; font-size: var(--fs-9); cursor: pointer; display: inline-flex; align-items: center; gap: 4px;" title="{degradedCount} component(s) degraded/down. Click to open Settings.">
          ⚠️ {degradedCount} DEGRADED
        </button>
      {/if}
      <button class="sb-btn" class:active={showFloatingAi} onclick={() => { showFloatingAi = !showFloatingAi; if (showFloatingAi) showFloatingRunner = false; }} title="AI Chat">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 2a4 4 0 0 1 4 4c0 2-2 4-4 4s-4-2-4-4a4 4 0 0 1 4-4z"/><path d="M16 14h.2a4 4 0 0 1 3.8 2.8l.8 2.2H3.2l.8-2.2A4 4 0 0 1 7.8 14H8"/></svg>
      </button>
    </div>
  </footer>

  <!-- Floating AI Chat -->
  <div class="float-panel float-ai" class:hidden={!showFloatingAi} style="width: {aiWidth}px; height: {aiHeight}px;">
    <!-- Resize Handles -->
    <div class="ai-resize-handle ai-resize-w" onmousedown={(e) => onAiResizeStart(e, "w")} role="presentation"></div>
    <div class="ai-resize-handle ai-resize-n" onmousedown={(e) => onAiResizeStart(e, "n")} role="presentation"></div>
    <div class="ai-resize-handle ai-resize-nw" onmousedown={(e) => onAiResizeStart(e, "nw")} role="presentation"></div>
    
    <div class="float-body"><AIChat
      bind:this={aiChatRef}
      onOpenDiff={handleOpenDiffTab}
      onMinimize={minimizeAiChat}
      onClose={closeAiChat}
      onFull={openAiChatTab}
      onOpenInTab={handleOpenAiInTab}
    /></div>
  </div>

  <!-- Floating Runner -->
  {#if showFloatingRunner}
    <div class="float-panel float-runner">
      <div class="float-header">
        <span>Runner Panel</span>
        <button class="float-close" onclick={() => (showFloatingRunner = false)} aria-label="Close">&times;</button>
      </div>
      <div class="float-body"><Runner /></div>
    </div>
  {/if}

  <!-- Floating Logs -->
  {#if showLogs}
    <div class="float-panel float-logs">
      <div class="float-header">
        <span>App Logs</span>
        <div class="float-header-actions">
          <button class="float-btn" onclick={() => (logs = [])} title="Clear">Clear</button>
          <button class="float-close" onclick={() => (showLogs = false)} aria-label="Close">&times;</button>
        </div>
      </div>
      <div class="float-body">
        <div class="logs-list">
          {#each [...logs].reverse() as log}
            <div class="log-entry" class:log-error={log.type === "error"}>
              <span class="log-time">{log.time}</span>
              <span class="log-msg">{log.msg}</span>
            </div>
          {:else}
            <div class="log-empty">No logs yet</div>
          {/each}
        </div>
      </div>
    </div>
  {/if}

  <!-- Floating AI Input Bar -->
  <div class="ai-floating-bar-container">
    <AIFloatingBar />
  </div>

  <!-- ═══ GLOBAL COMPONENTS ═══ -->
  <Toast />
  <ContextMenu open={ctxOpen} x={ctxX} y={ctxY} items={contextMenuItems} onclose={() => (ctxOpen = false)} />
  <ContextMenu open={ctxFileOpen} x={ctxFileX} y={ctxFileY} items={fileContextItems} onclose={() => (ctxFileOpen = false)} />
  <CommandPalette open={showCommandPalette} commands={commandItems} onclose={() => (showCommandPalette = false)} />
</div>

<style>
  .ai-floating-bar-container {
    z-index: 1000;
    pointer-events: none;
  }
  .ai-floating-bar-container :global(> *) {
    pointer-events: auto;
  }

  .workspace { display:flex; flex-direction:column; height:100vh; width:100vw; background:var(--bg-primary); color:var(--text-primary); overflow:hidden; }

  /* ═══ TAB BAR ═══ */
  .tab-bar {
    display:flex; align-items:center; min-height:34px; background:var(--glass-bg, var(--bg-secondary));
    border-bottom:1px solid var(--glass-border, var(--border-primary)); flex-shrink:0; user-select:none; padding:2px 4px; gap:4px; position:relative;
    z-index:1005;
    backdrop-filter: blur(var(--glass-blur, 12px));
    -webkit-backdrop-filter: blur(var(--glass-blur, 12px));
  }
  .tab-bar-left, .tab-bar-right {
    display:flex; align-items:center; gap:2px; flex-shrink:0; -webkit-app-region:no-drag;
  }
  .bar-divider { width:1px; height:16px; background:var(--border-primary); margin:0 2px; }
  .tool-btn {
    border:none; background:none; color:var(--text-muted); cursor:pointer;
    display:flex; align-items:center; justify-content:center;
    padding:4px; border-radius:4px; transition:all 0.15s ease;
  }
  .tool-btn:hover { color:var(--text-primary); background:var(--bg-hover); }
  .tool-btn.active { color:var(--accent-blue); background:color-mix(in srgb, var(--accent-blue) 12%, transparent); }

  /* Window controls (ghost SVG buttons) */
  .win-controls { display:flex; align-items:center; gap:4px; -webkit-app-region:no-drag; }
  .win-ctrl {
    display:inline-flex; align-items:center; justify-content:center;
    width:28px; height:20px; border:none; background:transparent;
    color:var(--text-muted); cursor:pointer; border-radius:4px;
    transition:all 0.15s ease; -webkit-app-region:no-drag;
  }
  .win-ctrl:hover { color:var(--text-primary); background:var(--bg-hover); }
  .win-close:hover { color:var(--accent-red); background:color-mix(in srgb, var(--accent-red) 12%, transparent); }

  /* Tab wrap — horizontal scroll, hidden scrollbar (seperti Brave) */
  .tab-wrap {
    display:flex; flex-wrap:nowrap; align-items:center; gap:2px; flex:1; min-width:0;
    overflow-x:auto; white-space:nowrap; scrollbar-width:none; -ms-overflow-style:none;
    -webkit-app-region:no-drag;
  }
  .tab-wrap::-webkit-scrollbar { display:none; }
  .tab {
    display:inline-flex; align-items:center; gap:4px;
    padding:3px 8px 3px 6px; height:24px;
    font-size:var(--fs-11); font-weight:500; cursor:pointer; white-space:nowrap;
    background:var(--bg-surface); color:var(--text-muted);
    border:1px solid var(--border-subtle);
    border-radius:4px;
    transition:all 0.12s ease;
    user-select:none; -webkit-app-region:no-drag;
  }
  .tab:hover { color:var(--text-primary); background:var(--bg-hover); border-color:var(--border-primary); }
  .tab-active { color:var(--text-primary); background:var(--bg-primary); border-color:var(--accent-blue); }
  .tab-icon { font-size:var(--font-size); line-height:1; display:flex; }
  .tab-label { font-size:var(--fs-11); max-width:100px; overflow:hidden; text-overflow:ellipsis; }
  .tab-dirty { color:var(--accent-blue); font-size:var(--fs-14); line-height:1; }
  .tab-close {
    display:inline-flex; align-items:center; justify-content:center;
    width:15px; height:15px; font-size:var(--fs-13); line-height:1;
    opacity:0; border-radius:3px; transition:all 0.1s ease;
    color:var(--text-muted);
  }
  .tab:hover .tab-close, .tab-active .tab-close { opacity:0.6; }
  .tab-close:hover { opacity:1 !important; color:var(--accent-red); background:var(--bg-hover); }

  /* Add menu dropdown */
  .add-menu {
    position:fixed; top:38px; right:50px;
    background:var(--bg-elevated); border:1px solid var(--border-primary);
    border-radius:8px; padding:4px; z-index:9999;
    box-shadow:0 8px 24px rgba(0,0,0,0.4); min-width:140px;
    animation:floatUp 0.15s ease;
    -webkit-app-region: no-drag;
  }
  .add-menu-item {
    display:flex; align-items:center; gap:8px; width:100%;
    padding:6px 10px; border:none; background:transparent;
    color:var(--text-primary); font-size:var(--font-size); cursor:pointer;
    border-radius:5px; transition:all 0.1s ease;
  }
  .add-menu-item:hover { background:var(--bg-hover); }
  .add-menu-icon { display:flex; }

  /* ═══ BODY ═══ */
  .body { flex:1; display:flex; overflow:hidden; }

  .activity-bar { width:var(--activity-bar-width); display:flex; flex-direction:column; align-items:center; background:var(--bg-secondary); border-right:1px solid var(--border-primary); padding:4px 0; flex-shrink:0; gap:2px; }
  .activity-top { display:flex; flex-direction:column; align-items:center; gap:2px; flex:1; }
  .activity-btn { display:flex; align-items:center; justify-content:center; width:30px; height:30px; background:none; border:none; color:var(--text-muted); cursor:pointer; border-radius:6px; transition:all 0.15s ease; position:relative; }
  .activity-btn:hover { color:var(--text-primary); background:var(--bg-hover); }
  .activity-btn.active { color:var(--accent-blue); }
  .activity-btn.active::before { content:''; position:absolute; left:-5px; top:50%; transform:translateY(-50%); width:3px; height:20px; background:var(--accent-blue); border-radius:0 3px 3px 0; }

  .sidebar { display:flex; flex-direction:column; background:var(--bg-secondary); border-right:1px solid var(--border-primary); overflow:hidden; flex-shrink:0; position:relative; animation:slideIn 0.15s ease; }
  @keyframes slideIn { from { opacity:0; transform:translateX(-8px); } to { opacity:1; transform:translateX(0); } }
  .sidebar-header { display:flex; align-items:center; justify-content:space-between; padding:6px 12px; border-bottom:1px solid var(--border-subtle); flex-shrink:0; }
  .sidebar-title { font-size:var(--fs-10); font-weight:600; color:var(--text-muted); text-transform:uppercase; letter-spacing:0.8px; }
  .sidebar-close { background:none; border:none; color:var(--text-muted); padding:2px; cursor:pointer; border-radius:3px; display:flex; }
  .sidebar-close:hover { color:var(--text-primary); background:var(--bg-hover); }
  .sidebar-body { flex:1; display:flex; flex-direction:column; overflow:hidden; min-width:0; }
  .sidebar-resize { position:absolute; top:0; right:-3px; width:6px; height:100%; cursor:col-resize; z-index:10; }
  .sidebar-resize:hover, .sidebar-resize:active { background:var(--accent-blue); opacity:0.4; }

  /* Sidebar Bottom Toggle Switch */
  .sidebar-workspace-content {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
  }
  .sidebar-toggle-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 8px;
    background: var(--bg-surface);
    border-top: 1px solid var(--border-subtle);
    gap: 6px;
    flex-shrink: 0;
  }
  .toggle-switch {
    display: flex;
    background: var(--bg-primary);
    border: 1px solid var(--border-subtle);
    border-radius: 6px;
    padding: 2px;
    flex: 1;
  }
  .toggle-switch button {
    flex: 1;
    background: transparent;
    border: none;
    color: var(--text-muted);
    font-size: var(--fs-9-5);
    padding: 2px 4px;
    cursor: pointer;
    border-radius: 4px;
    font-weight: 500;
    transition: all 0.1s ease;
  }
  .toggle-switch button.active {
    background: var(--accent-blue);
    color: var(--bg-primary);
    font-weight: 600;
  }
  .git-footer-actions {
    display: flex;
    gap: 3px;
  }
  .git-footer-actions button {
    background: none;
    border: 1px solid var(--border-subtle);
    color: var(--text-muted);
    padding: 3px;
    border-radius: 4px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.1s ease;
  }
  .git-footer-actions button:hover {
    color: var(--text-primary);
    border-color: var(--accent-blue);
    background: var(--bg-hover);
  }

  /* Platform IO */
  .pio-sidebar { display:flex; flex-direction:column; height:100%; overflow-y:auto; font-size:var(--font-size); }
  .pio-status-bar { display:flex; align-items:center; gap:8px; padding:8px 12px; border-bottom:1px solid var(--border-subtle); }
  .pio-status-text { font-size:var(--fs-11); font-weight:500; }
  .pio-installed { color:var(--accent-green); }
  .pio-not-installed { color:var(--accent-yellow); }
  .pio-spinner { width:12px; height:12px; border:2px solid var(--border-subtle); border-top-color:var(--accent-blue); border-radius:50%; animation:pioSpin 0.6s linear infinite; }
  @keyframes pioSpin { to{transform:rotate(360deg)} }
  .pio-section { padding:8px 0; border-bottom:1px solid var(--border-subtle); }
  .pio-section:last-child { border-bottom:none; }
  .pio-section-title { font-size:var(--fs-10); font-weight:600; color:var(--text-muted); text-transform:uppercase; letter-spacing:0.8px; padding:0 12px 6px; }
  .pio-item { display:flex; align-items:center; gap:8px; width:100%; padding:5px 12px; border:none; background:none; color:var(--text-primary); font-size:var(--font-size); cursor:pointer; transition:all 0.1s ease; }
  .pio-item:hover { background:var(--bg-hover); color:var(--accent-blue); }
  .pio-board { font-size:var(--fs-11); color:var(--text-secondary); }
  .pio-board-search { display:flex; align-items:center; gap:6px; padding:2px 12px 6px; color:var(--text-muted); }
  .pio-search-input { flex:1; background:var(--bg-surface); color:var(--text-primary); border:1px solid var(--border-subtle); border-radius:4px; padding:3px 6px; font-size:var(--fs-10); min-width:0; }
  .pio-search-input:focus { outline:none; border-color:var(--accent-blue); }
  .pio-no-results { padding:8px 12px; font-size:var(--fs-10); color:var(--text-muted); text-align:center; font-style:italic; }
  /* Install section */
  .pio-install-section { display:flex; flex-direction:column; gap:10px; padding:16px 12px; align-items:center; }
  .pio-install-info { display:flex; flex-direction:column; align-items:center; gap:6px; text-align:center; color:var(--text-secondary); font-size:var(--font-size); }
  .pio-detail { font-size:var(--fs-10); color:var(--text-muted); }
  .pio-py-ok { color:var(--accent-green); }
  .pio-py-missing { color:var(--accent-red); }
  .pio-install-btn { padding:7px 20px; border:1.5px solid var(--accent-blue); border-radius:6px; background:transparent; color:var(--accent-blue); font-size:var(--font-size); cursor:pointer; transition:all 0.15s ease; }
  .pio-install-btn:hover:not(:disabled) { background:var(--accent-blue); color:#fff; }
  .pio-install-btn:disabled { opacity:0.5; cursor:default; }
  /* Init project */
  .pio-init-row { display:flex; gap:4px; padding:0 12px; }
  .pio-init-input { flex:1; background:var(--bg-surface); border:1px solid var(--border-subtle); border-radius:4px; padding:4px 8px; font-size:var(--fs-11); color:var(--text-primary); font-family:monospace; min-width:0; }
  .pio-init-input:focus { outline:none; border-color:var(--accent-blue); }
  .pio-init-btn { padding:4px 12px; border:1px solid var(--accent-blue); border-radius:4px; background:transparent; color:var(--accent-blue); font-size:var(--fs-11); cursor:pointer; transition:all 0.15s ease; flex-shrink:0; }
  .pio-init-btn:hover:not(:disabled) { background:var(--accent-blue); color:#fff; }
  .pio-init-btn:disabled { opacity:0.5; cursor:default; }
  .pio-boards-list { max-height:280px; overflow-y:auto; }

  /* Extensions sidebar */
  .ext-sidebar { display:flex; flex-direction:column; height:100%; overflow-y:auto; font-size:var(--font-size); padding:8px; gap:6px; }
  .ext-input-row { display:flex; gap:4px; }
  .ext-input { flex:1; background:var(--bg-surface); color:var(--text-primary); border:1px solid var(--border-subtle); border-radius:4px; padding:5px 8px; font-size:var(--fs-11); min-width:0; }
  .ext-input:focus { outline:none; border-color:var(--accent-blue); }
  .ext-add-btn { padding:5px 12px; border:1px solid var(--accent-blue); border-radius:4px; background:transparent; color:var(--accent-blue); font-size:var(--fs-14); cursor:pointer; transition:all 0.15s ease; flex-shrink:0; line-height:1; }
  .ext-add-btn:hover:not(:disabled) { background:var(--accent-blue); color:#fff; }
  .ext-add-btn:disabled { opacity:0.5; cursor:default; }
  .ext-msg { font-size:var(--fs-10); color:var(--accent-blue); padding:2px 0; }
  .ext-section { display:flex; flex-direction:column; gap:4px; }
  .ext-section-title { font-size:var(--fs-10); font-weight:600; color:var(--text-muted); text-transform:uppercase; letter-spacing:0.8px; padding:4px 0; }
  .ext-item { display:flex; align-items:center; justify-content:space-between; gap:8px; padding:7px 10px; background:var(--bg-surface); border:1px solid var(--border-subtle); border-radius:6px; transition:all 0.1s ease; }
  .ext-item:hover { border-color:var(--border-primary); }
  .ext-info { display:flex; flex-direction:column; gap:2px; min-width:0; flex:1; }
  .ext-name { font-size:var(--fs-11); font-weight:600; color:var(--text-primary); }
  .ext-ver { font-size:var(--fs-9); color:var(--text-muted); font-weight:400; }
  .ext-desc { font-size:var(--fs-10); color:var(--text-muted); white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }
  .ext-btn { padding:3px 10px; border:1px solid var(--accent-blue); border-radius:4px; background:transparent; color:var(--accent-blue); font-size:var(--fs-10); cursor:pointer; transition:all 0.15s ease; flex-shrink:0; }
  .ext-btn:hover { background:var(--accent-blue); color:#fff; }
  .ext-btn.ext-installed { border-color:var(--accent-red); color:var(--accent-red); }
  .ext-btn.ext-installed:hover { background:var(--accent-red); color:#fff; }
  .ext-empty { padding:16px; text-align:center; color:var(--text-muted); font-size:var(--fs-11); }

  .workspace-area { flex:1; display:flex; flex-direction:column; overflow:hidden; min-width:200px; }
  .tab-panel { display: flex; flex-direction: column; width: 100%; height: 100%; }
  .tab-panel.hidden { display: none !important; }
  .tab-panel.hidden.terminal-tab { display: flex !important; position: absolute; left: -9999px; top: -9999px; visibility: hidden; pointer-events: none; }
  .placeholder { display:flex; flex-direction:column; align-items:center; justify-content:center; gap:12px; height:100%; color:var(--text-muted); font-size:var(--fs-13); }
  .placeholder strong { color:var(--accent-blue); }

  /* Preview */
  .preview-wrap { display:flex; flex-direction:column; height:100%; }
  .preview-toolbar { display:flex; align-items:center; gap:6px; padding:6px 10px; border-bottom:1px solid var(--border-subtle); flex-shrink:0; }
  .preview-input { flex:1; background:var(--bg-surface); color:var(--text-primary); border:1px solid var(--border-subtle); border-radius:5px; padding:5px 8px; font-size:var(--font-size); font-family:monospace; }
  .preview-input:focus { outline:none; border-color:var(--accent-blue); }
  .preview-open-btn { display:flex; align-items:center; justify-content:center; width:28px; height:28px; border:1px solid var(--border-subtle); border-radius:5px; background:transparent; color:var(--text-muted); cursor:pointer; flex-shrink:0; transition:all 0.12s ease; }
  .preview-open-btn:hover { color:var(--accent-blue); border-color:var(--accent-blue); background:var(--bg-hover); }
  .preview-external-block { flex:1; display:flex; flex-direction:column; align-items:center; justify-content:center; gap:10px; color:var(--text-muted); }
  .preview-external-block p { font-size:var(--fs-12); margin:0; }
  .preview-external-btn { padding:6px 18px; border:1px solid var(--accent-blue); border-radius:6px; background:var(--accent-blue); color:var(--bg-primary); font-size:var(--fs-11); font-weight:600; cursor:pointer; transition:all 0.12s ease; }
  .preview-external-btn:hover { filter:brightness(1.15); }
  .preview-frame { flex:1; border:none; background:#fff; }
  .preview-start-input { background:var(--bg-surface); color:var(--text-primary); border:1px solid var(--border-subtle); border-radius:5px; padding:6px 10px; font-size:var(--fs-13); font-family:monospace; width:280px; }
  .preview-start-input:focus { outline:none; border-color:var(--accent-blue); }

  /* ═══ STATUS BAR ═══ */
  .status-bar { display:flex; align-items:center; justify-content:space-between; height:var(--status-bar-height); padding:0 8px; background:var(--bg-secondary); border-top:1px solid var(--border-primary); font-size:var(--fs-11); color:var(--text-muted); flex-shrink:0; user-select:none; }
  .sb-left { display:flex; align-items:center; gap:4px; flex:1; min-width:0; overflow-x:auto; scrollbar-width:none; }
  .path-breadcrumb { display:flex; align-items:center; gap:0; }
  .path-seg { font-size:var(--fs-11); color:var(--text-muted); padding:1px 5px; border:1px solid var(--border-subtle); border-radius:3px; white-space:nowrap; background:var(--bg-surface); margin:0 1px; }
  .path-seg:hover { border-color:var(--accent-blue); color:var(--text-secondary); }
  .path-seg-last { color:var(--accent-blue); border-color:var(--accent-blue); background:color-mix(in srgb, var(--accent-blue) 8%, transparent); font-weight:500; }
  .path-sep { color:var(--text-muted); font-size:var(--fs-10); font-weight:700; margin:0 2px; }
  .sb-muted { color:var(--text-muted); font-style:italic; }
  .sb-center { display:flex; align-items:center; justify-content:center; gap:6px; cursor:pointer; padding:0 6px; border-radius:4px; transition:all 0.12s ease; }
  .sb-center:hover { background:var(--bg-hover); }
  .sb-clock { font-family:monospace; font-size:var(--fs-11); letter-spacing:0.5px; }
  .sb-log-badge { font-size:var(--fs-9); background:var(--bg-elevated); color:var(--text-muted); padding:0 5px; border-radius:8px; line-height:14px; font-weight:600; }
  .sb-log-badge.sb-log-has-error { background:var(--accent-red); color:#fff; }
  .sb-right { display:flex; align-items:center; gap:2px; }
  .sb-btn { display:flex; align-items:center; justify-content:center; background:none; border:none; color:var(--text-muted); padding:3px 6px; cursor:pointer; border-radius:4px; transition:all 0.12s ease; }
  .sb-btn:hover { color:var(--text-primary); background:var(--bg-hover); }
  .sb-btn.active { color:var(--accent-blue); }

  /* Floating panels */
  .float-panel { position:fixed; bottom:calc(var(--status-bar-height) + 8px); width:360px; height:440px; background:var(--bg-secondary); border:1px solid var(--border-primary); border-radius:12px; box-shadow:0 8px 32px rgba(0,0,0,0.5); display:flex; flex-direction:column; overflow:hidden; z-index:100; animation:floatUp 0.2s ease; }
  .float-ai { right:12px; }
  .ai-resize-handle {
    position: absolute;
    background: transparent;
    z-index: 1000;
  }
  .ai-resize-w {
    left: 0;
    top: 0;
    bottom: 0;
    width: 6px;
    cursor: ew-resize;
  }
  .ai-resize-n {
    left: 0;
    right: 0;
    top: 0;
    height: 6px;
    cursor: ns-resize;
  }
  .ai-resize-nw {
    left: 0;
    top: 0;
    width: 12px;
    height: 12px;
    cursor: nwse-resize;
    z-index: 1001;
  }
  .float-runner { left:12px; }
  .float-logs { position:fixed; right:0; top:34px; bottom:var(--status-bar-height); width:350px; height:calc(100vh - 34px - var(--status-bar-height)); background:var(--bg-secondary); border:none; border-left:1px solid var(--border-primary); border-radius:0; box-shadow:-4px 0 24px rgba(0,0,0,0.35); z-index:1005; animation:slideInRight 0.2s cubic-bezier(0.16, 1, 0.3, 1); }
  @keyframes slideInRight { from { transform: translateX(100%); } to { transform: translateX(0); } }
  .float-header { display:flex; align-items:center; justify-content:space-between; padding:8px 12px; background:var(--bg-surface); border-bottom:1px solid var(--border-subtle); font-size:var(--font-size); font-weight:600; color:var(--text-primary); }
  .float-close { background:none; border:none; color:var(--text-muted); padding:2px 5px; cursor:pointer; border-radius:3px; font-size:var(--fs-16); line-height:1; }
  .float-close:hover { color:var(--accent-red); background:var(--bg-hover); }
  .float-btn { background:none; border:1px solid var(--border-subtle); color:var(--text-muted); padding:1px 8px; cursor:pointer; border-radius:4px; font-size:var(--fs-10); transition:all 0.12s ease; }
  .float-btn:hover { color:var(--text-primary); border-color:var(--text-muted); }
  .float-header-actions { display:flex; align-items:center; gap:4px; }
  .float-body { flex:1; overflow:hidden; }

  /* Logs */
  .logs-list { height:100%; overflow-y:auto; padding:4px 0; font-size:var(--fs-11); font-family:monospace; user-select: text; -webkit-user-select: text; }
  .log-entry { display:flex; gap:8px; padding:3px 12px; border-bottom:1px solid var(--border-subtle); transition:background 0.1s ease; user-select: text; -webkit-user-select: text; }
  .log-entry:hover { background:var(--bg-hover); }
  .log-entry.log-error { color:var(--accent-red); }
  .log-time { color:var(--text-muted); flex-shrink:0; }
  .log-msg { color:var(--text-secondary); word-break:break-word; }
  .log-empty { display:flex; align-items:center; justify-content:center; height:100%; color:var(--text-muted); font-size:var(--font-size); font-family:initial; }

  /* Private Terminal */
  .private-terminal-wrap { display:flex; flex-direction:column; height:100%; }
  .private-badge { display:flex; align-items:center; gap:6px; padding:4px 10px; background:color-mix(in srgb, var(--accent-red) 10%, transparent); border-bottom:1px solid color-mix(in srgb, var(--accent-red) 30%, transparent); color:var(--accent-red); font-size:var(--fs-10); font-weight:600; flex-shrink:0; }
  .private-badge svg { flex-shrink:0; }
  .private-item { color:var(--accent-red) !important; }
  .private-item:hover { background:color-mix(in srgb, var(--accent-red) 10%, transparent) !important; }
  .private-tag { margin-left:auto; font-size:var(--fs-9); background:color-mix(in srgb, var(--accent-red) 15%, transparent); color:var(--accent-red); padding:1px 5px; border-radius:3px; font-weight:700; }

  /* Glassmorphic Backdrop and Panel styles */
  .workspace-backdrop {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: 0;
    background: var(--app-bg-gradient, none);
    background-image: var(--app-bg-image, none);
    background-size: cover;
    background-position: center;
    pointer-events: none;
  }
  .workspace {
    position: relative;
    background: transparent !important;
  }
  .sidebar, .tab-bar, .status-bar, .activity-bar, .pm-sidebar, .ext-sidebar, .pio-sidebar, .float-panel, .settings-tabs,
  .workspace-area, .sidebar-body, .sidebar-workspace-content {
    background: var(--glass-bg, var(--bg-secondary)) !important;
    backdrop-filter: blur(var(--glass-blur, 12px));
    -webkit-backdrop-filter: blur(var(--glass-blur, 12px));
    border-color: var(--glass-border, var(--border-primary)) !important;
    z-index: 1;
  }
  .workspace-area {
    z-index: 1;
  }
  .tab-panel {
    background: transparent !important;
  }
  .hidden {
    display: none !important;
  }
  @keyframes pulse-red {
    0% { transform: scale(1); box-shadow: 0 0 0 0 rgba(239, 68, 68, 0.7); }
    70% { transform: scale(1); box-shadow: 0 0 0 4px rgba(239, 68, 68, 0); }
    100% { transform: scale(1); box-shadow: 0 0 0 0 rgba(239, 68, 68, 0); }
  }
  .sb-degraded-badge {
    animation: pulse-red 2s infinite;
  }

  /* CMMO Status Bar Integrations */
  .sb-framework-tag {
    display: inline-flex;
    align-items: center;
    font-size: var(--fs-9);
    font-weight: 700;
    color: var(--accent-blue);
    background: color-mix(in srgb, var(--accent-blue) 8%, transparent);
    border: 1px solid color-mix(in srgb, var(--accent-blue) 20%, transparent);
    padding: 1.5px 6px;
    border-radius: 4px;
    letter-spacing: 0.3px;
    text-transform: uppercase;
  }
  .sb-graph-status {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: var(--fs-9);
    font-weight: 600;
    padding: 1.5px 6px;
    border-radius: 4px;
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
  }
  .status-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    display: inline-block;
  }
  .sb-graph-synced {
    color: var(--accent-green);
    border-color: color-mix(in srgb, var(--accent-green) 20%, transparent);
    background: color-mix(in srgb, var(--accent-green) 5%, transparent);
  }
  .sb-graph-synced .status-dot {
    background: var(--accent-green);
    box-shadow: 0 0 6px var(--accent-green);
  }
  .sb-graph-syncing {
    color: var(--accent-yellow);
    border-color: color-mix(in srgb, var(--accent-yellow) 20%, transparent);
    background: color-mix(in srgb, var(--accent-yellow) 5%, transparent);
  }
  .sb-graph-syncing .status-dot {
    background: var(--accent-yellow);
    animation: statusPulse 1.2s infinite ease-in-out;
  }
  .sb-graph-error {
    color: var(--accent-red);
    border-color: color-mix(in srgb, var(--accent-red) 20%, transparent);
    background: color-mix(in srgb, var(--accent-red) 5%, transparent);
  }
  .sb-graph-error .status-dot {
    background: var(--accent-red);
  }
  @keyframes statusPulse {
    0%, 100% { opacity: 0.3; transform: scale(0.8); }
    50% { opacity: 1; transform: scale(1.2); }
  }
  .sb-indexing-bar {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-9);
    font-weight: 600;
    color: var(--accent-blue);
    background: color-mix(in srgb, var(--accent-blue) 10%, transparent);
    border: 1px solid color-mix(in srgb, var(--accent-blue) 25%, transparent);
    padding: 1.5px 7px;
    border-radius: 4px;
    margin-left: 8px;
    animation: indexingPulse 2.5s infinite ease-in-out;
  }
  @keyframes indexingPulse {
    0%, 100% { border-color: color-mix(in srgb, var(--accent-blue) 20%, transparent); }
    50% { border-color: color-mix(in srgb, var(--accent-blue) 50%, transparent); box-shadow: 0 0 8px color-mix(in srgb, var(--accent-blue) 15%, transparent); }
  }
  .sb-spinner {
    width: 10px;
    height: 10px;
    border: 1.5px solid var(--border-subtle);
    border-top-color: var(--accent-blue);
    border-radius: 50%;
    animation: pioSpin 0.8s linear infinite;
  }
  .sb-swarm-badge {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: var(--fs-9);
    font-weight: 600;
    color: var(--accent-purple, #c084fc);
    background: color-mix(in srgb, var(--accent-purple, #c084fc) 8%, transparent);
    border: 1px solid color-mix(in srgb, var(--accent-purple, #c084fc) 20%, transparent);
    padding: 1.5px 6px;
    border-radius: 4px;
    margin-right: 4px;
  }
  .swarm-pulse {
    width: 5px;
    height: 5px;
    background: var(--accent-purple, #c084fc);
    border-radius: 50%;
    animation: statusPulse 1s infinite ease-in-out;
  }
  .sb-budget-badge {
    display: inline-flex;
    align-items: center;
    font-size: var(--fs-9);
    font-weight: 700;
    color: var(--accent-green);
    background: color-mix(in srgb, var(--accent-green) 8%, transparent);
    border: 1px solid color-mix(in srgb, var(--accent-green) 20%, transparent);
    padding: 1.5px 6px;
    border-radius: 4px;
    margin-right: 4px;
    font-family: monospace;
  }
  .sb-budget-badge.sb-budget-high {
    color: var(--accent-red);
    background: color-mix(in srgb, var(--accent-red) 8%, transparent);
    border-color: color-mix(in srgb, var(--accent-red) 25%, transparent);
    animation: pulse-red 1.5s infinite;
  }
  .sb-health-badge {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    font-size: var(--fs-9);
    font-weight: 600;
    color: var(--text-secondary);
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    padding: 1.5px 6px;
    border-radius: 4px;
    cursor: pointer;
    transition: all 0.15s ease;
    margin-right: 4px;
  }
  .sb-health-badge:hover {
    border-color: var(--accent-red);
    color: var(--text-primary);
    background: color-mix(in srgb, var(--accent-red) 4%, transparent);
  }
  .sb-health-badge.sb-health-degraded {
    color: var(--accent-red);
    border-color: color-mix(in srgb, var(--accent-red) 30%, transparent);
    background: color-mix(in srgb, var(--accent-red) 8%, transparent);
    animation: pulse-red 2s infinite;
  }

  .tab-bar {
    z-index: 2000 !important;
  }
  /* consolidated dropdown menu styling */
  .nyx-dropdown-menu {
    position: absolute;
    top: 30px;
    left: 0;
    z-index: 9999 !important;
    min-width: 180px;
    background: var(--glass-bg, rgba(23, 23, 23, 0.85));
    border: 1px solid var(--glass-border, rgba(255, 255, 255, 0.08));
    border-radius: 8px;
    padding: 4px;
    backdrop-filter: blur(var(--glass-blur, 12px));
    -webkit-backdrop-filter: blur(var(--glass-blur, 12px));
    box-shadow: 0 10px 30px -5px rgba(0, 0, 0, 0.6), 0 0 1px 0 rgba(255, 255, 255, 0.15) inset;
    animation: cmIn 0.1s ease;
  }
  .nyx-dropdown-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 10px;
    border: none;
    background: transparent;
    color: var(--text-secondary);
    font-size: var(--fs-11);
    cursor: pointer;
    border-radius: 5px;
    transition: all 0.12s ease;
    white-space: nowrap;
    text-align: left;
    box-sizing: border-box;
  }
  .nyx-dropdown-item:hover {
    color: var(--text-primary);
    background: rgba(255, 255, 255, 0.06);
  }
  .nyx-dropdown-item svg {
    flex-shrink: 0;
  }
  .nyx-dropdown-divider {
    border: none;
    border-top: 1px solid var(--border-subtle);
    margin: 4px 0;
  }
</style>
