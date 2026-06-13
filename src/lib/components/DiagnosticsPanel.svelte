<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { currentDir, addToast, openFile } from "../stores.svelte";

  type DiagnosticIssue = {
    file: string;
    line: number;
    column: number;
    severity: "error" | "warning" | "info";
    message: string;
  };

  let workspaceDir = $derived($currentDir);
  let selectedType = $state<"rust" | "node" | "platformio">("node");
  let isChecking = $state(false);
  let status = $state<"idle" | "running" | "success" | "failed">("idle");
  let rawOutput = $state("");
  let issues = $state<DiagnosticIssue[]>([]);
  let autoDetected = $state(false);

  $effect(() => {
    if (workspaceDir && !autoDetected) {
      detectProjectType();
    }
  });

  async function detectProjectType() {
    try {
      const info = await invoke<any>("project_detect", { root: workspaceDir });
      if (info) {
        autoDetected = true;
        const fw: string = info.framework || "";
        const lang: string = info.language || "";
        if (fw === "PlatformIO") {
          selectedType = "platformio";
        } else if (lang.toLowerCase().includes("rust")) {
          selectedType = "rust";
        } else {
          selectedType = "node";
        }
      }
    } catch (e) {
      console.warn("Auto-detect failed:", e);
    }
  }

  async function runCheck() {
    if (!workspaceDir) {
      addToast("No workspace folder opened", "error");
      return;
    }
    isChecking = true;
    status = "running";
    rawOutput = "";
    issues = [];

    try {
      const output = await invoke<string>("sys_run_diagnostics", {
        cmdType: selectedType,
        directory: workspaceDir
      });

      rawOutput = output;
      parseIssues(output);

      const hasErrors = issues.some(i => i.severity === "error");
      status = hasErrors ? "failed" : "success";

      if (hasErrors) {
        addToast(`Diagnostics completed. Found ${issues.length} compiler issues.`, "error");
      } else {
        addToast("Diagnostics check passed with 0 compile errors!", "success");
      }
    } catch (e) {
      console.error(e);
      status = "failed";
      rawOutput = String(e);
      addToast("Diagnostics command failed to run: " + String(e), "error");
    } finally {
      isChecking = false;
    }
  }

  function parseIssues(output: string) {
    const lines = output.split("\n");
    const parsed: DiagnosticIssue[] = [];

    if (selectedType === "rust") {
      let currentErrorMsg = "";
      for (let i = 0; i < lines.length; i++) {
        const line = lines[i];
        if (line.startsWith("error[") || line.startsWith("error:") || line.startsWith("warning:")) {
          currentErrorMsg = line.trim();
        }
        const match = line.match(/^\s*-->\s*([^:\n]+):(\d+):(\d+)/);
        if (match && currentErrorMsg) {
          const file = match[1].trim();
          const lineNum = parseInt(match[2]);
          const colNum = parseInt(match[3]);
          const isWarning = currentErrorMsg.startsWith("warning");
          parsed.push({ file, line: lineNum, column: colNum, severity: isWarning ? "warning" : "error", message: currentErrorMsg });
          currentErrorMsg = "";
        }
      }
    } else if (selectedType === "node") {
      for (const line of lines) {
        const m = line.match(/^([^\s:\n]+):(\d+):(\d+)\s+-\s+(error|warning)\s+TS\d+:\s*(.+)/);
        const m2 = line.match(/^([^\s:\n]+)\((\d+),(\d+)\):\s*(error|warning)\s+TS\d+:\s*(.+)/);
        if (m) parsed.push({ file: m[1].trim(), line: parseInt(m[2]), column: parseInt(m[3]), severity: m[4].toLowerCase() as any, message: m[5].trim() });
        else if (m2) parsed.push({ file: m2[1].trim(), line: parseInt(m2[2]), column: parseInt(m2[3]), severity: m2[4].toLowerCase() as any, message: m2[5].trim() });
      }
    } else if (selectedType === "platformio") {
      for (const line of lines) {
        const m = line.match(/^([a-zA-Z0-9_\-\.\/\\ ]+):(\d+):(\d+):\s*(error|warning|note):\s*(.+)/);
        if (m) parsed.push({ file: m[1].trim(), line: parseInt(m[2]), column: parseInt(m[3]), severity: m[4] === "note" ? "info" : (m[4].toLowerCase() as any), message: m[5].trim() });
      }
    }

    issues = parsed.filter(item => !item.file.includes("node_modules") && !item.file.includes("target/"));
  }

  function handleOpenFile(file: string, line: number) {
    const absPath = workspaceDir + "/" + file;
    openFile(absPath);
    addToast(`Opened ${file.split("/").pop()} at line ${line}`, "info");
  }

  function sevColor(s: string): string {
    if (s === "error") return "var(--accent-red)";
    if (s === "warning") return "var(--accent-yellow)";
    return "var(--accent-blue)";
  }
