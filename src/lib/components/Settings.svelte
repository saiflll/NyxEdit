<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import type { Agent, AgentPersona } from "../stores.svelte";
  const PROVIDERS = [
    { id: "openai", name: "OpenAI", needsApiKey: true, defaultUrl: "https://api.openai.com/v1" },
    { id: "gemini", name: "Gemini (Google)", needsApiKey: true, defaultUrl: "" },
    { id: "cerebras", name: "Cerebras", needsApiKey: true, defaultUrl: "https://api.cerebras.ai/v1" },
    { id: "mistral", name: "Mistral AI", needsApiKey: true, defaultUrl: "https://api.mistral.ai/v1" },
    { id: "alibaba", name: "Alibaba (DashScope)", needsApiKey: true, defaultUrl: "https://dashscope.aliyuncs.com/compatible-mode/v1" },
    { id: "openrouter", name: "OpenRouter", needsApiKey: true, defaultUrl: "https://openrouter.ai/api/v1" },
    { id: "xai", name: "xAI (Grok)", needsApiKey: true, defaultUrl: "https://api.x.ai/v1" },
    { id: "vercel", name: "Vercel AI Gateway", needsApiKey: true, defaultUrl: "https://ai-gateway.vercel.sh/v1" },
    { id: "ollama", name: "Ollama (Local)", needsApiKey: false, defaultUrl: "http://localhost:11434/v1" },
  ];
  // ─── Agent state ──────────────────────────────
  import { loadNyxConfig, saveNyxConfig } from "$lib/nyxConfig";
  import { currentDir, addToast } from "../stores.svelte";

  let globalInstructions = $state("");
  let skillRead = $state(true);
  let skillWrite = $state(true);
  let skillTerminal = $state(true);
  let workspaceDir = $state("");
  let savedStatus = $state("");

  $effect(() => {
    const unsub = currentDir.subscribe(val => {
      workspaceDir = val;
      loadGlobalSettings();
    });
    return unsub;
  });

  async function loadGlobalSettings() {
    try {
      if (workspaceDir) {
        const config = await loadNyxConfig("style_coding.json", {
          globalInstructions: "",
          skillRead: true,
          skillWrite: true,
          skillTerminal: true
        });
        globalInstructions = config.globalInstructions ?? "";
        skillRead = config.skillRead ?? true;
        skillWrite = config.skillWrite ?? true;
        skillTerminal = config.skillTerminal ?? true;
      } else {
        globalInstructions = localStorage.getItem("nyxedit-global-instructions") || "";
        skillRead = localStorage.getItem("nyxedit-skill-read") !== "false";
        skillWrite = localStorage.getItem("nyxedit-skill-write") !== "false";
        skillTerminal = localStorage.getItem("nyxedit-skill-terminal") !== "false";
      }
    } catch {
      globalInstructions = localStorage.getItem("nyxedit-global-instructions") || "";
      skillRead = localStorage.getItem("nyxedit-skill-read") !== "false";
      skillWrite = localStorage.getItem("nyxedit-skill-write") !== "false";
      skillTerminal = localStorage.getItem("nyxedit-skill-terminal") !== "false";
    }
  }

  async function saveGlobalSettings() {
    try {
      localStorage.setItem("nyxedit-global-instructions", globalInstructions);
      localStorage.setItem("nyxedit-skill-read", String(skillRead));
      localStorage.setItem("nyxedit-skill-write", String(skillWrite));
      localStorage.setItem("nyxedit-skill-terminal", String(skillTerminal));

      if (workspaceDir) {
        await saveNyxConfig("style_coding.json", {
          globalInstructions,
          skillRead,
          skillWrite,
          skillTerminal
        });
      }
      savedStatus = "Saved successfully!";
      addToast("Coding style settings saved", "success");
      setTimeout(() => {
        savedStatus = "";
      }, 2500);
    } catch (e) {
      console.error("Save global settings error:", e);
      addToast("Failed to save settings", "error");
    }
  }

  let agents = $state<Agent[]>([]);
  let personas = $state<AgentPersona[]>([]);
  let editingId = $state<string | null>(null);
  let showForm = $state(false);
  let formPersonaId = $state("");
  let formProvider = $state("openai");
  let formApiKey = $state("");
  let formBaseUrl = $state("");
  let formModel = $state("");
  let formTemperature = $state(0.7);
  let formSystemPrompt = $state("");
  let fetchedModels = $state<string[]>([]);
  let isFetching = $state(false);
  let fetchError = $state("");

  let activeProviderDef = $derived(PROVIDERS.find(p => p.id === formProvider) || PROVIDERS[0]);

  async function loadPersonas() {
    try {
      personas = await invoke<AgentPersona[]>("ai_list_personas");
    } catch (e) {
      console.error("Failed to load personas:", e);
    }
  }

  async function loadAgents() {
    try {
      agents = await invoke<Agent[]>("ai_list_agents");
    } catch (e) {
      console.error("Failed to load agents:", e);
    }
  }

  function startAdd() {
    editingId = null;
    showForm = true;
    formPersonaId = "";
    formProvider = "openai";
    formApiKey = "";
    formBaseUrl = "";
    formModel = "";
    formTemperature = 0.7;
    formSystemPrompt = "";
    fetchedModels = [];
    fetchError = "";
  }

  async function startEdit(a: Agent) {
    editingId = a.id;
    showForm = true;
    formPersonaId = a.persona_id || "";
    formProvider = a.provider;
    formApiKey = a.api_key || "";
    formBaseUrl = a.base_url || "";
    formModel = a.model;
    formTemperature = a.temperature ?? 0.7;
    formSystemPrompt = a.system_prompt || "";
    fetchedModels = [];
    fetchError = "";
  }

  function cancelEdit() {
    editingId = null;
    showForm = false;
  }

  function onProviderChange() {
    formBaseUrl = activeProviderDef.defaultUrl || "";
    fetchedModels = [];
    formModel = "";
    fetchError = "";
  }

  function onPersonaChange() {
    const p = personas.find(x => x.id === formPersonaId);
    formSystemPrompt = p?.instructions || "";
  }

  async function fetchModels() {
    isFetching = true;
    fetchError = "";
    try {
      const provider = formProvider;
      const baseUrl = activeProviderDef.defaultUrl || formBaseUrl || null;
      const models = await invoke<{ id: string; source: string }[]>("ai_list_models", {
        apiKey: formApiKey.trim() || null,
        baseUrl,
        provider,
      });
      fetchedModels = models.map(m => m.id);
      if (fetchedModels.length > 0 && !formModel) {
        formModel = fetchedModels[0];
      }
    } catch (e: any) {
      fetchError = typeof e === "string" ? e : e.message || "Failed to fetch models";
      fetchedModels = [];
    } finally {
      isFetching = false;
    }
  }

  function safeId(s: string): string {
    return s.toLowerCase().replace(/[^a-z0-9-]/g, '-').replace(/-+/g, '-').replace(/^-|-$/g, '');
  }

  async function saveAgent() {
    if (!formModel.trim()) return;
    const baseId = safeId(formModel);
    const id = editingId || `${formProvider}-${baseId}`;
    const name = formPersonaId
      ? `${personas.find(p => p.id === formPersonaId)?.name || formPersonaId} (${formModel})`
      : `${formProvider} ${formModel}`;
    const config: Agent = {
      id,
      name,
      provider: formProvider,
      model: formModel.trim(),
      base_url: formBaseUrl.trim() || null,
      api_key: formApiKey.trim() || null,
      capabilities: [],
      temperature: formTemperature,
      system_prompt: formSystemPrompt.trim() || null,
      persona_id: formPersonaId.trim() || null,
      built_in: false,
    };
    try {
      await invoke("ai_update_agent", { config });
      await loadAgents();
      cancelEdit();
    } catch (e) {
      console.error("Failed to save agent:", e);
    }
  }

  async function removeAgent(id: string) {
    try {
      await invoke("ai_remove_agent", { agentId: id });
      await loadAgents();
      if (editingId === id) cancelEdit();
    } catch (e) {
      console.error("Failed to remove agent:", e);
    }
  }
  // ─── Shared Theme Layer ──────────────────────
  import { THEMES, FONTS, getStoredTheme, getStoredFont, getStoredFontSize, applyTheme as sharedApplyTheme, applyFont as sharedApplyFont, applyFontSize as sharedApplyFontSize } from "$lib/themes";

  let settingsTab = $state<"agent" | "appearance" | "setup" | "shortcuts" | "about">("appearance");

  let currentTheme = $state(getStoredTheme());
  let currentFont = $state(getStoredFont());
  let currentFontSize = $state(getStoredFontSize());

  function applyTheme(themeId: string) {
    currentTheme = themeId;
    sharedApplyTheme(themeId);
  }

  function applyFont(fontValue: string) {
    currentFont = fontValue;
    sharedApplyFont(fontValue);
  }

  function applyFontSize(val: number) {
    currentFontSize = val;
    sharedApplyFontSize(val);
  }

  let bgImage = $state(localStorage.getItem("nyxedit-bg-image") || "");
  let bgGradient = $state(localStorage.getItem("nyxedit-bg-gradient") || "");
  let bgBlur = $state(parseInt(localStorage.getItem("nyxedit-bg-blur") || "12", 10));
  let bgOpacity = $state(parseFloat(localStorage.getItem("nyxedit-bg-opacity") || "0.75"));

  function updateBg() {
    const root = document.documentElement;
    if (bgImage.trim()) {
      root.style.setProperty("--app-bg-image", `url('${bgImage.trim()}')`);
    } else {
      root.style.removeProperty("--app-bg-image");
    }

    if (bgGradient.trim()) {
      root.style.setProperty("--app-bg-gradient", bgGradient.trim());
    } else {
      root.style.removeProperty("--app-bg-gradient");
    }

    root.style.setProperty("--glass-blur", `${bgBlur}px`);
    root.style.setProperty("--glass-opacity", `${bgOpacity}`);
    
    // Compute transparent bg color for glass effect based on theme
    const isLightTheme = currentTheme === "light";
    const baseColor = isLightTheme ? "255, 255, 255" : "15, 15, 25";
    root.style.setProperty("--glass-bg", `rgba(${baseColor}, ${bgOpacity})`);

    localStorage.setItem("nyxedit-bg-image", bgImage);
    localStorage.setItem("nyxedit-bg-gradient", bgGradient);
    localStorage.setItem("nyxedit-bg-blur", String(bgBlur));
    localStorage.setItem("nyxedit-bg-opacity", String(bgOpacity));
  }

  function resetBg() {
    bgImage = "";
    bgGradient = "";
    bgBlur = 12;
    bgOpacity = 0.75;
    updateBg();
  }

  import { activeTerminalSessionId } from "../stores.svelte";

  let activeTermId = $state<string | null>(null);
  $effect(() => {
    const unsub = activeTerminalSessionId.subscribe(val => {
      activeTermId = val;
    });
    return unsub;
  });



  type ToolSetup = {
    id: string;
    name: string;
    description: string;
    checkCmd: string;
    installScript: string;
    icon: string;
  };

  const TOOLS: ToolSetup[] = [
    {
      id: "python",
      name: "Python 3",
      description: "Required for data science, scripting, and PlatformIO.",
      checkCmd: "python",
      installScript: "winget install Python.Python.3\r\n",
      icon: `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="color:var(--accent-blue)"><path d="M12 2a5 5 0 0 0-5 5v3H5a3 3 0 0 0-3 3v4a5 5 0 0 0 5 5h3v-3a3 3 0 0 1 3-3h4a3 3 0 0 0 3-3V8a5 5 0 0 0-5-5z"></path><path d="M12 22a5 5 0 0 0 5-5v-3h2a3 3 0 0 0 3-3V7a5 5 0 0 0-5-5h-3v3a3 3 0 0 1-3 3h-4a3 3 0 0 0-3 3v5a5 5 0 0 0 5 5z" opacity="0.4"></path></svg>`,
    },
    {
      id: "rustc",
      name: "Rust Toolchain",
      description: "Systems programming language and Cargo build system.",
      checkCmd: "rustc",
      installScript: "winget install Rustlang.Rustup\r\n",
      icon: `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="color:var(--accent-green)"><circle cx="12" cy="12" r="7"></circle><path d="M12 2v4M12 18v4M4.93 4.93l2.83 2.83M16.24 16.24l2.83 2.83M2 12h4M18 12h4M4.93 19.07l2.83-2.83M16.24 7.76l2.83-2.83"></path></svg>`,
    },
    {
      id: "go",
      name: "Go Lang",
      description: "Fast, reliable systems programming language.",
      checkCmd: "go",
      installScript: "winget install GoLang.Go\r\n",
      icon: `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="color:var(--accent-blue)"><path d="M18 11.5a6 6 0 1 1-12 0 6 6 0 0 1 12 0Z"></path><path d="M12 7.5v8M9.5 11h5"></path></svg>`,
    },
    {
      id: "pio",
      name: "PlatformIO Core",
      description: "Professional collaborative platform for embedded development.",
      checkCmd: "pio",
      installScript: "pip install platformio\r\n",
      icon: `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="color:var(--accent-green)"><rect x="3" y="3" width="18" height="18" rx="4"></rect><path d="M8 12h8M12 8v8M7 7h1M16 7h1M7 16h1M16 16h1"></path></svg>`,
    },
    {
      id: "docker",
      name: "Docker WSL",
      description: "Run containerized applications on Windows with WSL2.",
      checkCmd: "docker",
      installScript: "wsl --install; winget install Docker.DockerDesktop\r\n",
      icon: `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="color:var(--accent-blue)"><path d="M22 12h-4a2 2 0 0 0-2 2v2a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h12a2 2 0 0 1 2 2v2a2 2 0 0 0 2 2Z"></path><rect x="6" y="9" width="3" height="3" rx="0.5"></rect><rect x="10" y="9" width="3" height="3" rx="0.5"></rect><rect x="14" y="9" width="3" height="3" rx="0.5"></rect></svg>`,
    },
    {
      id: "flutter",
      name: "Flutter SDK",
      description: "Google's UI toolkit for building beautiful native apps.",
      checkCmd: "flutter",
      installScript: "winget install Flutter.Flutter\r\n",
      icon: `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="color:var(--accent-blue)"><path d="M12 2 2 12l10 10M17 7l-5 5 5 5"></path></svg>`,
    },
  ];

  let statuses = $state<Record<string, "unknown" | "checking" | "installed" | "not_detected">>({
    python: "unknown",
    rustc: "unknown",
    go: "unknown",
    pio: "unknown",
    docker: "unknown",
    flutter: "unknown",
  });

  async function checkTool(id: string, checkCmd: string) {
    statuses[id] = "checking";
    const installed = await invoke<boolean>("sys_check_installed", { cmd: checkCmd });
    statuses[id] = installed ? "installed" : "not_detected";
  }

  function checkAllTools() {
    for (const tool of TOOLS) {
      checkTool(tool.id, tool.checkCmd);
    }
  }

  async function runInstall(tool: ToolSetup) {
    let termId = activeTermId;
    if (!termId) {
      alert("No active terminal found! Please open a terminal tab first so the installation can run there.");
      return;
    }
    
    await invoke("pty_write", {
      sessionId: termId,
      data: tool.installScript,
    });
  }

  // ─── Keyboard Shortcuts State ─────────────────
  type ShortcutKey = {
    ctrl: boolean;
    alt: boolean;
    shift: boolean;
    key: string;
  };

  type ShortcutBinding = {
    id: string;
    name: string;
    description: string;
    binding: ShortcutKey;
  };

  const DEFAULT_SHORTCUTS: ShortcutBinding[] = [
    { id: "sidebar", name: "Toggle Sidebar Explorer", description: "Toggle left explorer sidebar panel visibility", binding: { ctrl: true, alt: false, shift: false, key: "b" } },
    { id: "terminal", name: "Toggle Active Terminal", description: "Activate or focus the main terminal panel", binding: { ctrl: true, alt: false, shift: false, key: "j" } },
    { id: "ai", name: "Toggle Floating AI Chat", description: "Toggle the bottom-right AI Assistant chat box", binding: { ctrl: true, alt: true, shift: false, key: "a" } },
    { id: "runner", name: "Toggle Runner Panel", description: "Toggle the bottom-right script/command runner panel", binding: { ctrl: true, alt: true, shift: false, key: "n" } },
    { id: "closeTab", name: "Close Active Tab", description: "Close the currently active editor or terminal tab", binding: { ctrl: true, alt: false, shift: false, key: "w" } },
    { id: "commandPalette", name: "Command Palette", description: "Open the command palette", binding: { ctrl: true, alt: false, shift: true, key: "p" } },
    { id: "searchFiles", name: "Search in Files", description: "Toggle the search sidebar panel", binding: { ctrl: true, alt: false, shift: true, key: "f" } },
  ];

  const aboutParticles = Array.from({ length: 16 }, (_, i) => ({
    x: ((i * 31 + 7) % 100) / 100,
    y: ((i * 47 + 11) % 100) / 100,
    s: 0.4 + ((i * 7) % 4) * 0.2,
    d: 4 + (i % 5) * 0.7,
    c: `var(--particle-${i % 5})`,
  }));

  let shortcuts = $state<ShortcutBinding[]>(loadShortcuts());
  let shortcutSearch = $state("");
  let editingShortcutId = $state<string | null>(null);
  let editCtrl = $state(false);
  let editAlt = $state(false);
  let editShift = $state(false);
  let editKey = $state("");

  function loadShortcuts(): ShortcutBinding[] {
    try {
      const stored = localStorage.getItem("nyxedit-shortcuts");
      if (stored) {
        const parsed = JSON.parse(stored);
        return DEFAULT_SHORTCUTS.map(item => {
          if (parsed[item.id]) {
            return { ...item, binding: parsed[item.id] };
          }
          return item;
        });
      }
    } catch {}
    return DEFAULT_SHORTCUTS;
  }

  function saveShortcuts() {
    const map: Record<string, ShortcutKey> = {};
    for (const s of shortcuts) {
      map[s.id] = s.binding;
    }
    try {
      localStorage.setItem("nyxedit-shortcuts", JSON.stringify(map));
    } catch {}
  }

  function startEditShortcut(item: ShortcutBinding) {
    editingShortcutId = item.id;
    editCtrl = item.binding.ctrl;
    editAlt = item.binding.alt;
    editShift = item.binding.shift;
    editKey = item.binding.key;
  }

  function cancelEditShortcut() {
    editingShortcutId = null;
  }

  function saveEditedShortcut() {
    if (!editKey.trim()) return;
    const item = shortcuts.find(s => s.id === editingShortcutId);
    if (item) {
      item.binding = {
        ctrl: editCtrl,
        alt: editAlt,
        shift: editShift,
        key: editKey.trim().toLowerCase(),
      };
      saveShortcuts();
    }
    editingShortcutId = null;
  }

  function formatBinding(binding: ShortcutKey): string {
    const parts = [];
    if (binding.ctrl) parts.push("Ctrl");
    if (binding.alt) parts.push("Alt");
    if (binding.shift) parts.push("Shift");
    parts.push(binding.key.toUpperCase());
    return parts.join(" + ");
  }

  const filteredShortcuts = $derived(
    shortcuts.filter(s =>
      s.name.toLowerCase().includes(shortcutSearch.toLowerCase()) ||
      s.description.toLowerCase().includes(shortcutSearch.toLowerCase())
    )
  );

  $effect(() => {
    loadAgents();
    loadPersonas();
    loadGlobalSettings();
    applyTheme(currentTheme);
    applyFont(currentFont);
    updateBg();
  });

  $effect(() => {
    if (settingsTab === "setup") {
      checkAllTools();
    }
  });
