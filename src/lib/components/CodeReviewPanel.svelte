<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { activeFile, fileContent, addToast } from "../stores.svelte";
  import { get } from "svelte/store";

  type ReviewFinding = {
    file: string;
    line: number;
    severity: "Error" | "Warning" | "Info";
    rule_id: string;
    message: string;
    suggestion: string;
  };

  let currentFilePath = $state<string | null>(null);
  let currentFileCode = $state("");

  $effect(() => {
    const unsubFile = activeFile.subscribe(val => {
      currentFilePath = val;
    });
    const unsubContent = fileContent.subscribe(val => {
      currentFileCode = val;
    });
    return () => {
      unsubFile();
      unsubContent();
    };
  });

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
      console.error(e);
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
      reviewSummary = {
        total: res.total,
        errors: res.errors,
        warnings: res.warnings
      };
      addToast(`Code block review finished! Found ${res.total} issues.`, "success");
    } catch (e) {
      console.error(e);
      addToast("Failed to review text: " + String(e), "error");
    } finally {
      isReviewing = false;
    }
  }
</script>

<div class="review-panel" style="padding: 16px; display: flex; flex-direction: column; gap: 16px; flex: 1; overflow-y: auto;">
  <div class="settings-header" style="padding: 0 0 10px 0; border-bottom: 1px solid var(--border-subtle); display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 10px;">
    <span class="settings-title">Static Code Review</span>
    <div class="mode-toggles" style="display: flex; background: var(--bg-primary); padding: 2px; border-radius: 6px; border: 1px solid var(--border-subtle);">
      <button class="toggle-btn" class:active={reviewMode === "file"} onclick={() => { reviewMode = "file"; findings = []; }} style="border: none; background: transparent; padding: 4px 10px; font-size: var(--fs-10); font-weight: 600; cursor: pointer; border-radius: 4px; color: reviewMode === 'file' ? 'var(--accent-blue)' : 'var(--text-muted)'; transition: all 0.1s ease;">
        Active File
      </button>
      <button class="toggle-btn" class:active={reviewMode === "text"} onclick={() => { reviewMode = "text"; findings = []; }} style="border: none; background: transparent; padding: 4px 10px; font-size: var(--fs-10); font-weight: 600; cursor: pointer; border-radius: 4px; color: reviewMode === 'text' ? 'var(--accent-blue)' : 'var(--text-muted)'; transition: all 0.1s ease;">
        Custom Code Block
      </button>
    </div>
  </div>

  {#if reviewMode === "file"}
    <div class="file-review-box" style="background: var(--bg-surface); border: 1px solid var(--border-subtle); border-radius: 8px; padding: 12px; display: flex; align-items: center; justify-content: space-between; gap: 12px; flex-wrap: wrap;">
      <div style="display: flex; align-items: center; gap: 8px; min-width: 0;">
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="var(--text-muted)" stroke-width="2" style="flex-shrink: 0;"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/></svg>
        <div style="min-width: 0;">
          <span style="font-size: var(--fs-10); color: var(--text-muted); display: block; font-weight: 600; text-transform: uppercase;">Active File</span>
          <span style="font-size: var(--fs-11); color: var(--text-primary); font-family: monospace; display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">
            {currentFilePath ? currentFilePath.split(/[\\/]/).pop() : "No file opened"}
          </span>
        </div>
      </div>
      <button class="settings-btn settings-btn-add" onclick={runFileReview} disabled={isReviewing || !currentFilePath} style="background: var(--accent-blue); color: var(--bg-primary); padding: 6px 14px; font-weight: 600; border-radius: 6px; font-size: var(--fs-11); cursor: pointer; border: none; transition: all 0.12s ease;">
        {#if isReviewing}
          <span class="spinner-tiny" style="border-color: var(--bg-primary); border-top-color: transparent;"></span>
          Reviewing...
        {:else}
          Run Review
        {/if}
      </button>
    </div>
  {:else}
    <div class="text-review-box" style="display: flex; flex-direction: column; gap: 8px;">
      <textarea bind:value={textToReview} rows={6} placeholder="Paste markdown file with code block, or general code text to review here..." style="background: var(--bg-surface); color: var(--text-primary); border: 1px solid var(--border-subtle); border-radius: 8px; padding: 10px; font-size: var(--fs-11); font-family: monospace; resize: vertical; outline: none;"></textarea>
      <button class="settings-btn settings-btn-add" onclick={runTextReview} disabled={isReviewing || !textToReview.trim()} style="align-self: flex-end; background: var(--accent-blue); color: var(--bg-primary); padding: 6px 14px; font-weight: 600; border-radius: 6px; font-size: var(--fs-11); cursor: pointer; border: none; transition: all 0.12s ease;">
        {#if isReviewing}
          <span class="spinner-tiny" style="border-color: var(--bg-primary); border-top-color: transparent;"></span>
          Reviewing...
        {:else}
          Review Code Block
        {/if}
      </button>
    </div>
  {/if}

  {#if findings.length > 0}
    <!-- Summary stats bar -->
    <div class="review-summary-bar" style="display: flex; gap: 12px; background: var(--bg-primary); padding: 12px; border-radius: 8px; border: 1px solid var(--border-subtle);">
      <div style="flex: 1; text-align: center;">
        <span style="font-size: var(--fs-18); font-weight: 700; color: var(--accent-blue); display: block;">{reviewSummary.total}</span>
        <span style="font-size: var(--fs-9); color: var(--text-muted); font-weight: 600; text-transform: uppercase;">Total Issues</span>
      </div>
      <div style="width: 1px; background: var(--border-subtle);"></div>
      <div style="flex: 1; text-align: center;">
        <span style="font-size: var(--fs-18); font-weight: 700; color: var(--accent-red); display: block;">{reviewSummary.errors}</span>
        <span style="font-size: var(--fs-9); color: var(--text-muted); font-weight: 600; text-transform: uppercase;">Errors</span>
      </div>
      <div style="width: 1px; background: var(--border-subtle);"></div>
      <div style="flex: 1; text-align: center;">
        <span style="font-size: var(--fs-18); font-weight: 700; color: var(--accent-yellow); display: block;">{reviewSummary.warnings}</span>
        <span style="font-size: var(--fs-9); color: var(--text-muted); font-weight: 600; text-transform: uppercase;">Warnings</span>
      </div>
    </div>

    <!-- Findings list -->
    <div class="findings-list" style="display: flex; flex-direction: column; gap: 8px;">
      {#each findings as finding}
        <div class="finding-card" style="background: var(--bg-surface); border: 1px solid var(--border-subtle); border-radius: 8px; padding: 12px; display: flex; flex-direction: column; gap: 6px; position: relative; overflow: hidden; transition: border-color 0.15s ease;">
          <div style="position: absolute; left: 0; top: 0; bottom: 0; width: 4px; background: {finding.severity === 'Error' || (finding.severity as any) === 'Error' ? 'var(--accent-red)' : finding.severity === 'Warning' || (finding.severity as any) === 'Warning' ? 'var(--accent-yellow)' : 'var(--accent-blue)'}"></div>
          
          <div style="display: flex; align-items: center; justify-content: space-between; gap: 8px; flex-wrap: wrap;">
            <div style="display: flex; align-items: center; gap: 6px;">
              {#if finding.severity === "Error" || (finding.severity as any) === "Error"}
                <span style="font-size: var(--fs-9); font-weight: 600; padding: 2px 6px; border-radius: 4px; background: color-mix(in srgb, var(--accent-red) 12%, transparent); color: var(--accent-red); text-transform: uppercase;">Error</span>
              {:else if finding.severity === "Warning" || (finding.severity as any) === "Warning"}
                <span style="font-size: var(--fs-9); font-weight: 600; padding: 2px 6px; border-radius: 4px; background: color-mix(in srgb, var(--accent-yellow) 12%, transparent); color: var(--accent-yellow); text-transform: uppercase;">Warning</span>
              {:else}
                <span style="font-size: var(--fs-9); font-weight: 600; padding: 2px 6px; border-radius: 4px; background: color-mix(in srgb, var(--accent-blue) 12%, transparent); color: var(--accent-blue); text-transform: uppercase;">Info</span>
              {/if}
              <span style="font-size: var(--fs-9); font-weight: 600; color: var(--text-muted); font-family: monospace;">Rule: {finding.rule_id}</span>
            </div>
            <span style="font-size: var(--fs-10); color: var(--text-muted); font-family: monospace;">
              Line {finding.line}
            </span>
          </div>

          <div style="font-size: var(--fs-11); color: var(--text-primary); font-weight: 500; margin-top: 2px;">
            {finding.message}
          </div>

          {#if finding.suggestion}
            <div style="font-size: var(--fs-10); color: var(--text-secondary); background: var(--bg-primary); padding: 6px 8px; border-radius: 4px; border: 1px solid var(--border-subtle); margin-top: 4px; line-height: 1.3;">
              <span style="color: var(--accent-green); font-weight: 600;">Suggestion:</span> {finding.suggestion}
            </div>
          {/if}
        </div>
      {/each}
    </div>
  {:else if !isReviewing}
    <!-- Empty state -->
    <div style="flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; padding: 40px 20px; text-align: center; color: var(--text-muted);">
      <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" style="margin-bottom: 12px; color: var(--text-muted);"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
      <p style="margin: 0; font-size: var(--fs-11); font-weight: 500;">No static review analysis results to display.</p>
      {#if reviewMode === "file" && currentFilePath}
        <p style="margin: 4px 0 0 0; font-size: var(--fs-10); color: var(--text-muted);">Click "Run Review" above to analyze the active file.</p>
      {:else if reviewMode === "file"}
        <p style="margin: 4px 0 0 0; font-size: var(--fs-10); color: var(--text-muted);">Open a source file to run static analysis review.</p>
      {/if}
    </div>
  {/if}
</div>

<style>
  .toggle-btn.active {
    background: var(--bg-surface) !important;
    color: var(--accent-blue) !important;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.12);
  }
  .toggle-btn:hover:not(.active) {
    color: var(--text-primary) !important;
  }
  .finding-card:hover {
    border-color: var(--border-primary) !important;
  }
</style>
