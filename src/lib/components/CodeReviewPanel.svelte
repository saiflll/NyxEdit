<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { activeFile, fileContent, addToast } from "../stores.svelte";

  type ReviewFinding = {
    file: string;
    line: number;
    severity: "Error" | "Warning" | "Info";
    rule_id: string;
    message: string;
    suggestion: string;
  };

  let currentFilePath = $derived($activeFile);
  let currentFileCode = $derived($fileContent);

  let findings = $state<ReviewFinding[]>([]);
  let isReviewing = $state(false);
  let reviewSummary = $state({ total: 0, errors: 0, warnings: 0 });
  let textToReview = $state("");
  let reviewMode = $state<"file" | "text">("file");

  async function runFileReview() {
    if (!currentFilePath) {
      addToast("No active file to review", "error");
      return;
    }
    isReviewing = true;
    findings = [];
    try {
      const res = await invoke<ReviewFinding[]>("review_file", {
        filePath: currentFilePath,
        content: currentFileCode
      });
      findings = res;
      const errors = res.filter(f => f.severity === "Error" || (f.severity as any) === "Error").length;
      const warnings = res.length - errors;
      reviewSummary = { total: res.length, errors, warnings };
      addToast(`Reviewed ${currentFilePath.split(/[\\/]/).pop()}! Found ${res.length} issues.`, "success");
    } catch (e) {
      addToast("Failed to review file: " + String(e), "error");
    } finally {
      isReviewing = false;
    }
  }

  async function runTextReview() {
    if (!textToReview.trim()) {
      addToast("Please enter code text to review", "error");
      return;
    }
    isReviewing = true;
    findings = [];
    try {
      const res = await invoke<{
        findings: ReviewFinding[];
        total: number;
        errors: number;
        warnings: number;
      }>("review_text", { text: textToReview });
      findings = res.findings;
      reviewSummary = { total: res.total, errors: res.errors, warnings: res.warnings };
      addToast(`Code block review finished! Found ${res.total} issues.`, "success");
    } catch (e) {
      addToast("Failed to review text: " + String(e), "error");
    } finally {
      isReviewing = false;
    }
  }

  function sevColor(s: string): string {
    if (s === "Error") return "var(--accent-red)";
    if (s === "Warning") return "var(--accent-yellow)";
    return "var(--accent-blue)";
  }
</script>