</script>

<div class="diag">
  <div class="diag-h">
    <span class="diag-title">Compiler Diagnostics</span>
    <div class="diag-tools">
      <select bind:value={selectedType}>
        <option value="node">Node.js</option>
        <option value="rust">Rust</option>
        <option value="platformio">PlatformIO</option>
      </select>
      <button class="db db-p" onclick={runCheck} disabled={isChecking || !workspaceDir}>
        {isChecking ? 'Checking...' : 'Run'}
      </button>
    </div>
  </div>

  {#if !workspaceDir}
    <div class="diag-empty">
      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
      <p>No workspace opened</p>
    </div>
  {:else}
    <div class="diag-status">
      <span class="diag-sl">Status:</span>
      {#if status === "idle"}
        <span class="diag-badge">Not Run</span>
      {:else if status === "running"}
        <span class="diag-badge diag-bd-ok" style="background:color-mix(in srgb,var(--accent-blue)12%,transparent);color:var(--accent-blue)">Running...</span>
      {:else if status === "success"}
        <span class="diag-badge" style="background:color-mix(in srgb,var(--accent-green)12%,transparent);color:var(--accent-green);border-color:var(--accent-green)">0 Errors</span>
      {:else}
        <span class="diag-badge" style="background:color-mix(in srgb,var(--accent-red)12%,transparent);color:var(--accent-red);border-color:var(--accent-red)">{issues.filter(i => i.severity === "error").length} Errors</span>
      {/if}
      <span class="diag-dir">{workspaceDir.split(/[\\/]/).pop()}</span>
    </div>

    {#if isChecking}
      <div class="diag-progress">
        <span>Running {selectedType} diagnostics...</span>
        <div class="diag-pr-bar"><div class="diag-pr-fill" style="width:60%"></div></div>
      </div>
    {/if}

    {#if issues.length > 0}
      <div class="diag-list">
        {#each issues as issue}
          <div class="diag-card" style="border-left-color:{sevColor(issue.severity)}">
            <div class="diag-card-h">
              <span class="diag-sev" style="background:color-mix(in srgb,{sevColor(issue.severity)}12%,transparent);color:{sevColor(issue.severity)}">{issue.severity}</span>
              <button class="diag-fl" onclick={() => handleOpenFile(issue.file, issue.line)}>{issue.file}:{issue.line}:{issue.column}</button>
            </div>
            <div class="diag-msg">{issue.message}</div>
          </div>
        {/each}
      </div>
    {/if}

    {#if rawOutput}
      <div class="diag-raw">
        <span class="diag-raw-l">Raw Output</span>
        <pre>{rawOutput}</pre>
      </div>
    {/if}
  {/if}
</div>

<style>
  .diag { display:flex;flex-direction:column;gap:5px; }
  .diag-h { display:flex;align-items:center;justify-content:space-between;gap:6px; }
  .diag-title { font-size:var(--fs-9);font-weight:700;color:var(--text-primary); }
  .diag-tools { display:flex;gap:4px;align-items:center; }
  .diag-tools select { background:var(--bg-primary);color:var(--text-primary);border:1px solid var(--border-subtle);border-radius:3px;padding:2px 6px;font-size:var(--fs-8);outline:none;cursor:pointer; }
  .db { display:inline-flex;align-items:center;gap:3px;padding:2px 8px;border-radius:3px;border:1px solid var(--border-subtle);background:var(--bg-primary);color:var(--text-primary);font-size:var(--fs-8);cursor:pointer;font-weight:600;white-space:nowrap; }
  .db-p { background:var(--accent-blue);color:var(--bg-primary);border-color:var(--accent-blue); }
  .db:disabled { opacity:.4;cursor:default; }

  .diag-empty { display:flex;flex-direction:column;align-items:center;gap:4px;padding:16px;text-align:center;color:var(--text-muted); }
  .diag-empty p { margin:0;font-size:var(--fs-9);font-weight:500; }

  .diag-status { display:flex;align-items:center;gap:6px;padding:4px 7px;border-radius:3px;background:var(--bg-primary);border:1px solid var(--border-subtle);flex-wrap:wrap; }
  .diag-sl { font-size:var(--fs-8);color:var(--text-muted);font-weight:600;text-transform:uppercase; }
  .diag-badge { font-size:var(--fs-8);font-weight:700;padding:1px 6px;border-radius:2px;background:var(--bg-surface);border:1px solid var(--border-subtle);color:var(--text-muted); }
  .diag-dir { font-size:var(--fs-8);color:var(--text-muted);font-family:monospace;margin-left:auto; }

  .diag-progress { display:flex;flex-direction:column;gap:3px;padding:4px 6px;border-radius:3px;background:var(--bg-primary);border:1px solid var(--border-subtle); }
  .diag-progress span { font-size:var(--fs-8);color:var(--text-muted); }
  .diag-pr-bar { height:3px;background:var(--bg-surface);border-radius:2px;overflow:hidden; }
  .diag-pr-fill { height:100%;background:var(--accent-blue);border-radius:2px;animation:pulse 1.5s infinite; }
  @keyframes pulse { 0%,100%{opacity:1} 50%{opacity:.5} }

  .diag-list { display:flex;flex-direction:column;gap:3px;max-height:300px;overflow-y:auto; }
  .diag-card { padding:4px 6px;border-radius:3px;background:var(--bg-primary);border:1px solid var(--border-subtle);border-left:3px solid var(--accent-red);display:flex;flex-direction:column;gap:2px; }
  .diag-card-h { display:flex;align-items:center;gap:4px; }
  .diag-sev { font-size:var(--fs-7);font-weight:700;padding:0 4px;border-radius:2px;text-transform:uppercase; }
  .diag-fl { background:none;border:none;font-size:var(--fs-8);font-family:monospace;font-weight:600;color:var(--accent-green);cursor:pointer;padding:0;text-decoration:underline;overflow:hidden;text-overflow:ellipsis;white-space:nowrap; }
  .diag-msg { font-size:var(--fs-9);color:var(--text-primary);font-family:monospace;white-space:pre-wrap;line-height:1.3;background:var(--bg-surface);padding:3px 5px;border-radius:2px; }

  .diag-raw { display:flex;flex-direction:column;gap:3px; }
  .diag-raw-l { font-size:var(--fs-8);font-weight:700;color:var(--text-muted);text-transform:uppercase; }
  .diag-raw pre { margin:0;background:var(--bg-surface);border:1px solid var(--border-subtle);border-radius:3px;padding:6px 8px;font-family:monospace;font-size:var(--fs-8);color:var(--text-secondary);overflow-x:auto;max-height:200px;white-space:pre-wrap;line-height:1.3; }
</style>
