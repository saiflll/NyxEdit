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

  let workspaceDir = $state("");
  let selectedType = $state<"rust" | "node" | "platformio">("node");
  let isChecking = $state(false);
  let status = $state<"idle" | "running" | "success" | "failed">("idle");
  let rawOutput = $state("");
  let issues = $state<DiagnosticIssue[]>([]);
  let autoDetected = $state(false);

  $effect(() => {
    const unsub = currentDir.subscribe(val => {
      workspaceDir = val;
      if (val && !autoDetected) {
        detectProjectType();
      }
    });
    return unsub;
  });

  async function detectProjectType() {
    try {
      const info = await invoke<any>("project_detect", { path: workspaceDir });
      if (info) {
        autoDetected = true;
        // Simple heuristic based on project detection details
        const frameworks = info.frameworks || [];
        const lang = info.languages || [];
        
        if (frameworks.includes("platformio") || frameworks.includes("PlatformIO")) {
          selectedType = "platformio";
        } else if (lang.includes("Rust") || lang.includes("rust")) {
          selectedType = "rust";
        } else {
          selectedType = "node";
        }
      }
    } catch (e) {
      console.warn("Failed to auto-detect framework:", e);
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
      // Rust Cargo check parser
      // e.g.
      // error[E0308]: mismatched types
      //   --> src/main.rs:10:15
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

          parsed.push({
            file,
            line: lineNum,
            column: colNum,
            severity: isWarning ? "warning" : "error",
            message: currentErrorMsg
          });
          currentErrorMsg = ""; // consume
        }
      }
    } else if (selectedType === "node") {
      // TypeScript compiler check parser
      // e.g. src/lib/components/AIChat.svelte:204:5 - error TS2322: Type ...
      // or src/lib/components/AIChat.svelte(204,5): error TS2322: Type ...
      for (const line of lines) {
        const matchTsc = line.match(/^([^\s:\n]+):(\d+):(\d+)\s+-\s+(error|warning)\s+TS\d+:\s*(.+)/);
        const matchTscAlt = line.match(/^([^\s:\n]+)\((\d+),(\d+)\):\s*(error|warning)\s+TS\d+:\s*(.+)/);
        
        if (matchTsc) {
          parsed.push({
            file: matchTsc[1].trim(),
            line: parseInt(matchTsc[2]),
            column: parseInt(matchTsc[3]),
            severity: matchTsc[4].toLowerCase() as any,
            message: matchTsc[5].trim()
          });
        } else if (matchTscAlt) {
          parsed.push({
            file: matchTscAlt[1].trim(),
            line: parseInt(matchTscAlt[2]),
            column: parseInt(matchTscAlt[3]),
            severity: matchTscAlt[4].toLowerCase() as any,
            message: matchTscAlt[5].trim()
          });
        }
      }
    } else if (selectedType === "platformio") {
      // PlatformIO C++ compiler check parser
      // e.g. src/main.cpp:15:20: error: 'xxx' was not declared in this scope
      for (const line of lines) {
        const matchPio = line.match(/^([a-zA-Z0-9_\-\.\/\\ ]+):(\d+):(\d+):\s*(error|warning|note):\s*(.+)/);
        if (matchPio) {
          parsed.push({
            file: matchPio[1].trim(),
            line: parseInt(matchPio[2]),
            column: parseInt(matchPio[3]),
            severity: matchPio[4] === "note" ? "info" : (matchPio[4].toLowerCase() as any),
            message: matchPio[5].trim()
          });
        }
      }
    }

    // Filter duplicates and invalid outputs
    issues = parsed.filter(item => !item.file.includes("node_modules") && !item.file.includes("target/"));
  }

  function handleOpenFile(file: string, line: number) {
    // resolve absolute path
    const absPath = workspaceDir + "/" + file;
    openFile(absPath);
    // Focus active file (handled inside stores by setting activeFile)
    addToast(`Opened ${file.split("/").pop()} at line ${line}`, "info");
  }
</script>

