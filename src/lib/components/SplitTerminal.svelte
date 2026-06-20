<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import Terminal from "./Terminal.svelte";
  import { activeTerminalSessionId } from "../stores.svelte";

  let {
    cwd = "",
    onCwdChange = (_cwd: string) => {},
    onSessionCreated = (_sessionId: string) => {},
    initialCommand = "",
  } = $props();

  let prevCwd = $state(cwd);

  let terminalIds = $state(["term-1"]);
  let sessions = $state<Record<string, string>>({});
  let cols = $derived(Math.ceil(Math.sqrt(terminalIds.length)));
  let activeSessionId = $state<string | null>(null);
  let terminalCounter = 1;
  let labels = $state<Record<string, string>>({
    "term-1": "0",
  });

  $effect(() => {
    const unsub = activeTerminalSessionId.subscribe((val) => {
      activeSessionId = val;
    });
    return unsub;
  });

  // CWD tracking per terminal
  let cwds = $state<Record<string, string>>({
    "term-1": "C:\\Users\\Lenovo\\Documents\\dev\\NyxEdit",
  });
  let inputBuffers = $state<Record<string, string>>({});

  // Sync workspace → terminal: when cwd prop changes externally, send cd
  $effect(() => {
    if (cwd && cwd !== prevCwd && terminalIds.length > 0) {
      const primaryId = primaryTerminalId();
      if (primaryId && sessions[primaryId]) {
        const isWin = navigator.userAgent.toLowerCase().includes("win");
        invoke("pty_write", {
          sessionId: sessions[primaryId],
          data: `cd "${cwd}"${isWin ? "\r" : "\n"}`,
        }).catch(() => {});
        cwds[primaryId] = cwd;
        cwds = { ...cwds };
      }
      prevCwd = cwd;
    }
  });

  function primaryTerminalId(): string | null {
    if (terminalIds.length === 0) return null;
    return terminalIds.toSorted((a, b) => {
      const na = parseInt(a.replace("term-", "")) || 0;
      const nb = parseInt(b.replace("term-", "")) || 0;
      return na - nb;
    })[0];
  }

  function resolvePath(cwd: string, target: string): string {
    const trimmed = target.trim().replace(/^["']|["']$/g, "");
    if (trimmed.includes(":\\") || trimmed.startsWith("/")) return trimmed;
    const parts = cwd.split("\\");
    for (const p of trimmed.replace(/\//g, "\\").split("\\")) {
      if (!p || p === ".") continue;
      if (p === "..") {
        if (parts.length > 1) parts.pop();
      } else parts.push(p);
    }
    return parts.join("\\");
  }

  function handleCommand(termId: string, line: string) {
    const trimmed = line.trim();
    if (trimmed.startsWith("cd ") || trimmed.startsWith("cd\t")) {
      const target = trimmed.slice(trimmed.startsWith("cd ") ? 3 : 2).trim();
      if (target) {
        const currentCwd = cwds[termId] || "C:\\";
        const resolved = resolvePath(currentCwd, target);
        cwds[termId] = resolved;
        if (termId === primaryTerminalId()) {
          onCwdChange(resolved);
        }
      }
    }
  }

  function addTerminal() {
    const nextId = `term-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`;
    const nextLabel = String(terminalCounter++);
    labels[nextId] = nextLabel;
    labels = { ...labels };
    terminalIds = [...terminalIds, nextId];
  }
  function removeTerminal(id: string) {
    if (terminalIds.length <= 1) return;
    terminalIds = terminalIds.filter((t) => t !== id);
    
    // Clean up closed terminal states
    delete sessions[id];
    delete cwds[id];
    delete inputBuffers[id];
    delete labels[id];
    sessions = { ...sessions };
    cwds = { ...cwds };
    inputBuffers = { ...inputBuffers };
    labels = { ...labels };
  }
  function handleSessionCreated(id: string, sessionId: string) {
    sessions[id] = sessionId;
    sessions = { ...sessions };
    // Notify parent (used by private terminal to mark session as restricted)
    onSessionCreated(sessionId);

    // Run initial command if it is the primary terminal session
    if (id === primaryTerminalId() && initialCommand) {
      setTimeout(() => {
        const isWin = navigator.userAgent.toLowerCase().includes("win");
        invoke("pty_write", { sessionId, data: initialCommand + (isWin ? "\r" : "\n") }).catch(
          (e) => console.error(e),
        );
      }, 300);
    }
  }
</script>

<div class="tiling">
  <div class="tiling-header">
    <span class="tiling-title">Terminal ({terminalIds.length})</span>
    <button class="tiling-btn" onclick={addTerminal} title="New terminal">
      <svg
        width="14"
        height="14"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        ><line x1="12" y1="5" x2="12" y2="19" /><line
          x1="5"
          y1="12"
          x2="19"
          y2="12"
        /></svg
      >
    </button>
  </div>
  <div class="tiling-grid" style="grid-template-columns: repeat({cols}, 1fr)">
    {#each terminalIds as id, i (id)}
      {@const isActive = sessions[id] === activeSessionId}
      <div class="tiling-cell" class:cell-active={isActive}>
        <div class="tiling-cell-actions">
          <button
            class="tiling-cell-btn"
            onclick={() => addTerminal()}
            title="Split"
          >
            <svg
              width="11"
              height="11"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              ><rect x="3" y="3" width="7" height="7" /><rect
                x="14"
                y="3"
                width="7"
                height="7"
              /><rect x="14" y="14" width="7" height="7" /><rect
                x="3"
                y="14"
                width="7"
                height="7"
              /></svg
            >
          </button>
          {#if terminalIds.length > 1}
            <button
              class="tiling-cell-btn"
              onclick={() => removeTerminal(id)}
              title="Close"
            >
              <svg
                width="11"
                height="11"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                ><line x1="18" y1="6" x2="6" y2="18" /><line
                  x1="6"
                  y1="6"
                  x2="18"
                  y2="18"
                /></svg
              >
            </button>
          {/if}
        </div>
        <div class="tiling-cell-body">
          <Terminal
            sessionId={sessions[id]}
            label={labels[id] || "0"}
            onReady={(sid: string) => handleSessionCreated(id, sid)}
            onCommand={(sid: string, line: string) => handleCommand(id, line)}
          />
        </div>
      </div>
    {/each}
  </div>
</div>

<style>
  .tiling {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--glass-bg, var(--bg-secondary));
    backdrop-filter: blur(var(--glass-blur, 12px));
    -webkit-backdrop-filter: blur(var(--glass-blur, 12px));
  }

  .tiling-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 4px 10px;
    border-bottom: 1px solid var(--border-subtle);
    flex-shrink: 0;
  }
  .tiling-title {
    font-size: var(--fs-11);
    font-weight: 600;
    color: var(--text-muted);
  }
  .tiling-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: none;
    color: var(--text-muted);
    padding: 3px;
    cursor: pointer;
    border-radius: 4px;
  }
  .tiling-btn:hover {
    color: var(--text-primary);
    background: var(--bg-hover);
  }

  .tiling-grid {
    flex: 1;
    display: grid;
    gap: 2px;
    padding: 2px;
    overflow: hidden;
    grid-auto-rows: 1fr;
  }

  .tiling-cell {
    display: flex;
    flex-direction: column;
    border: 1px solid var(--border-subtle);
    border-radius: 6px;
    overflow: hidden;
    min-height: 60px;
    background: transparent;
    transition: border-color 0.15s ease;
    position: relative;
  }
  .tiling-cell.cell-active {
    border-color: var(--accent-blue);
  }
  .tiling-cell-actions {
    display: flex;
    gap: 1px;
    position: absolute;
    top: 2px;
    right: 4px;
    z-index: 10;
    opacity: 0;
    transition: opacity 0.12s ease;
  }
  .tiling-cell:hover .tiling-cell-actions { opacity: 0.6; }
  .tiling-cell-actions:hover { opacity: 1 !important; }
  .tiling-cell-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--glass-bg, var(--bg-secondary));
    border: 1px solid var(--border-subtle);
    color: var(--text-muted);
    padding: 1px 4px;
    cursor: pointer;
    border-radius: 4px;
    backdrop-filter: blur(4px);
  }
  .tiling-cell-btn:hover {
    color: var(--text-primary);
    background: var(--bg-hover);
  }
  .tiling-cell-body {
    flex: 1;
    overflow: hidden;
  }
</style>
