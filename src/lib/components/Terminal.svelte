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

  $effect(() => {
    if (sessionId) {
      activeSessionId = sessionId;
      activeTerminalSessionId.set(sessionId);
    }
  });

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
      fontSize: 13,
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
      if (event.payload.session_id === activeSessionId) {
        const data = event.payload.data;
        if (password && !passwordSent && /password:/i.test(data)) {
          passwordSent = true;
          invoke("pty_write", { sessionId: activeSessionId, data: password + "\n" });
        }
        pendingData += data;
        if (animationFrameId === null) {
          animationFrameId = requestAnimationFrame(flushTerminalData);
        }
      }
    });

    if (!sessionId) {
      await openTerminal();
    } else {
      activeSessionId = sessionId;
    }

    // Dynamic resize observer with 30ms debounce & safe bounds checks to prevent UI freezing
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

    // Initial fit
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

    // Live theme update when CSS variables change
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
      if (activeSessionId) {
        invoke("pty_write", { sessionId: activeSessionId, data });
      }

      // Track input line buffer to parse navigation commands
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
    });
    sessionId = id;
    activeSessionId = id;
    activeTerminalSessionId.set(id);
    onReady(id);
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
</style>