<div class="diagnostics-panel" style="padding: 16px; display: flex; flex-direction: column; gap: 16px; flex: 1; overflow-y: auto;">
  <div class="settings-header" style="padding: 0 0 10px 0; border-bottom: 1px solid var(--border-subtle); display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 10px;">
    <span class="settings-title">LSP & Compiler Diagnostics</span>
    <div style="display: flex; gap: 6px; align-items: center;">
      <select 
        bind:value={selectedType} 
        style="background: var(--bg-primary); color: var(--text-primary); border: 1px solid var(--border-subtle); border-radius: 6px; padding: 4px 10px; font-size: var(--fs-10); outline: none; cursor: pointer;"
      >
        <option value="node">Node.js (NPM check)</option>
        <option value="rust">Rust (Cargo check)</option>
        <option value="platformio">PlatformIO (PIO check)</option>
      </select>
      <button 
        class="settings-btn settings-btn-add" 
        onclick={runCheck} 
        disabled={isChecking || !workspaceDir}
        style="background: var(--accent-blue); color: var(--bg-primary); padding: 5px 12px; font-weight: 600; border-radius: 6px; font-size: var(--fs-10); cursor: pointer; border: none; display: flex; align-items: center; gap: 4px; transition: all 0.12s ease;"
      >
        {#if isChecking}
          <span class="spinner-tiny" style="border-color: var(--bg-primary); border-top-color: transparent;"></span>
          Checking...
        {:else}
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/></svg>
          Run Diagnostics
        {/if}
      </button>
    </div>
  </div>

  {#if !workspaceDir}
    <div style="flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; padding: 40px; text-align: center; color: var(--text-muted);">
      <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" style="margin-bottom: 12px;"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
      <p style="margin: 0; font-size: var(--fs-11); font-weight: 500;">No workspace directory opened.</p>
    </div>
  {:else}
    <!-- Status Card -->
    <div class="diagnostics-status-card" style="background: var(--bg-surface); border: 1px solid var(--border-subtle); border-radius: 8px; padding: 12px; display: flex; align-items: center; justify-content: space-between; gap: 12px; flex-wrap: wrap;">
      <div style="display: flex; align-items: center; gap: 8px;">
        <span style="font-size: var(--fs-10); color: var(--text-muted); font-weight: 600; text-transform: uppercase;">Status:</span>
        {#if status === "idle"}
          <span style="font-size: var(--fs-9); font-weight: 700; padding: 2px 6px; border-radius: 4px; background: var(--bg-primary); border: 1px solid var(--border-subtle); color: var(--text-muted);">NOT RUN</span>
        {:else if status === "running"}
          <span style="font-size: var(--fs-9); font-weight: 700; padding: 2px 6px; border-radius: 4px; background: color-mix(in srgb, var(--accent-blue) 12%, transparent); color: var(--accent-blue); display: flex; align-items: center; gap: 4px;">
            <span class="spinner-tiny" style="width: 8px; height: 8px; border-width: 1.5px; border-color: var(--accent-blue); border-top-color: transparent;"></span>
            RUNNING COMPILER CHECK
          </span>
        {:else if status === "success"}
          <span style="font-size: var(--fs-9); font-weight: 700; padding: 2px 6px; border-radius: 4px; background: color-mix(in srgb, var(--accent-green) 12%, transparent); color: var(--accent-green);">BUILD SUCCESS (0 ERRORS)</span>
        {:else}
          <span style="font-size: var(--fs-9); font-weight: 700; padding: 2px 6px; border-radius: 4px; background: color-mix(in srgb, var(--accent-red) 12%, transparent); color: var(--accent-red);">BUILD ERROR (ISSUES FOUND)</span>
        {/if}
      </div>

      <div style="font-size: var(--fs-10); color: var(--text-muted); font-family: monospace;">
        Directory: {workspaceDir.split(/[\\/]/).pop()}
      </div>
    </div>

    {#if issues.length > 0}
      <!-- Parsed Issues List -->
      <div style="display: flex; flex-direction: column; gap: 8px;">
        <div style="font-size: var(--fs-10); font-weight: 700; color: var(--text-secondary); text-transform: uppercase; margin-bottom: 2px;">Parsed Compiler Diagnostics</div>
        <div class="issues-list" style="display: flex; flex-direction: column; gap: 8px;">
          {#each issues as issue}
            <div class="issue-card" style="background: var(--bg-surface); border: 1px solid var(--border-subtle); border-radius: 8px; padding: 12px; display: flex; flex-direction: column; gap: 6px; position: relative; overflow: hidden; transition: border-color 0.15s ease;">
              <div style="position: absolute; left: 0; top: 0; bottom: 0; width: 4px; background: {issue.severity === 'error' ? 'var(--accent-red)' : issue.severity === 'warning' ? 'var(--accent-yellow)' : 'var(--accent-blue)'}"></div>
              
              <div style="display: flex; align-items: center; justify-content: space-between; gap: 8px; flex-wrap: wrap;">
                <div style="display: flex; align-items: center; gap: 6px;">
                  <span style="font-size: var(--fs-8-5); font-weight: 700; padding: 1px 5px; border-radius: 3px; background: {issue.severity === 'error' ? 'color-mix(in srgb, var(--accent-red) 12%, transparent)' : 'color-mix(in srgb, var(--accent-yellow) 12%, transparent)'}; color: {issue.severity === 'error' ? 'var(--accent-red)' : 'var(--accent-yellow)'}; text-transform: uppercase;">{issue.severity}</span>
                  <button 
                    onclick={() => handleOpenFile(issue.file, issue.line)}
                    style="background: none; border: none; font-size: var(--fs-9-5); font-family: monospace; font-weight: 700; color: var(--accent-green); cursor: pointer; padding: 0; text-decoration: underline; text-align: left;"
                  >
                    {issue.file}:{issue.line}:{issue.column}
                  </button>
                </div>
              </div>

              <div style="font-size: var(--fs-10-5); color: var(--text-primary); font-family: var(--font-mono, monospace); white-space: pre-wrap; line-height: 1.4; background: rgba(0, 0, 0, 0.15); padding: 6px 8px; border-radius: 4px; border: 1px solid rgba(255, 255, 255, 0.03);">
                {issue.message}
              </div>
            </div>
          {/each}
        </div>
      </div>
    {/if}

    <!-- Raw Terminal output collapsible -->
    <div style="display: flex; flex-direction: column; gap: 8px;">
      <div style="font-size: var(--fs-10); font-weight: 700; color: var(--text-secondary); text-transform: uppercase; margin-bottom: 2px;">Compiler Raw Output Log</div>
      <pre style="background: rgba(0, 0, 0, 0.25); border: 1px solid var(--border-subtle); border-radius: 8px; padding: 12px; font-family: var(--font-mono, monospace); font-size: var(--fs-10); color: var(--text-secondary); overflow-x: auto; max-height: 300px; white-space: pre-wrap; line-height: 1.4; margin: 0;">{rawOutput || "No diagnostics log output yet. Run a check above."}</pre>
    </div>
  {/if}
</div>

<style>
  .issue-card:hover {
    border-color: var(--border-primary) !important;
  }
</style>
