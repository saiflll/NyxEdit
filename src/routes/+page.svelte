<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
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
  import { currentDir, addToast, aiSendRequest, activeSshProfile, activeFile, fileContent, type Agent } from "$lib/stores.svelte";
  import { onMount } from "svelte";
  import { getStoredTheme, getStoredFont, applyTheme, applyFont } from "$lib/themes";
  import { setExtensionIcons, getExtensionIcons } from "$lib/icon-overrides";
  import { initIdle } from "$lib/idle.svelte";

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

  // ─── Primary CWD (from terminal) ──────────────
  let primaryCwd = $state("");
  let activeFilePath = $state("");

  // ─── App Logs ─────────────────────────────────
  let logs = $state<{ time: string; msg: string; type: string }[]>([]);
  function addLog(msg: string, type = "info") {
    const t = new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
    logs = [...logs, { time: t, msg, type }];
  }
  // Capture console.log/error
  const origLog = console.log;
  const origError = console.error;
  console.log = (...args: any[]) => { origLog(...args); addLog(args.map(a => typeof a === "string" ? a : JSON.stringify(a)).join(" "), "info"); };
  console.error = (...args: any[]) => { origError(...args); addLog(args.map(a => typeof a === "string" ? a : JSON.stringify(a)).join(" "), "error"); };
  addLog("App started");

  // ─── Clock ────────────────────────────────────
  let now = $state(new Date());
  $effect(() => {
    const id = setInterval(() => { now = new Date(); }, 1000);
    return () => clearInterval(id);
  });
  function fmtTime(d: Date) { return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }); }

  // ─── Platform IO ──────────────────────────────
  type PioResult = { success: boolean; output: string; error: string | null };
  type PioStatus = { installed: boolean; version: string | null; python: string | null; error?: string };
  let pioStatus = $state<PioStatus>({ installed: false, version: null, python: null });
  let pioBoards = $state<string[]>([]);
  let pioBusy = $state(false);
  let pioInstalling = $state(false);
  let pioInitPath = $state("");
  let pioStatusMsg = $state("");
  let pioBoardSearch = $state("");
  let pioBoardLimit = $state(50);
  const MAX_PIO_BOARDS = 50;
  let pioFilteredBoards = $derived(
    pioBoardSearch
      ? pioBoards.filter(b => b.toLowerCase().includes(pioBoardSearch.toLowerCase())).slice(0, MAX_PIO_BOARDS)
      : pioBoards.slice(0, MAX_PIO_BOARDS)
  );

  let hasCheckedPio = false;
  async function checkPio(force = false) {
    if (hasCheckedPio && !force) return;
    try {
      const s = await invoke<PioStatus>("pio_detect");
      pioStatus = s;
      hasCheckedPio = true;
      if (s.installed) {
        addLog(`PlatformIO detected: ${s.version}`);
        if (pioBoards.length === 0) {
          const boards = await invoke<string[]>("pio_list_boards", { search: null });
          pioBoards = boards.slice(0, 20);
        }
      }
    } catch (e: any) {
      pioStatus = { installed: false, version: null, python: null, error: String(e) };
    }
  }

  async function installPio() {
    pioInstalling = true;
    pioStatusMsg = "Installing PlatformIO...";
    try {
      const res = await invoke<PioResult>("pio_install");
      if (res.success) {
        pioStatusMsg = "PlatformIO installed successfully!";
        addLog("PlatformIO installed");
        await checkPio();
      } else {
        pioStatusMsg = "Installation failed";
        console.error(res.error);
      }
    } catch (e: any) {
      pioStatusMsg = "Installation error";
      console.error(e);
    }
    pioInstalling = false;
  }

  async function initPioProject(board?: string) {
    const path = pioInitPath || primaryCwd;
    if (!path) return;
    pioBusy = true;
    pioStatusMsg = "Initializing project...";
    try {
      const res = await invoke<PioResult>("pio_init", { path, board: board || null });
      if (res.success) {
        pioStatusMsg = board ? `Project initialized with ${board}!` : "Project initialized!";
        addLog(`PIO project initialized at ${path}${board ? ` (${board})` : ""}`);
      } else {
        pioStatusMsg = "Init failed";
        console.error(res.error);
      }
    } catch (e: any) {
      pioStatusMsg = "Init error";
      console.error(e);
    }
    pioBusy = false;
  }

  function initPioProjectWithBoard(board: string) {
    pioInitPath = primaryCwd;
    initPioProject(board);
  }

  async function runPioTarget(target: string) {
    try {
      const res = await invoke<PioResult>("pio_run", { target, directory: primaryCwd });
      pioStatusMsg = `pio ${target}: ${res.success ? "done" : "failed"}`;
      addLog(`PIO ${target}: ${res.success ? "OK" : "FAIL"}`);
      if (!res.success) console.error(res.error);
    } catch (e: any) {
      pioStatusMsg = `pio ${target} error`;
      console.error(e);
    }
  }

  $effect(() => {
    if (sidebarView === "platformio") checkPio();
  });

  function runScriptSetup(type: string) {
    const scripts: Record<string, { cmd: string; args: string[] }> = {
      antigravity: { cmd: "powershell", args: ["-c", "Invoke-Expression (Invoke-RestMethod https://raw.githubusercontent.com/antigravity/cli/main/install.ps1)"] },
      opencode: { cmd: "powershell", args: ["-c", "Invoke-Expression (Invoke-RestMethod https://raw.githubusercontent.com/opencode-ai/cli/main/install.ps1)"] },
    };
    const s = scripts[type];
    if (!s) return;
    // Try to find an existing terminal tab, or create one
    let termTab = tabs.find(t => t.type === "terminal");
    if (!termTab) {
      const id = `tab-term-${Date.now()}`;
      tabs = [...tabs, { id, type: "terminal" as TabType, label: "Terminal" }];
      termTab = tabs[tabs.length - 1];
    }
    addLog(`Running ${type} setup script...`);
    const isWin = navigator.userAgent.toLowerCase().includes("win");
    // Write the script command into the terminal via invoke
    invoke("pty_write", { sessionId: "term-1", data: `${s.cmd} ${s.args.join(" ")}${isWin ? "\r" : "\n"}` }).catch(e => console.error(e));
  }

  // ─── Extensions ──────────────────────────────
  type Extension = {
    id: string; name: string; version: string; description: string; type: string;
    url?: string; installed: boolean;
    theme?: Record<string, string>;
    icons?: Record<string, string>;
    scripts?: { install?: string; uninstall?: string };
  };

  let extensions = $state<Extension[]>([]);
  let extUrl = $state("");
  let extBusy = $state(false);
  let extMsg = $state("");

  function loadExtensions() {
    try {
      const raw = localStorage.getItem("nyxedit-extensions");
      if (raw) extensions = JSON.parse(raw);
    } catch {}
  }

  function saveExtensions() {
    try { localStorage.setItem("nyxedit-extensions", JSON.stringify(extensions)); } catch {}
  }

  function applyExtensionTheme(ext: Extension) {
    if (!ext.theme || !ext.installed) return;
    const root = document.documentElement;
    for (const [key, val] of Object.entries(ext.theme)) {
      root.style.setProperty(key, val);
    }
  }

  function removeExtensionTheme(ext: Extension) {
    if (!ext.theme) return;
    const root = document.documentElement;
    for (const key of Object.keys(ext.theme)) {
      root.style.removeProperty(key);
    }
    applyTheme(getStoredTheme());
  }

  async function addExtensionFromUrl(url: string) {
    if (!url.trim()) return;
    extBusy = true; extMsg = "Fetching...";
    try {
      const res = await tauriFetch(url.trim());
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = await res.json();
      if (!data.name) throw new Error("Invalid extension format: missing 'name'");
      const id = "ext-" + Date.now().toString(36);
      const ext: Extension = {
        id, name: data.name, version: data.version || "1.0",
        description: data.description || "", type: data.type || "misc",
        url: url.trim(), installed: true,
        theme: data.theme, icons: data.icons, scripts: data.scripts,
      };
      extensions = [...extensions, ext];
      saveExtensions();
      if (ext.theme) applyExtensionTheme(ext);
      if (ext.icons) {
        const all = { ...getExtensionIcons(), ...ext.icons };
        setExtensionIcons(all);
      }
      if (ext.scripts?.install) {
        const termTab = tabs.find(t => t.type === "terminal");
        if (termTab) activeTabId = termTab.id;
        const isWin = navigator.userAgent.toLowerCase().includes("win");
        invoke("pty_write", { sessionId: "term-1", data: ext.scripts.install + (isWin ? "\r" : "\n") }).catch(() => {});
      }
      extMsg = `Installed: ${ext.name}`;
      extUrl = "";
    } catch (e: any) {
      extMsg = `Error: ${e.message}`;
    }
    extBusy = false;
  }

  function removeExtension(id: string) {
    const ext = extensions.find(e => e.id === id);
    if (!ext) return;
    if (ext.scripts?.uninstall) {
      const isWin = navigator.userAgent.toLowerCase().includes("win");
      invoke("pty_write", { sessionId: "term-1", data: ext.scripts.uninstall + (isWin ? "\r" : "\n") }).catch(() => {});
    }
    removeExtensionTheme(ext);
    if (ext.icons) {
      const all = getExtensionIcons();
      for (const k of Object.keys(ext.icons)) delete all[k];
      setExtensionIcons(all);
    }
    extensions = extensions.filter(e => e.id !== id);
    saveExtensions();
    extMsg = `Removed: ${ext.name}`;
  }

  function toggleExtension(id: string) {
    const ext = extensions.find(e => e.id === id);
    if (!ext) return;
    if (ext.installed) {
      removeExtension(id);
    } else {
      if (ext.url) addExtensionFromUrl(ext.url);
    }
  }

  loadExtensions();

  // ─── Tab labels ───────────────────────────────
  const TAB_LABELS: Record<TabType, string> = {
    file: "Untitled", settings: "Settings", setup: "Setup",
    terminal: "Terminal", preview: "Preview",
    ssh_session: "SSH", private: "Private",
    api_client: "API Client",
    db_query: "DB Query",
    diff: "AI Diff",
    ai_chat: "AI Chat",
  };
  const TAB_ICONS: Record<TabType, string> = {
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
  };

  function normalizeUrl(url: string): string {
    if (!url || url === "undefined") return url;
    if (url.startsWith("http://") || url.startsWith("https://") || url.startsWith("file://")) return url;
    return "https://" + url;
  }

  // ─── Tab management ───────────────────────────
  let addMenuOpen = $state(false);

  function addTab(type: TabType, extra?: Partial<Tab>) {
    const base = TAB_LABELS[type];
    const count = tabs.filter((t) => t.type === type).length;
    const label = type === "terminal" || type === "file" ? `${base} ${count + 1}` : base;
    const id = "tab-" + Date.now().toString(36) + Math.random().toString(36).slice(2, 4);
    tabs = [...tabs, { id, type, label, ...extra }];
    activeTabId = id;
    addMenuOpen = false;
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

  import { ensureNyxDir } from "$lib/nyxConfig";
  $effect(() => {
    const unsub = currentDir.subscribe(async (val) => {
      if (val) {
        await ensureNyxDir();
        // Notify AI backend of workspace root for agent logs
        invoke("ai_set_workspace", { root: val }).catch(() => {});
        invoke("project_detect", { root: val }).catch(() => {});
      }
    });
    return unsub;
  });

  // Open SSH session as a dedicated tab
  function openSshSessionTab(profile: any) {
    const existing = tabs.find(t => t.type === "ssh_session" && t.sshProfile?.id === profile.id);
    if (existing) { activeTabId = existing.id; return; }
    const id = "tab-ssh-" + Date.now().toString(36);
    tabs = [...tabs, { id, type: "ssh_session", label: `SSH: ${profile.name}`, sshProfile: profile }];
    activeTabId = id;
  }

  function openDbQueryTab(connId: string, label: string) {
    const existing = tabs.find(t => t.type === "db_query" && t.connectionId === connId);
    if (existing) { activeTabId = existing.id; return; }
    const id = "tab-db-" + Date.now().toString(36);
    tabs = [...tabs, { id, type: "db_query", label: `DB: ${label}`, connectionId: connId }];
    activeTabId = id;
  }

  async function onRemoteFileOpen(sessionId: string, remotePath: string, name: string) {
    const path = `sftp://${sessionId}${remotePath}`;
    const existing = tabs.find((t) => t.filePath === path);
    if (existing) { activeTabId = existing.id; return; }
    
    try {
      addLog(`Opening remote file ${remotePath} via SFTP...`);
      const content = await invoke<string>("sftp_read_file", {
        sessionId, remotePath
      });
      addTab("file", { label: `[Remote] ${name}`, filePath: path, fileContent: content });
    } catch (e) {
      addToast(`Failed to open remote file: ${e}`, "error");
    }
  }

  // Register a PTY session as private (AI-restricted)
  async function markSessionPrivate(sessionId: string) {
    try {
      await invoke("pty_mark_private", { sessionId });
      privateSessionIds = new Set([...privateSessionIds, sessionId]);
    } catch (e) {
      console.error("Failed to mark private:", e);
    }
  }

  // ─── Custom Window Controls ───────────────────
  const appWindow = getCurrentWindow();
  function minimizeWindow() {
    console.log("Minimizing window...");
    appWindow.minimize().catch(err => {
      console.error("Minimize error:", err);
      alert("Minimize error: " + err);
    });
  }
  async function toggleMaximizeWindow() {
    console.log("Toggling maximize window...");
    try {
      if (await appWindow.isMaximized()) {
        await appWindow.unmaximize();
      } else {
        await appWindow.maximize();
      }
    } catch (err) {
      console.error("Maximize error:", err);
      alert("Maximize error: " + err);
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
    } catch (err) {
      console.error("Failed to open new window:", err);
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
    } catch (err) {
      console.error("Open file error:", err);
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
    } catch (err) {
      console.error("Open folder error:", err);
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
    console.log("Closing window...");
    appWindow.close().catch(err => {
      console.error("Close error:", err);
      alert("Close error: " + err);
    });
  }

  function handleHeaderMousedown(e: MouseEvent) {
    if (e.button !== 0) return; // Only left-click
    const target = e.target as HTMLElement;
    if (target.closest(".mac-controls, .tab-add-wrap, button, input, select, textarea, .add-menu")) {
      return; // Skip drag when clicking active controls
    }
    appWindow.startDragging().catch(err => {
      console.log("Manual drag failed:", err);
    });
  }

  // ─── CWD from terminal ────────────────────────
  function onTerminalCwdChange(cwd: string) {
    primaryCwd = cwd;
    currentDir.set(cwd);
  }

  // ─── Sidebar ──────────────────────────────────
  const SIDEBAR_LABELS: Record<string, string> = {
    files: "Workspace", search: "Search", ssh: "SSH Tree", postman: "API Client", mqtt: "MQTT Client", platformio: "Platform IO", extensions: "Extensions", database: "Database Client", intel: "Intel",
  };

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
        invoke("pty_write", { sessionId: "term-1", data: `cd "${dir}"${isWin ? "\r" : "\n"}` }).catch(() => {});
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

  function onResizeStart(e: MouseEvent) {
    isResizing = true;
    resizeStartX = e.clientX;
    resizeStartWidth = sidebarWidth;
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
  }
  function onResizeMove(e: MouseEvent) {
    if (!isResizing) return;
    sidebarWidth = Math.max(180, Math.min(480, resizeStartWidth + (e.clientX - resizeStartX)));
  }
  function onResizeEnd() {
    isResizing = false;
    document.body.style.cursor = "";
    document.body.style.userSelect = "none";
  }
</script>

<svelte:window onkeydown={handleGlobalKeydown} onmousemove={onResizeMove} onmouseup={onResizeEnd} onclick={() => (addMenuOpen = false)} />

<div class="workspace">
  <!-- Custom Background Backdrop -->
  <div class="workspace-backdrop"></div>
  <!-- ═══ TAB BAR ═══ -->
  <header class="tab-bar" onmousedown={handleHeaderMousedown}>
    <!-- Left tools -->
    <div class="tab-bar-left">
      <button class="tool-btn" onclick={triggerOpenFile} title="Open File">
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/></svg>
      </button>
      <button class="tool-btn" onclick={triggerOpenFolder} title="Open Folder">
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>
      </button>
      <button class="tool-btn" onclick={openSettingsTab} title="Settings">
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>
      </button>
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
                  {#if pioBusy || pioInstalling}
                    <span class="pio-spinner"></span>
                  {/if}
                  <span class="pio-status-text" class:pio-installed={pioStatus.installed} class:pio-not-installed={!pioStatus.installed}>
                    {#if pioStatusMsg}
                      {pioStatusMsg}
                    {:else if pioStatus.installed}
                      {pioStatus.version || "PlatformIO ready"}
                    {:else if pioStatus.error}
                      Error checking PlatformIO
                    {:else}
                      Checking...
                    {/if}
                  </span>
                </div>
                <button class="tool-btn" onclick={() => checkPio(true)} title="Recheck PlatformIO" style="padding: 2px;">
                  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21.5 2v6h-6M21.34 15.57a10 10 0 1 1-.57-8.38l5.67-5.67"/></svg>
                </button>
              </div>

              {#if !pioStatus.installed && !pioBusy}
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
                    {#if pioStatus.python}
                      <span class="pio-detail pio-py-ok">&#10003; {pioStatus.python}</span>
                    {:else}
                      <span class="pio-detail pio-py-missing">&#10007; Python not found</span>
                    {/if}
                  </div>
                  <button class="pio-install-btn" onclick={installPio} disabled={pioInstalling || !pioStatus.python}>
                    {pioInstalling ? "Installing..." : "Install PlatformIO"}
                  </button>
                </div>
              {/if}

              {#if pioStatus.installed}
                <div class="pio-section">
                  <div class="pio-section-title">QUICK ACCESS</div>
                  <button class="pio-item" onclick={() => runPioTarget("build")}>
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polygon points="5 3 19 12 5 21 5 3"/></svg>
                    <span>Build</span>
                  </button>
                  <button class="pio-item" onclick={() => runPioTarget("upload")}>
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="17 8 12 3 7 8"/><line x1="12" y1="3" x2="12" y2="15"/></svg>
                    <span>Upload</span>
                  </button>
                  <button class="pio-item" onclick={() => runPioTarget("clean")}>
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
                    <input class="pio-init-input" bind:value={pioInitPath} placeholder={primaryCwd || "Project path..."} />
                    <button class="pio-init-btn" onclick={() => initPioProject()} disabled={pioBusy}>Init</button>
                  </div>
                </div>

                <div class="pio-section">
                  <div class="pio-section-title">BOARDS ({pioBoards.length})</div>
                  <div class="pio-board-search">
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
                    <input type="text" bind:value={pioBoardSearch} placeholder="Search boards..." class="pio-search-input" />
                  </div>
                  <div class="pio-boards-list">
                    {#each pioFilteredBoards as board}
                      <button class="pio-item pio-board" onclick={() => initPioProjectWithBoard(board)}>
                        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="4" y="2" width="16" height="20" rx="2"/><line x1="9" y1="6" x2="15" y2="6"/><line x1="12" y1="2" x2="12" y2="6"/><circle cx="9" cy="12" r="1"/><circle cx="15" cy="12" r="1"/><circle cx="9" cy="17" r="1"/><circle cx="15" cy="17" r="1"/></svg>
                        <span>{board}</span>
                      </button>
                    {:else}
                      {#if pioBoardSearch}
                        <div class="pio-no-results">No boards match "{pioBoardSearch}"</div>
                      {/if}
                    {/each}
                    {#if !pioBoardSearch && pioBoards.length > MAX_PIO_BOARDS}
                      <div class="pio-no-results">Showing {MAX_PIO_BOARDS} of {pioBoards.length} boards</div>
                    {/if}
                  </div>
                </div>
              {/if}
            </div>
          {:else if sidebarView === "extensions"}
            <div class="ext-sidebar">
              <div class="ext-input-row">
                <input class="ext-input" bind:value={extUrl} placeholder="Paste URL to extension.json..." disabled={extBusy}
                  onkeydown={(e) => { if (e.key === "Enter") addExtensionFromUrl(extUrl); }} />
                <button class="ext-add-btn" onclick={() => addExtensionFromUrl(extUrl)} disabled={extBusy || !extUrl.trim()}>
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
                    <button class="ext-btn ext-installed" onclick={() => removeExtension(ext.id)}>Remove</button>
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
        <div class="tab-panel" class:hidden={activeTabId !== tab.id}>
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
    </div>

    <div class="sb-center" onclick={() => (showLogs = !showLogs)} role="button" tabindex="0" onkeydown={(e) => e.key === "Enter" && (showLogs = !showLogs)} title="Toggle logs">
      <span class="sb-clock">{fmtTime(now)}</span>
      <span class="sb-log-badge" class:sb-log-has-error={logs.some(l => l.type === "error")}>{logs.length}</span>
    </div>

    <div class="sb-right">
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
  <div class="float-panel float-ai" class:hidden={!showFloatingAi}>
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

  <!-- Photoshop-style Floating AI Input Bar -->
  <div class="photoshop-ai-bar-container">
    <AIFloatingBar />
  </div>

  <!-- ═══ GLOBAL COMPONENTS ═══ -->
  <Toast />
  <ContextMenu open={ctxOpen} x={ctxX} y={ctxY} items={contextMenuItems} onclose={() => (ctxOpen = false)} />
  <ContextMenu open={ctxFileOpen} x={ctxFileX} y={ctxFileY} items={fileContextItems} onclose={() => (ctxFileOpen = false)} />
  <CommandPalette open={showCommandPalette} commands={commandItems} onclose={() => (showCommandPalette = false)} />
</div>

<style>
  .photoshop-ai-bar-container {
    position: fixed;
    bottom: 50px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 1000;
    display: flex;
    justify-content: center;
    width: 100%;
    pointer-events: none;
  }
  .photoshop-ai-bar-container :global(> *) {
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
  .float-runner { left:12px; }
  .float-logs { right:calc(360px + 20px); width:380px; height:320px; }
  @keyframes floatUp { from { opacity:0; transform:translateY(16px) scale(0.96); } to { opacity:1; transform:translateY(0) scale(1); } }
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
</style>
