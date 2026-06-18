<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import {
    activeTerminalSessionId,
    type PtyOutputEvent,
  } from "../stores.svelte";

  let {
    sessionId = $bindable(),
    rows = 24,
    cols = 80,
    shell = undefined as string | undefined,
    password = undefined as string | undefined,
    label = "",
    onReady = (_id: string) => {},
    onCommand = (_id: string, _line: string) => {},
  } = $props();

  let terminalEl: HTMLDivElement = $state()!;
  let terminal: any = null;
  let fitAddon: any = null;
  let unlisten: UnlistenFn | null = null;
  let resizeObserver: ResizeObserver | null = null;
  let activeSessionId = $state<string | null>(null);
  let themeObserver: MutationObserver | null = null;
  let passwordSent = $state(false);
  const bufferedOutput = new Map<string, string>();

  let pendingPaste = $state<string | null>(null);
  let pasteConfirmVisible = $state(false);
  let contextMenu = $state<{ x: number; y: number } | null>(null);

  $effect(() => {
    if (sessionId) {
      activeSessionId = sessionId;
      activeTerminalSessionId.set(sessionId);
    }
  });

  function triggerPaste(text: string) {
    if (!text || !activeSessionId) return;
    pendingPaste = text;
    pasteConfirmVisible = true;
  }

  function confirmPaste() {
    if (pendingPaste && activeSessionId) {
      invoke("pty_write", { sessionId: activeSessionId, data: pendingPaste });
    }
    pendingPaste = null;
    pasteConfirmVisible = false;
  }

  function cancelPaste() {
    pendingPaste = null;
    pasteConfirmVisible = false;
  }

  function handleCopy() {
    if (terminal) {
      const sel = terminal.getSelection();
      if (sel) navigator.clipboard.writeText(sel).catch(() => {});
    }
    contextMenu = null;
  }

  function handleSelectAll() {
    if (terminal) terminal.selectAll();
    contextMenu = null;
  }

  function handleContextPaste() {
    contextMenu = null;
    navigator.clipboard.readText().then((text) => {
      if (text) triggerPaste(text);
    }).catch(() => {});
  }

  function closeContextMenu() {
    contextMenu = null;
  }

  onMount(async () => {
    const { Terminal } = await import("xterm");
    const { FitAddon } = await import("xterm-addon-fit");

    const s = getComputedStyle(document.documentElement);
    function c(name: string, fallback: string) {
      return s.getPropertyValue(name).trim() || fallback;
    }

    terminal = new Terminal({
      cursorBlink: true,
      cursorStyle: "block",
      fontSize: parseInt(localStorage.getItem("nyxedit-font-size") || "12", 10),
      fontFamily:
        localStorage.getItem("nyxedit-font") ||
        "'Cascadia Code', 'Fira Code', 'Consolas', monospace",
      theme: {
        background: "rgba(0,0,0,0)",
        foreground: c("--text-primary", "#c0caf5"),
        cursor: c("--accent-blue", "#ff3366"),
        cursorAccent: c("--bg-primary", "#0d0d1a"),
        selectionBackground: c("--accent-blue", "#ff3366") + "40",
        black: c("--bg-elevated", "#1a1b3e"),
        red: c("--accent-red", "#f87171"),
        green: c("--accent-green", "#4ade80"),
        yellow: c("--accent-yellow", "#fbbf24"),
        blue: c("--accent-blue", "#818cf8"),
        magenta: c("--accent-purple", "#a78bfa"),
        cyan: c("--accent-cyan", "#22d3ee"),
        white: c("--text-secondary", "#94a3b8"),
        brightBlack: c("--text-muted", "#64748b"),
        brightRed: c("--accent-red", "#f87171"),
        brightGreen: c("--accent-green", "#4ade80"),
        brightYellow: c("--accent-yellow", "#fbbf24"),
        brightBlue: c("--accent-blue", "#818cf8"),
        brightMagenta: c("--accent-purple", "#a78bfa"),
        brightCyan: c("--accent-cyan", "#22d3ee"),
        brightWhite: c("--text-primary", "#e2e8f0"),
      },
      allowTransparency: true,
      convertEol: true,
      scrollback: 10000,
      lineHeight: 1.1,
      letterSpacing: 0,
    });

    fitAddon = new FitAddon();
    terminal.loadAddon(fitAddon);

    if (!terminalEl) return;
    terminal.open(terminalEl);

    let pendingData = "";
    let animationFrameId: number | null = null;

    function flushTerminalData() {
      if (pendingData && terminal) {
        terminal.write(pendingData);
        pendingData = "";
      }
      animationFrameId = null;
    }

    unlisten = await listen<PtyOutputEvent>("pty-output", (event) => {
      const { session_id, data } = event.payload;
      if (session_id === activeSessionId) {
        if (password && !passwordSent && /password:/i.test(data)) {
          passwordSent = true;
          invoke("pty_write", { sessionId: activeSessionId, data: password + "\n" });
        }
        pendingData += data;
        if (animationFrameId === null) {
          animationFrameId = requestAnimationFrame(flushTerminalData);
        }
      } else {
        bufferedOutput.set(session_id, (bufferedOutput.get(session_id) || "") + data);
      }
    });

    if (!sessionId) {
      await openTerminal();
    } else {
      activeSessionId = sessionId;
      if (bufferedOutput.has(sessionId)) {
        terminal.write(bufferedOutput.get(sessionId)!);
        bufferedOutput.delete(sessionId);
      }
    }

    let resizeTimeout: any = null;
    resizeObserver = new ResizeObserver(() => {
      if (resizeTimeout) clearTimeout(resizeTimeout);
      resizeTimeout = setTimeout(() => {
        if (
          fitAddon &&
          terminalEl &&
          terminalEl.clientWidth > 0 &&
          terminalEl.clientHeight > 0
        ) {
          try {
            fitAddon.fit();
          } catch (e) {
            console.warn("xterm fit failed:", e);
          }
        }
        resizeTimeout = null;
      }, 30);
    });
    resizeObserver.observe(terminalEl);

    setTimeout(() => {
      if (
        fitAddon &&
        terminalEl &&
        terminalEl.clientWidth > 0 &&
        terminalEl.clientHeight > 0
      ) {
        try {
          fitAddon.fit();
        } catch (e) {}
      }
    }, 100);

    function updateTermTheme() {
      if (!terminal) return;
      const st = getComputedStyle(document.documentElement);
      function cv(name: string, fb: string) {
        return st.getPropertyValue(name).trim() || fb;
      }

      const storedFont =
        localStorage.getItem("nyxedit-font") ||
        "'Cascadia Code', 'Fira Code', 'Consolas', monospace";
      terminal.options.fontFamily = storedFont;
      terminal.options.fontSize = parseInt(localStorage.getItem("nyxedit-font-size") || "12", 10);

      terminal.options.theme = {
        background: "rgba(0,0,0,0)",
        foreground: cv("--text-primary", "#c0caf5"),
        cursor: cv("--accent-blue", "#ff3366"),
        cursorAccent: cv("--bg-primary", "#0d0d1a"),
        selectionBackground: cv("--accent-blue", "#ff3366") + "40",
        black: cv("--bg-elevated", "#1a1b3e"),
        red: cv("--accent-red", "#f87171"),
        green: cv("--accent-green", "#4ade80"),
        yellow: cv("--accent-yellow", "#fbbf24"),
        blue: cv("--accent-blue", "#818cf8"),
        magenta: cv("--accent-purple", "#a78bfa"),
        cyan: cv("--accent-cyan", "#22d3ee"),
        white: cv("--text-secondary", "#94a3b8"),
        brightBlack: cv("--text-muted", "#64748b"),
        brightRed: cv("--accent-red", "#f87171"),
        brightGreen: cv("--accent-green", "#4ade80"),
        brightYellow: cv("--accent-yellow", "#fbbf24"),
        brightBlue: cv("--accent-blue", "#818cf8"),
        brightMagenta: cv("--accent-purple", "#a78bfa"),
        brightCyan: cv("--accent-cyan", "#22d3ee"),
        brightWhite: cv("--text-primary", "#e2e8f0"),
      };
    }
    themeObserver = new MutationObserver(updateTermTheme);
    themeObserver.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["style"],
    });

    let currentLine = "";
    terminal.onData((data: string) => {
      if (pasteConfirmVisible) {
        if (data === "\r" || data === "\n") {
          confirmPaste();
        } else if (data === "\x1b" || data === "\x03") {
          cancelPaste();
        }
        return;
      }

      if (activeSessionId) {
        invoke("pty_write", { sessionId: activeSessionId, data });
      }

      for (let i = 0; i < data.length; i++) {
        const char = data[i];
        if (char === "\r" || char === "\n") {
          if (currentLine.trim() && activeSessionId) {
            onCommand(activeSessionId, currentLine);
          }
          currentLine = "";
        } else if (char === "\x7f") {
          currentLine = currentLine.slice(0, -1);
        } else if (char.charCodeAt(0) >= 32) {
          currentLine += char;
        }
      }
    });

    terminal.onResize(({ cols, rows }: { cols: number; rows: number }) => {
      if (activeSessionId) {
        invoke("pty_resize", { sessionId: activeSessionId, rows, cols });
      }
    });

    terminal.attachCustomKeyEventHandler((e: { event: KeyboardEvent; type: string }) => {
      const { event } = e;
      const isPaste =
        ((event.ctrlKey || event.metaKey) && event.key === "v") ||
        (event.shiftKey && event.key === "Insert");
      if (isPaste) {
        navigator.clipboard.readText().then((text) => {
          if (text) triggerPaste(text);
        }).catch(() => {});
        return false;
      }
      return true;
    });

    terminalEl.addEventListener("contextmenu", (e) => {
      e.preventDefault();
      contextMenu = { x: e.clientX, y: e.clientY };
    });

    terminalEl.addEventListener("paste", (e) => {
      e.preventDefault();
      const text = e.clipboardData?.getData("text/plain");
      if (text) triggerPaste(text);
    });

    document.addEventListener("click", closeContextMenu);
    document.addEventListener("keydown", (e) => {
      if (e.key === "Escape") closeContextMenu();
    });
  });

  onDestroy(() => {
    if (unlisten) unlisten();
    if (terminal) terminal.dispose();
    if (resizeObserver) resizeObserver.disconnect();
    if (themeObserver) themeObserver.disconnect();
    if (activeSessionId) {
      invoke("pty_close", { sessionId: activeSessionId }).catch((e) => {
        console.error("Failed to close PTY session on destroy:", e);
      });
    }
  });

  async function openTerminal() {
    const id = await invoke<string>("pty_open", {
      shell: shell ?? null,
      rows: terminal.rows,
      cols: terminal.cols,
      label: label || null,
    });
    sessionId = id;
    activeSessionId = id;
    activeTerminalSessionId.set(id);
    onReady(id);

    if (bufferedOutput.has(id)) {
      terminal.write(bufferedOutput.get(id)!);
      bufferedOutput.delete(id);
    }

    setTimeout(() => {
      if (fitAddon) {
        try {
          fitAddon.fit();
        } catch (e) {}
      }
    }, 150);

    if (label) {
      setTimeout(() => {
        const isWin = navigator.userAgent.toLowerCase().includes("win");
        if (!isWin) {
          invoke("pty_write", {
            sessionId: id,
            data: `export PS1="\\[\\033[94m\\][${label}]\\[\\033[96m\\]\\w\\[\\033[92m\\]>\\[\\033[0m\\] "\nclear\n`,
          }).catch(() => {});
        }
      }, 500);
    }
  }

  function fitTerminal() {
    if (fitAddon) fitAddon.fit();
  }