<div class="cr">
  <div class="cr-h">
    <span class="cr-title">Code Review</span>
    <div class="cr-mode">
      <button class="cr-mb" class:cr-mb-act={reviewMode === "file"} onclick={() => { reviewMode = "file"; findings = []; }}>File</button>
      <button class="cr-mb" class:cr-mb-act={reviewMode === "text"} onclick={() => { reviewMode = "text"; findings = []; }}>Text</button>
    </div>
  </div>

  {#if reviewMode === "file"}
    <div class="cr-file">
      <div class="cr-file-info">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/></svg>
        <div><span class="cr-label">Active File</span><span class="cr-fname">{currentFilePath ? currentFilePath.split(/[\\/]/).pop() : "No file opened"}</span></div>
      </div>
      <button class="cr-btn cr-btn-p" onclick={runFileReview} disabled={isReviewing || !currentFilePath}>
        {isReviewing ? 'Reviewing...' : 'Run Review'}
      </button>
    </div>
  {:else}
    <div class="cr-text">
      <textarea bind:value={textToReview} rows={4} placeholder="Paste code to review..."></textarea>
      <button class="cr-btn cr-btn-p" style="align-self:flex-end" onclick={runTextReview} disabled={isReviewing || !textToReview.trim()}>
        {isReviewing ? 'Reviewing...' : 'Review Code'}
      </button>
    </div>
  {/if}

  {#if findings.length > 0}
    <div class="cr-summary">
      <span class="cr-s-item"><span class="cr-s-n">{reviewSummary.total}</span> total</span>
      <span class="cr-s-item"><span class="cr-s-n" style="color:var(--accent-red)">{reviewSummary.errors}</span> errors</span>
      <span class="cr-s-item"><span class="cr-s-n" style="color:var(--accent-yellow)">{reviewSummary.warnings}</span> warnings</span>
    </div>

    <div class="cr-list">
      {#each findings as finding}
        <div class="cr-card" style="border-left-color:{sevColor(finding.severity)}">
          <div class="cr-card-h">
            <span class="cr-sev" style="background:color-mix(in srgb,{sevColor(finding.severity)}12%,transparent);color:{sevColor(finding.severity)}">{finding.severity}</span>
            <span class="cr-rule">{finding.rule_id}</span>
            <span class="cr-line">L{finding.line}</span>
          </div>
          <div class="cr-msg">{finding.message}</div>
          {#if finding.suggestion}
            <div class="cr-sug"><span class="cr-sug-l">Suggestion:</span> {finding.suggestion}</div>
          {/if}
        </div>
      {/each}
    </div>
  {:else if !isReviewing}
    <div class="cr-empty">
      <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
      <p>No review results yet.</p>
      {#if reviewMode === "file"}
        {#if currentFilePath}<p class="cr-empty-h">Click "Run Review" to analyze.</p>{:else}<p class="cr-empty-h">Open a file first.</p>{/if}
      {/if}
    </div>
  {/if}
</div>

<style>
  .cr { display:flex;flex-direction:column;gap:5px; }
  .cr-h { display:flex;align-items:center;justify-content:space-between;gap:6px; }
  .cr-title { font-size:var(--fs-9);font-weight:700;color:var(--text-primary); }
  .cr-mode { display:flex;background:var(--bg-primary);padding:1px;border-radius:4px;border:1px solid var(--border-subtle); }
  .cr-mb { border:none;background:transparent;padding:2px 8px;font-size:var(--fs-8);font-weight:600;cursor:pointer;border-radius:3px;color:var(--text-muted);transition:all .1s; }
  .cr-mb-act { background:var(--bg-surface) !important;color:var(--accent-blue) !important;box-shadow:0 1px 2px rgba(0,0,0,.1); }
  .cr-mb:hover:not(.cr-mb-act) { color:var(--text-primary); }

  .cr-file { display:flex;align-items:center;justify-content:space-between;gap:6px;padding:5px 7px;border-radius:4px;background:var(--bg-primary);border:1px solid var(--border-subtle); }
  .cr-file-info { display:flex;align-items:center;gap:6px;min-width:0;flex:1; }
  .cr-file-info svg { flex-shrink:0;color:var(--text-muted); }
  .cr-label { font-size:var(--fs-8);color:var(--text-muted);font-weight:600;text-transform:uppercase;display:block; }
  .cr-fname { font-size:var(--fs-9);color:var(--text-primary);font-family:monospace;display:block;overflow:hidden;text-overflow:ellipsis;white-space:nowrap; }

  .cr-text { display:flex;flex-direction:column;gap:5px; }
  .cr-text textarea { background:var(--bg-surface);color:var(--text-primary);border:1px solid var(--border-subtle);border-radius:4px;padding:6px 8px;font-size:var(--fs-9);font-family:monospace;resize:vertical;outline:none;min-height:60px; }

  .cr-btn { display:inline-flex;align-items:center;gap:3px;padding:3px 9px;border-radius:3px;border:1px solid var(--border-subtle);background:var(--bg-primary);color:var(--text-primary);font-size:var(--fs-9);cursor:pointer;font-weight:600;white-space:nowrap; }
  .cr-btn-p { background:var(--accent-blue);color:var(--bg-primary);border-color:var(--accent-blue); }
  .cr-btn:disabled { opacity:.4;cursor:default; }

  .cr-summary { display:flex;gap:8px;padding:4px 7px;border-radius:3px;background:var(--bg-primary);border:1px solid var(--border-subtle); }
  .cr-s-item { font-size:var(--fs-8);color:var(--text-muted);font-family:monospace;display:flex;align-items:center;gap:3px; }
  .cr-s-n { font-weight:700;color:var(--accent-blue);font-size:var(--fs-10); }

  .cr-list { display:flex;flex-direction:column;gap:3px; }
  .cr-card { padding:5px 7px;border-radius:3px;background:var(--bg-primary);border:1px solid var(--border-subtle);border-left:3px solid var(--accent-blue);display:flex;flex-direction:column;gap:3px; }
  .cr-card-h { display:flex;align-items:center;gap:5px; }
  .cr-sev { font-size:var(--fs-7);font-weight:700;padding:0 4px;border-radius:2px;text-transform:uppercase; }
  .cr-rule { font-size:var(--fs-8);color:var(--text-muted);font-family:monospace;flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap; }
  .cr-line { font-size:var(--fs-8);color:var(--text-muted);font-family:monospace;flex-shrink:0; }
  .cr-msg { font-size:var(--fs-9);color:var(--text-primary);font-weight:500;line-height:1.3; }
  .cr-sug { font-size:var(--fs-8);color:var(--text-secondary);background:var(--bg-surface);padding:4px 6px;border-radius:3px;border:1px solid var(--border-subtle);line-height:1.3; }
  .cr-sug-l { color:var(--accent-green);font-weight:600; }

  .cr-empty { display:flex;flex-direction:column;align-items:center;gap:4px;padding:20px;text-align:center;color:var(--text-muted); }
  .cr-empty svg { color:var(--text-muted); }
  .cr-empty p { margin:0;font-size:var(--fs-9);font-weight:500; }
  .cr-empty-h { margin:0;font-size:var(--fs-8); }
</style>