</script>

<div class="settings">
  <!-- ═══ Sub-tabs ═══ -->
  <div class="settings-tabs">
    <button class="settings-tab" class:active={settingsTab === "appearance"} onclick={() => (settingsTab = "appearance")}>
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="3"/><path d="M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42"/></svg>
      Appearance
    </button>
    <button class="settings-tab" class:active={settingsTab === "agent"} onclick={() => (settingsTab = "agent")}>
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="8" r="4"/><path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"/></svg>
      Agents
    </button>
    <button class="settings-tab" class:active={settingsTab === "setup"} onclick={() => (settingsTab = "setup")}>
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 2L2 7l10 5 10-5-10-5z"/><path d="M2 17l10 5 10-5"/><path d="M2 12l10 5 10-5"/></svg>
      Setup
    </button>
    <button class="settings-tab" class:active={settingsTab === "shortcuts"} onclick={() => (settingsTab = "shortcuts")}>
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="4" width="20" height="16" rx="2" ry="2"/><path d="M6 8h.01M10 8h.01M14 8h.01M18 8h.01M6 12h.01M10 12h.01M14 12h.01M18 12h.01M7 16h10"/></svg>
      Shortcuts
    </button>
    <button class="settings-tab" class:active={settingsTab === "about"} onclick={() => (settingsTab = "about")}>
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/></svg>
      About
    </button>
  </div>

  <!-- ═══ Appearance Tab ═══ -->
  {#if settingsTab === "appearance"}
    <div class="appearance">
      <div class="section-title">Theme</div>
      <div class="theme-grid">
        {#each Object.entries(THEMES) as [id, theme]}
          <button class="theme-card" class:theme-active={currentTheme === id} onclick={() => applyTheme(id)}>
            <div class="theme-preview">
              <div class="tp-bg" style="background:{theme.vars['--bg-primary']}">
                <div class="tp-side" style="background:{theme.vars['--bg-secondary']};border-right-color:{theme.vars['--border-primary']}"></div>
                <div class="tp-main">
                  <div class="tp-line" style="background:{theme.vars['--text-primary']};width:55%"></div>
                  <div class="tp-line" style="background:{theme.vars['--text-secondary']};width:35%"></div>
                  <div class="tp-line" style="background:{theme.vars['--accent-blue']};width:45%"></div>
                  <div class="tp-line" style="background:{theme.vars['--text-muted']};width:25%"></div>
                </div>
              </div>
            </div>
            <span class="theme-name">{theme.name}</span>
            {#if currentTheme === id}<span class="theme-check">&#x2713;</span>{/if}
          </button>
        {/each}
      </div>

      <div class="section-title">Font</div>
      <div class="font-section">
        <select class="font-select" onchange={(e) => applyFont((e.target as HTMLSelectElement).value)}>
          {#each FONTS as f}
            <option value={f.value} selected={currentFont === f.value}>{f.label}</option>
          {/each}
        </select>
        <div class="font-preview" style="font-family:{currentFont}">
          → const hello = "nyxedit"; ←
        </div>
      </div>

      <div class="section-title">Font Size</div>
      <div class="fontsize-section">
        <input type="range" min="9" max="24" value={currentFontSize}
          oninput={(e) => applyFontSize(parseInt((e.target as HTMLInputElement).value))}
          class="fontsize-slider"
        />
        <span class="fontsize-label">{currentFontSize}px</span>
      </div>

      <div class="section-title">Custom Backdrop & Glassmorphism</div>
      <div class="bg-section">
        <label class="form-field" style="display: flex; flex-direction: column; gap: 4px; margin-bottom: 8px;">
          <span style="font-size: var(--fs-10); font-weight: 600; color: var(--text-secondary);">Background Image URL</span>
          <input type="text" bind:value={bgImage} oninput={updateBg} placeholder="https://example.com/image.jpg" class="form-input" style="background: var(--bg-surface); color: var(--text-primary); border: 1px solid var(--border-subtle); border-radius: 4px; padding: 4px 6px; font-size: var(--fs-11); width: 100%; outline: none;" />
        </label>
        <label class="form-field" style="display: flex; flex-direction: column; gap: 4px; margin-bottom: 8px;">
          <span style="font-size: var(--fs-10); font-weight: 600; color: var(--text-secondary);">Gradient Background CSS</span>
          <input type="text" bind:value={bgGradient} oninput={updateBg} placeholder="linear-gradient(135deg, #0d0d2b 0%, #1a1a3a 100%)" class="form-input" style="background: var(--bg-surface); color: var(--text-primary); border: 1px solid var(--border-subtle); border-radius: 4px; padding: 4px 6px; font-size: var(--fs-11); width: 100%; outline: none;" />
        </label>
        <div class="form-row-grid" style="display: grid; grid-template-columns: repeat(2, 1fr); gap: 12px; margin-top: 8px;">
          <label class="form-field" style="display: flex; flex-direction: column; gap: 4px;">
            <span style="font-size: var(--fs-10); font-weight: 600; color: var(--text-muted);">Backdrop Blur ({bgBlur}px)</span>
            <input type="range" min="0" max="40" step="1" bind:value={bgBlur} oninput={updateBg} class="fontsize-slider" style="width: 100%;" />
          </label>
          <label class="form-field" style="display: flex; flex-direction: column; gap: 4px;">
            <span style="font-size: var(--fs-10); font-weight: 600; color: var(--text-muted);">Backdrop Opacity ({(bgOpacity * 100).toFixed(0)}%)</span>
            <input type="range" min="0.1" max="1.0" step="0.05" bind:value={bgOpacity} oninput={updateBg} class="fontsize-slider" style="width: 100%;" />
          </label>
        </div>
        <button class="settings-btn settings-btn-cancel" onclick={resetBg} style="margin-top: 14px; align-self: flex-start;">Reset Backdrop</button>
      </div>
    </div>

  <!-- ═══ Agent Tab ═══ -->
  {:else if settingsTab === "agent"}
    <div class="agent-section">
      <div class="settings-header">
        <span class="settings-title">AI Agents</span>
        <button class="settings-btn settings-btn-add" onclick={startAdd} disabled={showForm}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
          Add Agent
        </button>
      </div>

      <div class="global-agent-settings" style="margin-bottom: 20px; background: var(--bg-surface); border: 1px solid var(--border-subtle); border-radius: 8px; padding: 12px; display: flex; flex-direction: column; gap: 10px;">
        <label class="form-field form-field-full" style="display: flex; flex-direction: column; gap: 4px;">
          <span style="font-weight: 600; color: var(--text-secondary);">Global Custom Instructions</span>
          <textarea bind:value={globalInstructions} rows={3} placeholder="Add custom behavior instructions for all AI models (e.g. 'Always answer in Indonesian', 'Prefer modern ES6 syntax')..." style="background: var(--bg-primary); color: var(--text-primary); border: 1px solid var(--border-subtle); border-radius: 6px; padding: 8px; resize: none; font-size: var(--fs-11);"></textarea>
        </label>
        
        <div class="skills-section" style="display: flex; flex-direction: column; gap: 6px;">
          <span style="font-weight: 600; color: var(--text-secondary);">AI Agent Skills (Toggles)</span>
          <div class="skills-grid" style="display: flex; flex-wrap: wrap; gap: 15px;">
            <label class="checkbox-container" style="display: inline-flex; align-items: center; gap: 6px; cursor: pointer;">
              <input type="checkbox" bind:checked={skillRead} style="cursor: pointer;" />
              <span class="checkmark"></span>
              Allow Reading Files
            </label>
            <label class="checkbox-container" style="display: inline-flex; align-items: center; gap: 6px; cursor: pointer;">
              <input type="checkbox" bind:checked={skillWrite} style="cursor: pointer;" />
              <span class="checkmark"></span>
              Allow Writing/Modifying Files
            </label>
            <label class="checkbox-container" style="display: inline-flex; align-items: center; gap: 6px; cursor: pointer;">
              <input type="checkbox" bind:checked={skillTerminal} style="cursor: pointer;" />
              <span class="checkmark"></span>
              Allow Terminal Execution
            </label>
          </div>
        </div>

        <div style="display: flex; justify-content: space-between; align-items: center; margin-top: 4px; border-top: 1px solid var(--border-subtle); padding-top: 8px;">
          {#if savedStatus}
            <span style="color: var(--accent-green); font-size: var(--fs-10); font-weight: 600;">{savedStatus}</span>
          {:else}
            <span></span>
          {/if}
          <button 
            onclick={saveGlobalSettings} 
            style="padding: 6px 14px; background: var(--accent-blue); color: var(--bg-primary); border: none; border-radius: 6px; font-size: var(--fs-10); font-weight: 600; cursor: pointer; transition: all 0.12s ease;"
          >
            Save Settings
          </button>
        </div>
      </div>

      {#if showForm}
        <div class="settings-form">
          <div class="agent-form-grid">
            <label class="form-field">
              <span>Persona</span>
              <select bind:value={formPersonaId} onchange={onPersonaChange}>
                <option value="">None (manual)</option>
                {#each personas as p}
                  <option value={p.id}>{p.name} — {p.description}</option>
                {/each}
              </select>
            </label>
            <label class="form-field">
              <span>Provider</span>
              <select bind:value={formProvider} onchange={onProviderChange}>
                {#each PROVIDERS as p}
                  <option value={p.id}>{p.name}</option>
                {/each}
              </select>
            </label>
            <label class="form-field">
              <span>Model</span>
              <div class="model-input-row">
                {#if fetchedModels.length > 0}
                  <select bind:value={formModel}>
                    {#each fetchedModels as m}
                      <option value={m}>{m}</option>
                    {/each}
                  </select>
                {:else}
                  <input bind:value={formModel} placeholder="gpt-4o, claude-sonnet-4, ..." />
                {/if}
                <button class="settings-btn settings-btn-sm" onclick={fetchModels} disabled={isFetching} title="Detect models">
                  {#if isFetching}
                    <span class="spinner-tiny"></span>
                  {:else}
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M23 4v6h-6M1 20v-6h6M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/></svg>
                  {/if}
                </button>
              </div>
            </label>
            <label class="form-field">
              <span>API Key {#if !activeProviderDef.needsApiKey}<span class="form-hint-inline">(kosongi untuk Local LLM)</span>{/if}</span>
              <input bind:value={formApiKey} type="password" placeholder={activeProviderDef.needsApiKey ? `sk-...` : 'Kosongi untuk Local LLM'} />
            </label>
            <label class="form-field">
              <span>Base URL</span>
              <input bind:value={formBaseUrl} placeholder={activeProviderDef.defaultUrl || 'https://...'} />
            </label>
            <label class="form-field">
              <span>Temperature</span>
              <div class="temp-row">
                <input type="range" min="0" max="2" step="0.1" bind:value={formTemperature} class="temp-slider" />
                <span class="temp-label">{formTemperature.toFixed(1)}</span>
              </div>
            </label>
          </div>

          {#if fetchError}
            <div class="fetch-error">{fetchError}</div>
          {/if}

          {#if !formPersonaId}
            <label class="form-field form-field-full">
              <span>System Prompt</span>
              <textarea bind:value={formSystemPrompt} rows={3} placeholder="Optional system instructions for this agent..." class="form-textarea"></textarea>
            </label>
          {:else if formSystemPrompt}
            <div class="form-hint" style="padding:4px 0">System prompt diisi otomatis dari persona yang dipilih.</div>
          {/if}

          <div class="form-actions">
            <span class="form-hint">Nama & ID agent dibuat otomatis</span>
            <div class="form-btns">
              <button class="settings-btn settings-btn-cancel" onclick={cancelEdit}>Cancel</button>
              <button class="settings-btn settings-btn-save" onclick={saveAgent} disabled={!formModel.trim()}>
                {editingId ? "Update" : "Save"}
              </button>
            </div>
          </div>
        </div>
      {/if}

      <div class="settings-list">
        {#each agents as agent (agent.id)}
          <div class="agent-card">
            <div class="agent-header">
              <div class="agent-info">
                <span class="agent-name">{agent.name}</span>
                <span class="agent-meta">{agent.provider} / {agent.model}</span>
              </div>
              <div class="agent-header-badges">
                {#if agent.persona_id}<span class="agent-badge agent-badge-persona">{personas.find(p => p.id === agent.persona_id)?.name || agent.persona_id}</span>{/if}
                {#if agent.api_key}<span class="agent-badge agent-badge-key" title="API key">&#x1F512;</span>{/if}
                {#if agent.built_in}<span class="agent-badge agent-badge-builtin">built-in</span>{/if}
              </div>
            </div>
            {#if agent.base_url}<div class="agent-url">{agent.base_url}</div>{/if}
            <div class="agent-actions">
              <button class="agent-action agent-action-edit" onclick={() => startEdit(agent)} title="Edit">
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>
                Edit
              </button>
              <button class="agent-action agent-action-del" onclick={() => removeAgent(agent.id)} title="Delete" disabled={agent.built_in}>
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>
                Delete
              </button>
            </div>
          </div>
        {/each}
      </div>
    </div>
  {:else if settingsTab === "setup"}
    <div class="setup-section">
      <div class="settings-header">
        <span class="settings-title">Developer Toolchain Setup</span>
        <button class="settings-btn settings-btn-add" onclick={checkAllTools}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M23 4v6h-6M1 20v-6h6M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/></svg>
          Reverify All
        </button>
      </div>

      <div class="setup-list">
        {#each TOOLS as tool}
          <div class="setup-card" class:setup-card-installed={statuses[tool.id] === 'installed'}>
            <div class="setup-card-header">
              <span class="setup-card-icon">{@html tool.icon}</span>
              <div class="setup-card-info">
                <span class="setup-card-name">{tool.name}</span>
                <p class="setup-card-desc">{tool.description}</p>
              </div>
              <div class="setup-card-extra">
                <div class="setup-card-status">
                  {#if statuses[tool.id] === "checking"}
                    <span class="badge badge-checking">
                      <span class="spinner-tiny"></span>
                      Checking...
                    </span>
                  {:else if statuses[tool.id] === "installed"}
                    <span class="badge badge-installed">
                      <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" style="vertical-align:middle;margin-right:2px;"><polyline points="20 6 9 17 4 12"/></svg>
                      Installed
                    </span>
                  {:else if statuses[tool.id] === "not_detected"}
                    <span class="badge badge-missing">
                      <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" style="vertical-align:middle;margin-right:2px;"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
                      Not Detected
                    </span>
                  {:else}
                    <span class="badge badge-unknown">Unknown</span>
                  {/if}
                </div>
              </div>
            </div>
            <div class="setup-card-actions">
              <button 
                class="setup-btn-verify" 
                onclick={() => checkTool(tool.id, tool.checkCmd)}
                disabled={statuses[tool.id] === 'checking'}
              >
                Verify
              </button>
              <button 
                class="setup-btn-install" 
                class:setup-btn-install-disabled={statuses[tool.id] === 'installed'}
                onclick={() => runInstall(tool)}
                disabled={statuses[tool.id] === 'checking'}
              >
                <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="vertical-align:middle;margin-right:3px;"><polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/></svg>
                Auto Run Script
              </button>
            </div>
          </div>
        {/each}


      </div>
    </div>
  {:else if settingsTab === "shortcuts"}
    <div class="shortcuts-section">
      <div class="settings-header">
        <span class="settings-title">Keyboard Shortcuts</span>
        <div class="shortcut-search-wrap">
          <svg class="search-icon" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
          <input 
            type="text" 
            class="shortcut-search" 
            placeholder="Search shortcuts..." 
            bind:value={shortcutSearch} 
          />
        </div>
      </div>

      {#if editingShortcutId !== null}
        {@const editingItem = shortcuts.find(s => s.id === editingShortcutId)}
        {#if editingItem}
          <div class="settings-form">
            <div class="form-title">
              Configure Hotkey: <span class="accent-text">{editingItem.name}</span>
            </div>
            
            <div class="shortcut-editor-grid">
              <div class="form-field">
                <span>Modifiers</span>
                <div class="checkbox-row">
                  <label class="checkbox-container">
                    <input type="checkbox" bind:checked={editCtrl} />
                    <span class="checkmark"></span>
                    Ctrl
                  </label>
                  <label class="checkbox-container">
                    <input type="checkbox" bind:checked={editAlt} />
                    <span class="checkmark"></span>
                    Alt
                  </label>
                  <label class="checkbox-container">
                    <input type="checkbox" bind:checked={editShift} />
                    <span class="checkmark"></span>
                    Shift
                  </label>
                </div>
              </div>

              <div class="form-field">
                <span>Key Trigger</span>
                <div class="key-input-wrapper">
                  <input 
                    type="text" 
                    class="key-input"
                    bind:value={editKey} 
                    placeholder="Press keys directly here..." 
                    maxlength="12" 
                    onkeydown={(e) => {
                      if (e.key === "Tab" || e.key === "Escape") return;
                      e.preventDefault();
                      
                      editCtrl = e.ctrlKey;
                      editAlt = e.altKey;
                      editShift = e.shiftKey;
                      
                      const lowerKey = e.key.toLowerCase();
                      if (["control", "alt", "shift", "meta"].includes(lowerKey)) {
                        return;
                      }
                      
                      editKey = lowerKey;
                    }}
                  />
                  <span class="key-hint">Press keys directly in the box above to capture modifiers and the key.</span>
                </div>
              </div>
            </div>

            <div class="form-actions">
              <span class="form-hint">Shortcuts update globally and persist automatically across reloads.</span>
              <div class="form-btns">
                <button class="settings-btn settings-btn-cancel" onclick={cancelEditShortcut}>Cancel</button>
                <button 
                  class="settings-btn settings-btn-save" 
                  onclick={saveEditedShortcut} 
                  disabled={!editKey.trim()}
                >
                  Save Shortcut
                </button>
              </div>
            </div>
          </div>
        {/if}
      {/if}

      <div class="shortcuts-list">
        {#each filteredShortcuts as item}
          <div class="shortcut-card" class:shortcut-editing={editingShortcutId === item.id}>
            <div class="shortcut-card-info">
              <span class="shortcut-name">{item.name}</span>
              <p class="shortcut-desc">{item.description}</p>
            </div>
            
            <div class="shortcut-card-right">
              <div class="shortcut-badges">
                {#if item.binding.ctrl}
                  <kbd class="kbd-key">Ctrl</kbd>
                {/if}
                {#if item.binding.alt}
                  <kbd class="kbd-key">Alt</kbd>
                {/if}
                {#if item.binding.shift}
                  <kbd class="kbd-key">Shift</kbd>
                {/if}
                <kbd class="kbd-key trigger-key">{item.binding.key.toUpperCase()}</kbd>
              </div>

              <button class="shortcut-edit-btn" onclick={() => startEditShortcut(item)} title="Modify Keybinding">
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>
                Change
              </button>
            </div>
          </div>
        {/each}

        {#if filteredShortcuts.length === 0}
          <div class="no-shortcuts">
            <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><circle cx="12" cy="12" r="10"/><line x1="8" y1="12" x2="16" y2="12"/></svg>
            <p>No matching shortcuts found</p>
          </div>
        {/if}
      </div>
    </div>
  {:else if settingsTab === "about"}
    <div class="about-section">
      <div class="settings-header">
        <span class="settings-title">About</span>
      </div>
      <div class="about-content" style="position:relative; overflow:hidden;">
        <div class="about-particles" aria-hidden="true">
          {#each aboutParticles as p}
            <span
              class="about-particle"
              style="left:{p.x * 100}%; top:{p.y * 100}%; width:{p.s}rem; height:{p.s}rem; animation-duration:{p.d}s; background:{p.c}; color:{p.c};"
            ></span>
          {/each}
        </div>
        <div class="about-hero">
          <div class="about-logo">
            <img src="/nyx_logo.png" alt="NyxEdit Logo" class="about-logo-img" />
          </div>
          <h1 class="about-name">NyxEdit</h1>
          <p class="about-version">v0.1.0</p>
          <p class="about-desc">AI-native IoT workshop with split terminal, multi-agent chat, file viewer, PlatformIO integration, script/command runner, and code editor</p>
        </div>

        <div class="about-info">
          <div class="about-section-title">Tech Stack</div>
          <div class="about-tag-row">
            <span class="about-tag">Tauri v2</span>
            <span class="about-tag">Svelte 5</span>
            <span class="about-tag">Rust</span>
            <span class="about-tag">TypeScript</span>
            <span class="about-tag">xterm.js</span>
            <span class="about-tag">CodeMirror 6</span>
            <span class="about-tag">portable-pty</span>
          </div>
        </div>

        <div class="about-info">
          <div class="about-section-title">Features</div>
          <div class="about-features">
            <div class="about-feature">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--accent-green)" stroke-width="2"><polyline points="20 6 9 17 4 12"/></svg>
              <span>Multi-pane split terminal with CWD tracking</span>
            </div>
            <div class="about-feature">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--accent-green)" stroke-width="2"><polyline points="20 6 9 17 4 12"/></svg>
              <span>Multi-agent AI chat (All Api support)</span>
            </div>
            <div class="about-feature">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--accent-green)" stroke-width="2"><polyline points="20 6 9 17 4 12"/></svg>
              <span>Script / command runner configuration and execution panel</span>
            </div>
            <div class="about-feature">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--accent-green)" stroke-width="2"><polyline points="20 6 9 17 4 12"/></svg>
              <span>File explorer</span>
            </div>
            <div class="about-feature">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--accent-green)" stroke-width="2"><polyline points="20 6 9 17 4 12"/></svg>
              <span>Code editor with syntax highlighting & theme-aware colors (can adjust)</span>
            </div>
            <div class="about-feature">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--accent-green)" stroke-width="2"><polyline points="20 6 9 17 4 12"/></svg>
              <span>9 themes (Dracula, Nord, Tokyo Night, Catppuccin, etc.)</span>
            </div>
            <div class="about-feature">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--accent-green)" stroke-width="2"><polyline points="20 6 9 17 4 12"/></svg>
              <span>Platform IO integration (auto-detect, install, build, upload)</span>
            </div>
            <div class="about-feature">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--accent-green)" stroke-width="2"><polyline points="20 6 9 17 4 12"/></svg>
              <span>AI cost tracking with per-agent token & pricing breakdown</span>
            </div>
          </div>
        </div>

        <div class="about-info">
          <div class="about-section-title">Repository</div>
          <p class="about-repo">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M9 19c-5 1.5-5-2.5-7-3m14 6v-3.87a3.37 3.37 0 0 0-.94-2.61c3.14-.35 6.44-1.54 6.44-7A5.44 5.44 0 0 0 20 4.77 5.07 5.07 0 0 0 19.91 1S18.73.65 16 2.48a13.38 13.38 0 0 0-7 0C6.27.65 5.09 1 5.09 1A5.07 5.07 0 0 0 5 4.77a5.44 5.44 0 0 0-1.5 3.78c0 5.42 3.3 6.61 6.44 7A3.37 3.37 0 0 0 9 18.13V22"/></svg>
            <a href="https://github.com/saiflll/nyxedit" target="_blank" rel="noopener noreferrer">github.com/saiflll/nyxedit</a>
          </p>
        </div>
        <div class="about-info">
          <div class="about-section-title">Inspired</div>
          <p class="about-repo">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M9 19c-5 1.5-5-2.5-7-3m14 6v-3.87a3.37 3.37 0 0 0-.94-2.61c3.14-.35 6.44-1.54 6.44-7A5.44 5.44 0 0 0 20 4.77 5.07 5.07 0 0 0 19.91 1S18.73.65 16 2.48a13.38 13.38 0 0 0-7 0C6.27.65 5.09 1 5.09 1A5.07 5.07 0 0 0 5 4.77a5.44 5.44 0 0 0-1.5 3.78c0 5.42 3.3 6.61 6.44 7A3.37 3.37 0 0 0 9 18.13V22"/></svg>
            <a href="https://github.com/crynta/terax-ai" target="_blank" rel="noopener noreferrer">github.com/crynta/terax-ai</a>
          </p>
        </div>
        <div class="about-footer">
          <p>Built with &lt;3 by the Renagge39 aka Saiflll &middot; MIT License</p>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .settings { display:flex; flex-direction:column; height:100%; background:transparent; color:var(--text-primary); font-size:var(--font-size); overflow:hidden; }

  /* ═══ Sub-tabs ═══ */
  .settings-tabs { display:flex; gap:0; border-bottom:1px solid var(--border-subtle); flex-shrink:0; padding:0 8px; }
  .settings-tab { display:inline-flex; align-items:center; gap:5px; padding:8px 16px; background:none; border:none; border-bottom:2px solid transparent; color:var(--text-muted); font-size:var(--fs-11); font-weight:500; cursor:pointer; transition:all 0.12s ease; }
  .settings-tab:hover { color:var(--text-secondary); }
  .settings-tab.active { color:var(--accent-blue); border-bottom-color:var(--accent-blue); }

  /* ═══ Scrollable Tab Content Container Spacing ═══ */
  .appearance,
  .setup-list,
  .shortcuts-list {
    flex: 1;
    overflow-y: auto;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .setup-list {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    align-content: start;
  }
  .settings-list {
    flex: 1;
    overflow-y: auto;
    padding: 16px;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    align-content: start;
    gap: 12px;
  }

  .theme-card,
  .agent-card,
  .setup-card,
  .shortcut-card {
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: 10px;
    padding: 12px 16px;
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.05);
    transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
    box-sizing: border-box;
  }

  .theme-card:hover,
  .agent-card:hover,
  .setup-card:hover,
  .shortcut-card:hover {
    border-color: var(--accent-blue);
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.12);
    transform: translateY(-1.5px);
  }

  /* ═══ Scrollbar Overhauls ═══ */
  .appearance::-webkit-scrollbar,
  .settings-list::-webkit-scrollbar,
  .setup-list::-webkit-scrollbar,
  .shortcuts-list::-webkit-scrollbar {
    width: 6px;
    height: 6px;
  }
  .appearance::-webkit-scrollbar-thumb,
  .settings-list::-webkit-scrollbar-thumb,
  .setup-list::-webkit-scrollbar-thumb,
  .shortcuts-list::-webkit-scrollbar-thumb {
    background: var(--border-primary);
    border-radius: 3px;
  }
  .appearance::-webkit-scrollbar-thumb:hover,
  .settings-list::-webkit-scrollbar-thumb:hover,
  .setup-list::-webkit-scrollbar-thumb:hover,
  .shortcuts-list::-webkit-scrollbar-thumb:hover {
    background: var(--bg-hover);
  }

  /* ═══ Appearance Specifics ═══ */
  .section-title { font-size:var(--fs-10); font-weight:600; color:var(--text-muted); text-transform:uppercase; letter-spacing:0.8px; margin-bottom:4px; margin-top:4px; }
  .section-title:not(:first-child) { margin-top:12px; }

  .theme-grid { display:grid; grid-template-columns:repeat(auto-fill, minmax(130px, 1fr)); gap:10px; }
  .theme-card {
    position:relative; display:flex; flex-direction:column; align-items:center; gap:6px;
    padding:10px 8px 8px; cursor:pointer;
  }
  .theme-active { border-color:var(--accent-blue) !important; background:color-mix(in srgb, var(--accent-blue) 8%, var(--bg-surface)) !important; }
  .theme-preview { width:100%; height:46px; border-radius:6px; overflow:hidden; }
  .tp-bg { width:100%; height:100%; display:flex; border-radius:4px; overflow:hidden; }
  .tp-side { width:14px; border-right:1px solid; flex-shrink:0; }
  .tp-main { flex:1; display:flex; flex-direction:column; justify-content:center; gap:4px; padding:0 6px; }
  .tp-line { height:4px; border-radius:2px; opacity:0.7; }
  .theme-name { font-size:var(--fs-10); font-weight:500; color:var(--text-primary); }
  .theme-check { position:absolute; top:4px; right:6px; font-size:var(--font-size); color:var(--accent-blue); }

  .font-section { display:flex; flex-direction:column; gap:8px; }
  .font-select { background:var(--bg-primary); color:var(--text-primary); border:1px solid var(--border-subtle); border-radius:6px; padding:6px 10px; font-size:var(--font-size); }
  .font-select:focus { outline:none; border-color:var(--accent-blue); }
  .font-preview { background:var(--bg-primary); border:1px solid var(--border-subtle); border-radius:6px; padding:10px 12px; font-size:var(--fs-14); color:var(--accent-blue); text-align:center; }

  /* ═══ Agent Specifics ═══ */
  .agent-section { display:flex; flex-direction:column; flex:1; overflow:hidden; }
  .settings-header { display:flex; align-items:center; justify-content:space-between; padding:10px 16px; border-bottom:1px solid var(--border-subtle); flex-shrink:0; }
  .settings-title { font-size:var(--fs-10); font-weight:600; color:var(--text-muted); text-transform:uppercase; letter-spacing:0.8px; }
  
  /* Consolidated Interactive Button Design */
  .settings-btn, .setup-btn-verify, .setup-btn-install, .shortcut-edit-btn {
    display:inline-flex; align-items:center; justify-content:center; gap:6px;
    border:1px solid transparent; border-radius:6px; padding:6px 12px; font-size:var(--fs-11);
    font-weight:500; cursor:pointer; transition:all 0.15s ease;
  }
  .settings-btn-add { background:var(--accent-blue); color:var(--bg-primary); }
  .settings-btn-add:disabled { opacity:0.4; cursor:not-allowed; }
  .settings-btn-add:hover:not(:disabled) { filter:brightness(1.15); box-shadow:0 0 8px rgba(129, 140, 248, 0.25); }
  
  .fetch-error { background:color-mix(in srgb, var(--accent-red) 10%, transparent); border:1px solid var(--accent-red); border-radius:6px; padding:6px 10px; font-size:var(--fs-10); color:var(--accent-red); margin-bottom:8px; word-break:break-all; }
  .form-hint-inline { font-weight:400; text-transform:none; letter-spacing:0; color:var(--text-muted); font-size:var(--fs-9); }

  .settings-btn-cancel { background:transparent; color:var(--text-muted); border-color:var(--border-subtle); }
  .settings-btn-cancel:hover { color:var(--text-primary); background:var(--bg-hover); }
  
  .settings-btn-save { background:var(--accent-green); color:var(--bg-primary); }
  .settings-btn-save:disabled { opacity:0.4; cursor:not-allowed; }
  .settings-btn-save:hover:not(:disabled) { filter:brightness(1.15); }

  .settings-form { padding:14px 16px; background:var(--bg-surface); border-bottom:1px solid var(--border-subtle); flex-shrink:0; animation:slideDown 0.2s cubic-bezier(0.4, 0, 0.2, 1); }
  @keyframes slideDown { from { opacity:0; transform: translateY(-4px); } to { opacity:1; transform: translateY(0); } }
  .form-field { display:flex; flex-direction:column; gap:4px; }
  .form-field span { font-size:var(--fs-10); color:var(--text-muted); text-transform:uppercase; letter-spacing:0.5px; }
  .form-field input, .form-field select { background:var(--bg-primary); color:var(--text-primary); border:1px solid var(--border-subtle); border-radius:6px; padding:6px 10px; font-size:var(--font-size); }
  .form-field input:focus, .form-field select:focus { outline:none; border-color:var(--accent-blue); box-shadow:0 0 0 2px color-mix(in srgb, var(--accent-blue) 15%, transparent); }
  .agent-form-grid { display:grid; grid-template-columns:1fr 1fr; gap:10px; margin-bottom:10px; }
  .form-field-full { grid-column:1 / -1; }
  .form-textarea { background:var(--bg-primary); color:var(--text-primary); border:1px solid var(--border-subtle); border-radius:6px; padding:6px 10px; font-size:var(--font-size); resize:vertical; font-family:inherit; width:100%; box-sizing:border-box; }
  .form-textarea:focus { outline:none; border-color:var(--accent-blue); box-shadow:0 0 0 2px color-mix(in srgb, var(--accent-blue) 15%, transparent); }
  .model-input-row { display:flex; gap:4px; align-items:center; }
  .model-input-row select { flex:1; background:var(--bg-primary); color:var(--text-primary); border:1px solid var(--border-subtle); border-radius:6px; padding:6px 10px; font-size:var(--font-size); }
  .model-input-row select:focus { outline:none; border-color:var(--accent-blue); }
  .model-input-row input { flex:1; background:var(--bg-primary); color:var(--text-primary); border:1px solid var(--border-subtle); border-radius:6px; padding:6px 10px; font-size:var(--font-size); }
  .model-input-row input:focus { outline:none; border-color:var(--accent-blue); }
  .settings-btn-sm { display:inline-flex; align-items:center; justify-content:center; background:var(--bg-hover); color:var(--text-muted); border:1px solid var(--border-subtle); border-radius:6px; padding:4px 6px; cursor:pointer; transition:all 0.12s ease; flex-shrink:0; }
  .settings-btn-sm:hover { color:var(--accent-blue); border-color:var(--accent-blue); background:color-mix(in srgb, var(--accent-blue) 6%, transparent); }
  .temp-row { display:flex; align-items:center; gap:8px; }
  .temp-slider { flex:1; accent-color:var(--accent-blue); height:4px; cursor:pointer; }
  .temp-label { font-size:var(--fs-11); color:var(--text-muted); min-width:28px; text-align:center; font-variant-numeric:tabular-nums; }

  .agent-card { display:flex; flex-direction:column; gap:8px; }
  .agent-header { display:flex; align-items:center; gap:8px; margin-bottom:2px; }
  .agent-header-badges { display:flex; gap:4px; align-items:center; }
  .agent-info { flex:1; min-width:0; }
  .agent-name { font-weight:600; font-size:var(--font-size); color:var(--text-primary); }
  .agent-meta { font-size:var(--fs-10); color:var(--text-muted); margin-left:6px; }
  .agent-badge { font-size:var(--fs-9); padding:2px 6px; border-radius:3px; }
  .agent-badge-key { font-size:var(--font-size); padding:0; background:none; }
  .agent-badge-persona { background:color-mix(in srgb, var(--accent-purple, #a855f7) 12%, transparent); color:var(--accent-purple, #a855f7); border:1px solid color-mix(in srgb, var(--accent-purple, #a855f7) 20%, transparent); }
  .agent-badge-builtin { background:color-mix(in srgb, var(--accent-green, #22c55e) 12%, transparent); color:var(--accent-green, #22c55e); border:1px solid color-mix(in srgb, var(--accent-green, #22c55e) 20%, transparent); }
  .agent-url { font-size:var(--fs-10); color:var(--text-muted); font-family:monospace; margin-bottom:2px; padding:4px 8px; background:var(--bg-primary); border-radius:4px; border:1px solid var(--border-subtle); }
  .agent-actions { display:flex; gap:6px; margin-top:2px; }
  .agent-action { display:inline-flex; align-items:center; gap:4px; border:1px solid var(--border-subtle); border-radius:6px; padding:4px 10px; font-size:var(--fs-10); cursor:pointer; background:transparent; transition:all 0.12s ease; }
  .agent-action-edit { color:var(--accent-blue); }
  .agent-action-edit:hover { border-color:var(--accent-blue); background:color-mix(in srgb, var(--accent-blue) 6%, transparent); }
  .agent-action-del { color:var(--accent-red); }
  .agent-action-del:hover { border-color:var(--accent-red); background:color-mix(in srgb, var(--accent-red) 6%, transparent); }

  /* ═══ Developer Setup Specifics ═══ */
  .setup-section { display:flex; flex-direction:column; flex:1; overflow:hidden; }
  
  .setup-card {
    display:flex; flex-direction:column; gap:8px;
    padding: 10px 14px;
  }
  .setup-card-installed { border-left:4px solid var(--accent-green) !important; }
  
  .setup-card-header {
    display:flex; align-items:center; gap:10px;
    min-width:0;
  }
  .setup-card-icon {
    font-size:var(--fs-14); display:inline-flex; align-items:center; justify-content:center;
    width:28px; height:28px; border-radius:6px; background:var(--bg-primary); border:1px solid var(--border-subtle);
    flex-shrink:0;
  }
  .setup-card-info { display:flex; flex-direction:column; gap:1px; min-width:0; flex:1; }
  .setup-card-name { font-size:var(--font-size); font-weight:600; color:var(--text-primary); white-space:nowrap; }
  .setup-card-desc { font-size:var(--fs-9-5); color:var(--text-muted); margin:0; line-height:1.3; white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }
  .setup-card-extra { flex-shrink:0; }
  
  .setup-card-status { display:flex; align-items:center; }
  .badge { display:inline-flex; align-items:center; gap:5px; padding:3px 8px; border-radius:9999px; font-size:var(--fs-10); font-weight:500; letter-spacing:0.2px; }
  .badge-checking { background:color-mix(in srgb, var(--accent-blue) 12%, transparent); color:var(--accent-blue); border:1px solid color-mix(in srgb, var(--accent-blue) 30%, transparent); }
  .badge-installed { background:color-mix(in srgb, var(--accent-green) 12%, transparent); color:var(--accent-green); border:1px solid color-mix(in srgb, var(--accent-green) 30%, transparent); }
  .badge-missing { background:color-mix(in srgb, var(--accent-red) 12%, transparent); color:var(--accent-red); border:1px solid color-mix(in srgb, var(--accent-red) 30%, transparent); }
  .badge-unknown { background:var(--bg-primary); color:var(--text-muted); border:1px solid var(--border-subtle); }
  
  .spinner-tiny {
    width:10px; height:10px; border:1.5px solid var(--accent-blue); border-top-color:transparent;
    border-radius:50%; animation:spin 0.6s linear infinite; display:inline-block; margin-right:4px;
  }
  @keyframes spin { to { transform: rotate(360deg); } }
  
  .setup-card-actions { display: flex; gap: 6px; }
  
  .setup-btn-verify { background: transparent; color: var(--text-secondary); border: 1px solid var(--border-subtle); }
  .setup-btn-verify:hover:not(:disabled) { border-color: var(--accent-blue); color: var(--accent-blue); background: color-mix(in srgb, var(--accent-blue) 6%, transparent); }
  .setup-btn-verify:disabled { opacity: 0.5; cursor: not-allowed; }
  
  .setup-btn-install { background: var(--bg-elevated); color: var(--accent-blue); border: 1px solid var(--border-subtle); }
  .setup-btn-install:hover:not(:disabled) { background: var(--bg-hover); border-color: var(--accent-blue); }
  .setup-btn-install-disabled { background:var(--bg-primary); color:var(--text-muted); border:1px solid var(--border-subtle); opacity:0.6; cursor:not-allowed; }



  /* ═══ Keyboard Shortcuts Specifics ═══ */
  .shortcuts-section { display:flex; flex-direction:column; flex:1; overflow:hidden; }
  
  .shortcut-search-wrap { position:relative; display:flex; align-items:center; }
  .shortcut-search {
    background:var(--bg-primary); color:var(--text-primary); border:1px solid var(--border-subtle);
    border-radius:6px; padding:6px 8px 6px 26px; font-size:var(--fs-11); width:160px; transition:all 0.15s ease;
  }
  .shortcut-search:focus { outline:none; border-color:var(--accent-blue); width:200px; box-shadow:0 0 0 2px color-mix(in srgb, var(--accent-blue) 15%, transparent); }
  .search-icon { position:absolute; left:8px; color:var(--text-muted); pointer-events:none; }
  
  .shortcut-card {
    display:flex; justify-content:space-between; align-items:center; gap:16px;
  }
  .shortcut-editing { border-color:var(--accent-blue) !important; background:color-mix(in srgb, var(--accent-blue) 4%, var(--bg-surface)) !important; }
  
  .shortcut-card-info { display:flex; flex-direction:column; gap:2px; }
  .shortcut-name { font-size:var(--font-size); font-weight:600; color:var(--text-primary); }
  .shortcut-desc { font-size:var(--fs-10); color:var(--text-muted); margin:0; }
  
  .shortcut-card-right { display:flex; align-items:center; gap:12px; }
  .shortcut-badges { display:flex; align-items:center; gap:4px; }
  
  .kbd-key {
    display:inline-flex; align-items:center; justify-content:center;
    background:var(--bg-primary); color:var(--text-secondary);
    border:1px solid var(--border-primary); border-bottom-width:2px;
    border-radius:4px; padding:2px 6px; font-family:monospace; font-size:var(--fs-9-5); font-weight:600;
    box-shadow:0 1px 2px rgba(0,0,0,0.15); text-transform:uppercase; min-width:22px; text-align:center;
  }
  .trigger-key {
    background:color-mix(in srgb, var(--accent-blue) 12%, transparent);
    color:var(--accent-blue); border-color:color-mix(in srgb, var(--accent-blue) 35%, transparent);
  }
  
  .shortcut-edit-btn { background:transparent; color:var(--text-secondary); border:1px solid var(--border-subtle); }
  .shortcut-edit-btn:hover { border-color:var(--accent-blue); color:var(--accent-blue); background:color-mix(in srgb, var(--accent-blue) 6%, transparent); }
  
  .no-shortcuts { display:flex; flex-direction:column; align-items:center; justify-content:center; gap:8px; padding:32px 0; color:var(--text-muted); }
  .no-shortcuts p { margin:0; font-size:var(--fs-11); }

  /* Modifier checkboxes / Editor layout */
  .form-title { font-size:var(--fs-11); font-weight:600; color:var(--text-primary); margin-bottom:8px; }
  .accent-text { color:var(--accent-blue); }
  .shortcut-editor-grid { display:grid; grid-template-columns:1.2fr 1fr; gap:16px; margin-bottom:10px; }
  
  .checkbox-row { display:flex; gap:12px; margin-top:4px; }
  .checkbox-container {
    position:relative; display:inline-flex; align-items:center; gap:6px;
    font-size:var(--fs-11); color:var(--text-secondary); cursor:pointer; user-select:none;
  }
  .checkbox-container input { display:none; }
  .checkmark {
    width:14px; height:14px; border:1px solid var(--border-subtle); border-radius:3px;
    background:var(--bg-primary); display:inline-block; position:relative; transition:all 0.12s ease;
  }
  .checkbox-container:hover input ~ .checkmark { border-color:var(--accent-blue); }
  .checkbox-container input:checked ~ .checkmark { background:var(--accent-blue); border-color:var(--accent-blue); }
  .checkmark:after {
    content:""; position:absolute; display:none;
    left:4px; top:1px; width:4px; height:7px;
    border:solid var(--bg-primary); border-width:0 2px 2px 0; transform:rotate(45deg);
  }
  .checkbox-container input:checked ~ .checkmark:after { display:block; }
  
  .key-input-wrapper { display:flex; flex-direction:column; gap:4px; }
  .key-input {
    background:var(--bg-primary); color:var(--accent-blue); font-family:monospace; font-weight:600;
    border:1px solid var(--border-subtle); border-radius:6px; padding:6px 10px; font-size:var(--fs-11); text-transform:uppercase; text-align:center;
    letter-spacing:1px; box-shadow:inset 0 1px 3px rgba(0,0,0,0.1); width:100%; box-sizing:border-box;
  }
  .key-input:focus { outline:none; border-color:var(--accent-blue); box-shadow:0 0 0 2px color-mix(in srgb, var(--accent-blue) 15%, transparent); }
  .key-hint { font-size:var(--fs-9-5); color:var(--text-muted); line-height:1.3; }

  /* Font Size */
  .fontsize-section { display:flex; align-items:center; gap:10px; padding:6px 0; }
  .fontsize-slider { flex:1; height:4px; appearance:none; -webkit-appearance:none; background:var(--bg-elevated); border-radius:2px; outline:none; cursor:pointer; }
  .fontsize-slider::-webkit-slider-thumb { appearance:none; -webkit-appearance:none; width:14px; height:14px; border-radius:50%; background:var(--accent-blue); border:2px solid var(--bg-primary); cursor:pointer; }
  .fontsize-label { font-size:var(--font-size); color:var(--text-primary); font-family:monospace; min-width:32px; text-align:right; }

  /* About */
  .about-section { display:flex; flex-direction:column; flex:1; overflow:hidden; }
  .about-content { position:relative; display:flex; flex-direction:column; flex:1; overflow-y:auto; padding:16px 20px; gap:16px; }
  .about-hero { position:relative; z-index:1; display:flex; flex-direction:column; align-items:center; gap:8px; text-align:center; }
  .about-logo { opacity:1; display:flex; align-items:center; justify-content:center; }
  .about-logo-img { width:72px; height:72px; border-radius:14px; object-fit:contain; box-shadow:0 0 24px rgba(0,224,255,0.35), 0 0 8px rgba(129,140,248,0.2); }
  .about-name { font-size:var(--fs-22); font-weight:700; color:var(--text-primary); margin:0; }
  .about-version { font-size:var(--font-size); color:var(--text-muted); font-family:monospace; }
  .about-desc { font-size:var(--font-size); color:var(--text-secondary); max-width:360px; line-height:1.5; }
  .about-info { position:relative; z-index:1; }
  .about-section-title { font-size:var(--fs-10); font-weight:600; color:var(--text-muted); text-transform:uppercase; letter-spacing:0.8px; margin-bottom:8px; }
  .about-tag-row { display:flex; flex-wrap:wrap; gap:6px; }
  .about-tag { font-size:var(--fs-10); padding:3px 10px; background:color-mix(in srgb, var(--accent-blue) 10%, transparent); color:var(--accent-blue); border:1px solid color-mix(in srgb, var(--accent-blue) 20%, transparent); border-radius:10px; font-family:monospace; }
  .about-features { display:flex; flex-direction:column; gap:5px; }
  .about-feature { display:flex; align-items:center; gap:8px; font-size:var(--fs-11); color:var(--text-secondary); }
  .about-repo { display:flex; align-items:center; gap:6px; font-size:var(--fs-11); color:var(--text-secondary); }
  .about-repo a { color:var(--accent-blue); text-decoration:none; }
  .about-repo a:hover { text-decoration:underline; }
  .about-particles { position:absolute; inset:0; pointer-events:none; z-index:0; overflow:hidden; }
  .about-particle {
    position:absolute; border-radius:50%;
    animation-name: about-float;
    animation-duration: 4s;
    animation-timing-function: ease-in-out;
    animation-iteration-count: infinite;
    animation-direction: alternate;
    opacity:0.45;
    box-shadow:0 0 4px currentColor;
    color:inherit;
  }
  @keyframes about-float {
    0% { transform:translateY(0) translateX(0) scale(1); opacity:0.3; }
    100% { transform:translateY(-8px) translateX(4px) scale(1.2); opacity:0.6; }
  }
  .about-footer { margin-top:auto; text-align:center; font-size:var(--fs-10); color:var(--text-muted); padding-top:16px; }

  /* Backdrop Settings styles */
  .bg-section { display:flex; flex-direction:column; gap:8px; padding:8px 0; max-width:480px; }
</style>