</script>

<div
  class="term"
  role="presentation"
  onclick={() => {
    if (activeSessionId) activeTerminalSessionId.set(activeSessionId);
  }}
  onfocusin={() => {
    if (activeSessionId) activeTerminalSessionId.set(activeSessionId);
  }}
>
  <div bind:this={terminalEl} class="term-instance"></div>

  {#if pasteConfirmVisible}
    <div class="paste-confirm" role="alert" onclick={confirmPaste}>
      <svg class="pc-icon" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <rect x="8" y="2" width="8" height="4" rx="1" ry="1"/>
        <path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2"/>
        <rect x="10" y="9" width="4" height="4" rx="1"/>
      </svg>
      <span class="pc-label">Paste ~{pendingPaste?.split('\n').length || 0} lines</span>
      <span class="pc-hint">
        <span class="pc-key">Enter</span> confirm
        <span class="pc-key">Esc</span> cancel
      </span>
    </div>
  {/if}

  {#if contextMenu}
    <div
      class="ctx-menu"
      style="left: {contextMenu.x}px; top: {contextMenu.y}px;"
      onclick={(e) => e.stopPropagation()}
      role="menu"
    >
      <button class="ctx-item" role="menuitem" onclick={handleCopy}>
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <rect x="9" y="9" width="13" height="13" rx="2" ry="2"/>
          <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
        </svg>
        Copy
      </button>
      <button class="ctx-item" role="menuitem" onclick={handleContextPaste}>
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <rect x="8" y="2" width="8" height="4" rx="1" ry="1"/>
          <path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2"/>
          <rect x="10" y="9" width="4" height="4" rx="1"/>
        </svg>
        Paste
      </button>
      <div class="ctx-divider" role="separator"></div>
      <button class="ctx-item" role="menuitem" onclick={handleSelectAll}>
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <rect x="3" y="3" width="18" height="18" rx="2" ry="2"/>
          <line x1="9" y1="3" x2="9" y2="21"/>
          <line x1="15" y1="3" x2="15" y2="21"/>
        </svg>
        Select All
      </button>
    </div>
  {/if}
</div>

<style>
  .term {
    width: 100%;
    height: 100%;
    background: transparent;
    border-radius: 6px;
    overflow: hidden;
    border: 1px solid var(--border-primary);
    transition: border-color 0.2s;
    position: relative;
  }
  .term:focus-within {
    border-color: var(--accent-blue);
  }
  .term-instance {
    width: 100%;
    height: 100%;
  }
  .term :global(.xterm) {
    height: 100%;
    padding: 4px;
  }
  .term :global(.xterm-viewport) {
    scrollbar-width: thin;
    scrollbar-color: var(--accent-blue) transparent;
  }

  .paste-confirm {
    position: absolute;
    bottom: 8px;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 14px;
    background: color-mix(in srgb, var(--accent-blue) 12%, var(--bg-elevated));
    border: 1px solid var(--accent-blue);
    border-radius: 8px;
    cursor: pointer;
    transition: all 0.15s ease;
    z-index: 10;
    backdrop-filter: blur(8px);
    white-space: nowrap;
    user-select: none;
    box-shadow: 0 4px 16px rgba(0,0,0,0.35);
  }
  .paste-confirm:hover {
    background: color-mix(in srgb, var(--accent-blue) 20%, var(--bg-elevated));
    border-color: var(--accent-cyan);
  }
  .pc-icon {
    color: var(--accent-blue);
    flex-shrink: 0;
  }
  .pc-label {
    font-size: var(--font-size, 12px);
    font-weight: 600;
    color: var(--text-primary);
    letter-spacing: 0.02em;
  }
  .pc-hint {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 10px;
    color: var(--text-muted);
  }
  .pc-key {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 18px;
    height: 16px;
    padding: 0 4px;
    background: var(--bg-surface);
    border: 1px solid var(--border-primary);
    border-radius: 3px;
    font-size: 9px;
    font-weight: 600;
    color: var(--text-secondary);
    line-height: 1;
  }

  .ctx-menu {
    position: fixed;
    z-index: 1000;
    min-width: 150px;
    background: var(--bg-elevated);
    border: 1px solid var(--border-primary);
    border-radius: 8px;
    padding: 4px;
    box-shadow: 0 8px 24px rgba(0,0,0,0.4);
    backdrop-filter: blur(12px);
    overflow: hidden;
  }
  .ctx-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 7px 10px;
    background: none;
    border: none;
    border-radius: 5px;
    color: var(--text-primary);
    font-size: var(--font-size, 12px);
    cursor: pointer;
    transition: background 0.08s ease;
    text-align: left;
    line-height: 1;
  }
  .ctx-item:hover {
    background: var(--bg-hover);
  }
  .ctx-item svg {
    flex-shrink: 0;
    color: var(--text-muted);
  }
  .ctx-divider {
    height: 1px;
    margin: 4px 8px;
    background: var(--border-primary);
  }
</style>
