<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { open as openShell } from "@tauri-apps/plugin-shell";
  import SplitTerminal from "$lib/components/SplitTerminal.svelte";
  import AIChat from "$lib/components/AIChat.svelte";
  import GitStatus from "$lib/components/GitStatus.svelte";
  import FileManager from "$lib/components/FileManager.svelte";
  import CodeEditor from "$lib/components/CodeEditor.svelte";
  import ViewerRouter from "$lib/components/viewers/ViewerRouter.svelte";
  import Settings from "$lib/components/Settings.svelte";
  import Runner from "$lib/components/Runner.svelte";
  import Toast from "$lib/components/Toast.svelte";
  import ContextMenu from "$lib/components/ContextMenu.svelte";
  import SearchInFiles from "$lib/components/SearchInFiles.svelte";
  import CommandPalette from "$lib/components/CommandPalette.svelte";
  import { currentDir, addToast, type Agent } from "$lib/stores.svelte";
  import { onMount } from "svelte";
  import { getStoredTheme, getStoredFont, applyTheme, applyFont } from "$lib/themes";
  import { setExtensionIcons, getExtensionIcons } from "$lib/icon-overrides";
  import { initIdle } from "$lib/idle.svelte";

  type TabType = "file" | "settings" | "setup" | "terminal" | "preview";
  type SidebarView = "files" | "search" | "git" | "platformio" | "extensions" | null;

  let proxyPort = $state(0);

  let tabs = $state<Tab[]>([
    { id: "tab-term", type: "terminal", label: "Terminal" },
  ]);
  let activeTabId = $state("tab-term");

  type Tab = {
    id: string; type: TabType; label: string;
    filePath?: string; fileContent?: string; previewUrl?: string;
    isNew?: boolean; isDirty?: boolean;
  };

  // ─── Sidebar ───────────────────────────────────
  let sidebarView = $state<SidebarView>("files");
  let sidebarWidth = $state(260);

  // ─── Floating Panels ──────────────────────────
  let showFloatingAi = $state(false);
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

  async function checkPio() {
    try {
      const s = await invoke<PioStatus>("pio_detect");
      pioStatus = s;
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
    activeTabId = termTab.id;
    addLog(`Running ${type} setup script...`);
    // Write the script command into the terminal via invoke
    invoke("pty_write", { sessionId: "term-1", data: `${s.cmd} ${s.args.join(" ")}\n` }).catch(e => console.error(e));
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
      const res = await fetch(url.trim());
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
        invoke("pty_write", { sessionId: "term-1", data: ext.scripts.install + "\n" }).catch(() => {});
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
      invoke("pty_write", { sessionId: "term-1", data: ext.scripts.uninstall + "\n" }).catch(() => {});
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
  };
  const TAB_ICONS: Record<TabType, string> = {
    file: `<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5 2z"/><polyline points="14 2 14 8 20 8"/></svg>`,
    settings: `<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="3"/><path d="M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42"/></svg>`,
    setup: `<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 2L2 7l10 5 10-5-10-5z"/><path d="M2 17l10 5 10-5"/><path d="M2 12l10 5 10-5"/></svg>`,
    terminal: `<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/></svg>`,
    preview: `<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="5"/><line x1="12" y1="1" x2="12" y2="3"/><line x1="12" y1="21" x2="12" y2="23"/><line x1="4.22" y1="4.22" x2="5.64" y2="5.64"/><line x1="18.36" y1="18.36" x2="19.78" y2="19.78"/><line x1="1" y1="12" x2="3" y2="12"/><line x1="21" y1="12" x2="23" y2="12"/><line x1="4.22" y1="19.78" x2="5.64" y2="18.36"/><line x1="18.36" y1="5.64" x2="19.78" y2="4.22"/></svg>`,
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
    if (existing) { activeTabId = existing.id; return; }
    const name = path.split(/[\\/]/).pop() || "Untitled";
    const ext = name.split(".").pop()?.toLowerCase() || "";
    if (BINARY_EXTS.has(ext)) {
      // Binary/media — open immediately without reading content as text
      addTab("file", { label: name, filePath: path, fileContent: "" });
      return;
    }
    invoke<string>("fs_read_file", { path }).then((content) => {
      addTab("file", { label: name, filePath: path, fileContent: content });
    }).catch(() => {
      // If we can't read as text, still open the tab and let ViewerRouter handle it
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

  onMount(async () => {
    applyTheme(getStoredTheme());
    applyFont(getStoredFont());
    initIdle();
    try { proxyPort = await invoke<number>("get_proxy_port"); } catch {}
  });

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
    files: "Explorer", search: "Search", git: "Source Control", platformio: "Platform IO", extensions: "Extensions",
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
    { id: "sidebar-git", label: "Toggle Source Control", desc: "Show/hide git panel", action: () => toggleSidebar(sidebarView === "git" ? null : "git") },
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
        invoke("pty_write", { sessionId: "term-1", data: `cd "${dir}"\n` }).catch(() => {});
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
  <!-- ═══ TAB BAR ═══ -->
  <header class="tab-bar" data-tauri-drag-region onmousedown={handleHeaderMousedown}>
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
      <div class="mac-controls">
        <button class="mac-dot mac-minimize" onclick={minimizeWindow} aria-label="Minimize"></button>
        <button class="mac-dot mac-maximize" onclick={toggleMaximizeWindow} aria-label="Maximize"></button>
        <button class="mac-dot mac-close" onclick={closeWindow} aria-label="Close"></button>
      </div>
    </div>

    <!-- Add menu -->
    {#if addMenuOpen}
      <div class="add-menu" onclick={(e) => e.stopPropagation()} role="presentation">
        {#each [{ type: "terminal", icon: `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/></svg>`, label: "Terminal" }, { type: "preview", icon: `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="5"/><line x1="12" y1="1" x2="12" y2="3"/><line x1="12" y1="21" x2="12" y2="23"/><line x1="4.22" y1="4.22" x2="5.64" y2="5.64"/><line x1="18.36" y1="18.36" x2="19.78" y2="19.78"/><line x1="1" y1="12" x2="3" y2="12"/><line x1="21" y1="12" x2="23" y2="12"/><line x1="4.22" y1="19.78" x2="5.64" y2="18.36"/><line x1="18.36" y1="5.64" x2="19.78" y2="4.22"/></svg>`, label: "Preview" }, { type: "file", icon: `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5 2z"/><polyline points="14 2 14 8 20 8"/></svg>`, label: "Editor" }] as item}
          <button class="add-menu-item" onclick={() => addTab(item.type as TabType)}>
            <span class="add-menu-icon">{@html item.icon}</span>
            <span>{item.label}</span>
          </button>
        {/each}
      </div>
    {/if}
  </header>

  <!-- ═══ BODY ═══ -->
  <div class="body">
    <!-- Activity Bar -->
    <nav class="activity-bar">
      <div class="activity-top">
        {#each ["files", "search", "git", "platformio", "extensions"] as view}
          <button class="activity-btn" class:active={sidebarView === view} onclick={() => toggleSidebar(view as SidebarView)} title={SIDEBAR_LABELS[view]}>
            {#if view === "files"}
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>
            {:else if view === "search"}
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
            {:else if view === "git"}
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><line x1="6" y1="3" x2="6" y2="15"></line><circle cx="18" cy="6" r="3"></circle><circle cx="6" cy="18" r="3"></circle><path d="M18 9a9 9 0 0 1-9 9"></path></svg>
            {:else if view === "platformio"}
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="9"/><path d="M9 12h6M12 9v6"/><path d="M7.5 7.5l9 9M7.5 16.5l9-9"/></svg>
            {:else if view === "extensions"}
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2"/><line x1="9" y1="9" x2="15" y2="15"/><line x1="15" y1="9" x2="9" y2="15"/></svg>
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
            <FileManager onFileOpen={onFileOpen} revealPath={primaryCwd} onDirChange={(dir: string) => { primaryCwd = dir; currentDir.set(dir); }} onFileContext={onFileContextMenu} />
          {:else if sidebarView === "search"}
            <SearchInFiles searchPath={primaryCwd} onFileOpen={onFileOpen} onDirChange={(dir: string) => { primaryCwd = dir; currentDir.set(dir); }} />
          {:else if sidebarView === "git"}
            <GitStatus />
          {:else if sidebarView === "platformio"}
            <div class="pio-sidebar">
              <!-- Status header -->
              <div class="pio-status-bar">
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
                  <button class="pio-item" onclick={() => addTab("terminal")}>
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
            <SplitTerminal cwd={primaryCwd} onCwdChange={onTerminalCwdChange} />
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
      <button class="sb-btn" class:active={showFloatingAi} onclick={() => { showFloatingAi = !showFloatingAi; if (showFloatingAi) showFloatingRunner = false; }} title="AI Chat">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 2a4 4 0 0 1 4 4c0 2-2 4-4 4s-4-2-4-4a4 4 0 0 1 4-4z"/><path d="M16 14h.2a4 4 0 0 1 3.8 2.8l.8 2.2H3.2l.8-2.2A4 4 0 0 1 7.8 14H8"/></svg>
      </button>
    </div>
  </footer>

  <!-- Floating AI Chat -->
  {#if showFloatingAi}
    <div class="float-panel float-ai">
      <div class="float-header">
        <span>AI Chat</span>
        <button class="float-close" onclick={() => (showFloatingAi = false)} aria-label="Close">&times;</button>
      </div>
      <div class="float-body"><AIChat /></div>
    </div>
  {/if}

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

  <!-- ═══ GLOBAL COMPONENTS ═══ -->
  <Toast />
  <ContextMenu open={ctxOpen} x={ctxX} y={ctxY} items={contextMenuItems} onclose={() => (ctxOpen = false)} />
  <ContextMenu open={ctxFileOpen} x={ctxFileX} y={ctxFileY} items={fileContextItems} onclose={() => (ctxFileOpen = false)} />
  <CommandPalette open={showCommandPalette} commands={commandItems} onclose={() => (showCommandPalette = false)} />
</div>

<style>
  .workspace { display:flex; flex-direction:column; height:100vh; width:100vw; background:var(--bg-primary); color:var(--text-primary); overflow:hidden; }

  /* ═══ TAB BAR ═══ */
  .tab-bar {
    display:flex; align-items:center; min-height:34px; background:var(--bg-secondary);
    border-bottom:1px solid var(--border-primary); flex-shrink:0; user-select:none; padding:2px 4px; gap:4px; position:relative;
    -webkit-app-region:drag;
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

  /* macOS traffic light dots */
  .mac-controls { display:flex; align-items:center; gap:7px; -webkit-app-region:no-drag; }
  .mac-dot {
    display:inline-flex; align-items:center; justify-content:center;
    width:12px; height:12px; border-radius:50%; border:none; cursor:pointer; padding:0;
    transition:filter 0.15s ease; position:relative;
    -webkit-app-region:no-drag;
  }
  .mac-dot:hover { filter:brightness(1.2); }
  .mac-close { background:#ff5f57; border:1px solid #e0453e; }
  .mac-minimize { background:#febc2e; border:1px solid #dda01d; }
  .mac-maximize { background:#28c840; border:1px solid #1faa33; }
  .mac-close::before { content:"×"; position:absolute; font-weight:700; font-size:var(--fs-9); color:#4c0002; opacity:0; transition:opacity 0.15s; pointer-events:none; line-height:1; }
  .mac-minimize::before { content:"–"; position:absolute; font-weight:700; font-size:var(--fs-9); color:#5c3e00; opacity:0; transition:opacity 0.15s; pointer-events:none; line-height:1; top:4px; }
  .mac-maximize::before { content:"+"; position:absolute; font-weight:700; font-size:var(--fs-8); color:#024c0e; opacity:0; transition:opacity 0.15s; pointer-events:none; line-height:1; }
  .mac-controls:hover .mac-dot::before { opacity:1; }

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
    position:absolute; top:100%; right:60px; margin-top:2px;
    background:var(--bg-elevated); border:1px solid var(--border-primary);
    border-radius:8px; padding:4px; z-index:200;
    box-shadow:0 8px 24px rgba(0,0,0,0.4); min-width:140px;
    animation:floatUp 0.15s ease;
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
  .activity-btn { display:flex; align-items:center; justify-content:center; width:40px; height:40px; background:none; border:none; color:var(--text-muted); cursor:pointer; border-radius:8px; transition:all 0.15s ease; position:relative; }
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
</style>
