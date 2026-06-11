<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { ask } from "@tauri-apps/plugin-dialog";
  import { currentDir, agents } from "../stores.svelte";

  let workspacePath = $state("");
  let branchName = $state("detached");
  let untracked = $state<string[]>([]);
  let modified = $state<string[]>([]);
  let staged = $state<string[]>([]);
  let commitMessage = $state("");
  let isLoading = $state(false);
  let commitStatus = $state<string | null>(null);
  let gitRefreshTimer: ReturnType<typeof setTimeout> | undefined = undefined;
  let isGeneratingAiCommit = $state(false);
  let remoteUrl = $state("");
  let commitHistory = $state<{ hash: string; author: string; date: string; message: string }[]>([]);

  const MAX_DISPLAY_FILES = 100;

  let displayUntracked = $derived(untracked.slice(0, MAX_DISPLAY_FILES));
  let displayModified = $derived(modified.slice(0, MAX_DISPLAY_FILES));
  let displayStaged = $derived(staged.slice(0, MAX_DISPLAY_FILES));

  let isGitRepo = $derived(branchName !== "no git repo");

  $effect(() => {
    const unsub = currentDir.subscribe((val) => {
      workspacePath = val;
      if (val) {
        if (gitRefreshTimer) clearTimeout(gitRefreshTimer);
        gitRefreshTimer = setTimeout(() => refreshStatus(), 300);
      }
    });
    return () => { if (gitRefreshTimer) clearTimeout(gitRefreshTimer); unsub(); };
  });

  async function refreshStatus() {
    if (!workspacePath) return;
    isLoading = true;
    commitStatus = null;
    try {
      const res = await invoke<{
        branch: string; untracked: string[]; modified: string[]; staged: string[];
      }>("git_get_status", { repoPath: workspacePath });
      branchName = res.branch;
      untracked = res.untracked;
      modified = res.modified;
      staged = res.staged;
      if (res.branch !== "no git repo") {
        invoke<string>("git_remote_url", { repoPath: workspacePath }).then(u => remoteUrl = u).catch(() => remoteUrl = "");
        invoke<{ hash: string; author: string; date: string; message: string }[]>("git_log", { repoPath: workspacePath, maxCount: 20 }).then(h => commitHistory = h).catch(() => commitHistory = []);
      }
    } catch (e) {
      console.error("Failed to check Git status:", e);
      branchName = "no git repo";
      untracked = []; modified = []; staged = [];
    }
    isLoading = false;
  }

  async function handleInit() {
    if (!workspacePath) return;
    isLoading = true;
    try {
      await invoke("git_init", { repoPath: workspacePath });
      commitStatus = "Git repo initialized!";
      await refreshStatus();
    } catch (e: any) {
      commitStatus = `Error: ${e.toString()}`;
    }
    isLoading = false;
  }

  async function handleCommit() {
    if (!workspacePath || !commitMessage.trim()) return;
    isLoading = true;
    commitStatus = "Committing...";
    try {
      await invoke<string>("git_commit", {
        repoPath: workspacePath, message: commitMessage.trim(),
      });
      commitStatus = "Committed successfully!";
      commitMessage = "";
      await refreshStatus();
    } catch (e: any) {
      commitStatus = `Error: ${e.toString()}`;
    }
    isLoading = false;
  }

  async function generateAiCommit() {
    if (!workspacePath) return;
    isGeneratingAiCommit = true;
    commitStatus = "Analyzing changes...";
    try {
      const diff = await invoke<string>("git_diff_uncommitted", { repoPath: workspacePath, staged: false });
      if (!diff.trim()) {
        commitStatus = "No changes detected to describe.";
        isGeneratingAiCommit = false;
        return;
      }
      let agentList: { id: string; name: string; provider: string; model: string }[] = [];
      try { agentList = await invoke("ai_list_agents"); } catch {}
      const agentId = agentList.length > 0 ? agentList[0].id : "coder";

      const msg = `Generate a concise conventional commit message for this diff:\n\n\`\`\`diff\n${diff.slice(0, 4000)}\n\`\`\`\n\nRespond with ONLY the commit message, no explanation.`;
      const resp = await invoke<{ agent_id: string; content: string; provider: string; model: string }>("ai_chat", {
        agentId,
        messages: [{ role: "user", content: msg }],
      });
      commitMessage = resp.content.trim().replace(/^['"`]+|['"`]+$/g, "").split("\n")[0];
      commitStatus = "AI commit message generated!";
    } catch (e: any) {
      commitStatus = `AI gen failed: ${e.toString()}`;
    }
    isGeneratingAiCommit = false;
  }

  async function handlePush() {
    if (!workspacePath) return;
    isLoading = true;
    commitStatus = "Pushing...";
    try {
      const out = await invoke<string>("git_push", { repoPath: workspacePath, remote: null, branch: null });
      commitStatus = out || "Pushed successfully!";
      await refreshStatus();
    } catch (e: any) {
      commitStatus = `Push error: ${e.toString()}`;
    }
    isLoading = false;
  }

  async function handlePull() {
    if (!workspacePath) return;
    isLoading = true;
    commitStatus = "Pulling...";
    try {
      const out = await invoke<string>("git_pull", { repoPath: workspacePath, remote: null, branch: null });
      commitStatus = out || "Pull completed!";
      await refreshStatus();
    } catch (e: any) {
      commitStatus = `Pull error: ${e.toString()}`;
    }
    isLoading = false;
  }

  async function handleSync() {
    if (!workspacePath) return;
    isLoading = true;
    commitStatus = "Syncing...";
    try {
      const out = await invoke<string>("git_sync", { repoPath: workspacePath });
      commitStatus = out || "Sync completed!";
      await refreshStatus();
    } catch (e: any) {
      commitStatus = `Sync error: ${e.toString()}`;
    }
    isLoading = false;
  }

  async function handlePr() {
    if (!workspacePath || !remoteUrl) return;
    let url = remoteUrl.trim();
    if (url.endsWith(".git")) url = url.slice(0, -4);
    if (url.startsWith("git@")) {
      url = url.replace(":", "/").replace("git@", "https://");
    }
    url = url.replace(/^https:\/\/[^@]+@/, "https://");
    if (url.includes("github.com")) {
      url += `/compare/${branchName}?expand=1`;
    } else if (url.includes("gitlab")) {
      url += `/-/merge_requests/new?merge_request[source_branch]=${branchName}`;
    } else {
      url += `/pull/new/${branchName}`;
    }
    window.open(url, "_blank");
  }

  async function openRemote() {
    if (!remoteUrl) return;
    let url = remoteUrl.trim();
    if (url.endsWith(".git")) url = url.slice(0, -4);
    if (url.startsWith("git@")) {
      url = url.replace(":", "/").replace("git@", "https://");
    }
    url = url.replace(/^https:\/\/[^@]+@/, "https://");
    window.open(url, "_blank");
  }

  function formatDate(iso: string): string {
    try { return new Date(iso).toLocaleDateString(undefined, { month: "short", day: "numeric" }); }
    catch { return iso.slice(0, 10); }
  }

  async function stageFile(file: string) {
    if (!workspacePath) return;
    isLoading = true;
    try {
      await invoke("git_stage_file", { repoPath: workspacePath, filePath: file });
      await refreshStatus();
    } catch (e) { console.error(e); }
    isLoading = false;
  }

  async function unstageFile(file: string) {
    if (!workspacePath) return;
    isLoading = true;
    try {
      await invoke("git_unstage_file", { repoPath: workspacePath, filePath: file });
      await refreshStatus();
    } catch (e) { console.error(e); }
    isLoading = false;
  }

  async function discardFile(file: string) {
    if (!workspacePath) return;
    const confirmed = await ask(`Are you sure you want to discard changes for "${file}"?`, {
      title: "Discard Changes", kind: "warning",
    });
    if (!confirmed) return;
    isLoading = true;
    try {
      await invoke("git_discard_file", { repoPath: workspacePath, filePath: file });
      await refreshStatus();
    } catch (e) { console.error(e); }
    isLoading = false;
  }

  async function stageAll() {
    if (!workspacePath) return;
    isLoading = true;
    try {
      await invoke("git_stage_all", { repoPath: workspacePath });
      await refreshStatus();
    } catch (e) { console.error(e); }
    isLoading = false;
  }

  async function unstageAll() {
    if (!workspacePath) return;
    isLoading = true;
    try {
      await invoke("git_unstage_all", { repoPath: workspacePath });
      await refreshStatus();
    } catch (e) { console.error(e); }
    isLoading = false;
  }
</script>

<div class="git-status">
  <div class="git-header">
    <div class="git-branch-info">
      <svg class="branch-icon" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <line x1="6" y1="3" x2="6" y2="15"></line>
        <circle cx="18" cy="6" r="3"></circle>
        <circle cx="6" cy="18" r="3"></circle>
        <path d="M18 9a9 9 0 0 1-9 9"></path>
      </svg>
      <span class="branch-name" title="Active Branch">{branchName}</span>
    </div>
    <div class="git-header-actions">
      {#if isGitRepo}
        <button class="git-header-btn" onclick={handlePull} disabled={isLoading} title="Pull">
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
        </button>
        <button class="git-header-btn" onclick={handlePush} disabled={isLoading} title="Push">
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="17 8 12 3 7 8"/><line x1="12" y1="3" x2="12" y2="15"/></svg>
        </button>
        <button class="git-header-btn" onclick={handleSync} disabled={isLoading} title="Sync (Fetch + Pull + Push)">
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="23 4 23 10 17 10"/><polyline points="1 20 1 14 7 14"/><path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/></svg>
        </button>
        {#if remoteUrl}
          <button class="git-header-btn" onclick={handlePr} disabled={isLoading} title="Create Pull Request">
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="18" cy="18" r="3"/><circle cx="6" cy="6" r="3"/><path d="M13 6h3a2 2 0 0 1 2 2v7"/><line x1="6" y1="16" x2="6" y2="21"/></svg>
          </button>
        {/if}
      {/if}
      <button class="git-header-btn" onclick={refreshStatus} disabled={isLoading} title="Refresh">
        <svg class="refresh-icon" class:spinning={isLoading} width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M23 4v6h-6M1 20v-6h6M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"></path>
        </svg>
      </button>
    </div>
  </div>

  <div class="git-body">
    {#if !isGitRepo}
      <div class="no-repo">
        <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" style="color:var(--text-muted);"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="16"/><line x1="8" y1="12" x2="16" y2="12"/></svg>
        <p>Not a Git repository</p>
        <span>Initialize a repo to track changes</span>
        <button class="init-btn" onclick={handleInit} disabled={isLoading}>Initialize Repository</button>
      </div>
    {:else}
      <div class="commit-section">
        <div style="display:flex; gap:4px; align-items:stretch;">
          <textarea
            bind:value={commitMessage}
            class="commit-textarea"
            placeholder="Commit message (Ctrl+Enter)..."
            rows="2"
            disabled={isLoading || (untracked.length === 0 && modified.length === 0 && staged.length === 0)}
            onkeydown={(e) => {
              if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) { e.preventDefault(); handleCommit(); }
            }}
          ></textarea>
          <button
            class="ai-commit-btn"
            onclick={generateAiCommit}
            disabled={isGeneratingAiCommit || untracked.length === 0 && modified.length === 0 && staged.length === 0}
            title="Generate AI commit message"
          >
            {#if isGeneratingAiCommit}
              <div class="spinner-tiny"></div>
            {:else}
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/></svg>
            {/if}
          </button>
        </div>
        <div style="display:flex; gap:6px;">
          <button
            class="commit-btn"
            onclick={handleCommit}
            disabled={isLoading || !commitMessage.trim() || (untracked.length === 0 && modified.length === 0 && staged.length === 0)}
          >
            Commit to {branchName}
          </button>
          {#if remoteUrl}
            <button class="pr-btn" onclick={handlePr} disabled={isLoading} title="Create Pull Request">
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="18" cy="18" r="3"/><circle cx="6" cy="6" r="3"/><path d="M13 6h3a2 2 0 0 1 2 2v7"/><line x1="6" y1="16" x2="6" y2="21"/></svg>
              New Pull Request
            </button>
          {/if}
        </div>
        {#if commitStatus}
          <div class="commit-message-log" class:error={commitStatus.startsWith('Error') || commitStatus.startsWith('AI gen fail') || commitStatus.startsWith('Push error') || commitStatus.startsWith('Pull error') || commitStatus.startsWith('Sync error')}>
            {commitStatus}
          </div>
        {/if}
      </div>

      <div class="changes-section">
        {#if untracked.length === 0 && modified.length === 0 && staged.length === 0}
          <div class="empty-state">
            <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
              <polyline points="20 6 9 17 4 12"></polyline>
            </svg>
            <p>No changes detected</p>
            <span>Working tree is clean</span>
          </div>
        {:else}
          {#if staged.length > 0}
            <div class="changes-group">
              <div class="group-header">
                <span class="group-title">Staged ({staged.length})</span>
                <button class="bulk-action-btn" onclick={unstageAll} disabled={isLoading}>Unstage All</button>
              </div>
              <div class="changes-list">
                {#each displayStaged as file}
                  <div class="change-item change-staged">
                    <span class="change-badge badge-staged">A</span>
                    <span class="change-path" title={file}>{file.split('/').pop()}</span>
                    <span class="change-fullpath">{file}</span>
                    <div class="change-actions">
                      <button class="action-btn" onclick={() => unstageFile(file)} disabled={isLoading} title="Unstage">
                        <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><line x1="5" y1="12" x2="19" y2="12"/></svg>
                      </button>
                    </div>
                  </div>
                {/each}
              </div>
            </div>
          {/if}
          {#if modified.length > 0}
            <div class="changes-group">
              <div class="group-header">
                <span class="group-title">Modified ({modified.length})</span>
                <button class="bulk-action-btn" onclick={stageAll} disabled={isLoading}>Stage All</button>
              </div>
              <div class="changes-list">
                {#each displayModified as file}
                  <div class="change-item change-modified">
                    <span class="change-badge badge-modified">M</span>
                    <span class="change-path" title={file}>{file.split('/').pop()}</span>
                    <span class="change-fullpath">{file}</span>
                    <div class="change-actions">
                      <button class="action-btn" onclick={() => discardFile(file)} disabled={isLoading} title="Discard">
                        <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><polyline points="3 3 3 8 8 8"/></svg>
                      </button>
                      <button class="action-btn" onclick={() => stageFile(file)} disabled={isLoading} title="Stage">
                        <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
                      </button>
                    </div>
                  </div>
                {/each}
              </div>
            </div>
          {/if}
          {#if untracked.length > 0}
            <div class="changes-group">
              <div class="group-header">
                <span class="group-title">Untracked ({untracked.length})</span>
                <button class="bulk-action-btn" onclick={stageAll} disabled={isLoading}>Stage All</button>
              </div>
              <div class="changes-list">
                {#each displayUntracked as file}
                  <div class="change-item change-untracked">
                    <span class="change-badge badge-untracked">U</span>
                    <span class="change-path" title={file}>{file.split('/').pop()}</span>
                    <span class="change-fullpath">{file}</span>
                    <div class="change-actions">
                      <button class="action-btn" onclick={() => discardFile(file)} disabled={isLoading} title="Delete">
                        <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>
                      </button>
                      <button class="action-btn" onclick={() => stageFile(file)} disabled={isLoading} title="Stage">
                        <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
                      </button>
                    </div>
                  </div>
                {/each}
              </div>
            </div>
          {/if}
        {/if}
      </div>
    {/if}

    {#if isGitRepo && commitHistory.length > 0}
      <div class="history-section">
        <div class="history-header">
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/></svg>
          <span>Recent Commits</span>
        </div>
        <div class="history-list">
          {#each commitHistory as commit}
            <div class="history-item">
              <span class="history-dot"></span>
              <div class="history-content">
                <span class="history-msg">{commit.message}</span>
                <span class="history-meta">
                  <span class="history-hash" title={commit.hash}>{commit.hash.slice(0, 7)}</span>
                  <span>{commit.author}</span>
                  <span>{formatDate(commit.date)}</span>
                </span>
              </div>
            </div>
          {/each}
        </div>
      </div>
    {/if}
  </div>
</div>

<style>
  .git-status {
    display:flex; flex-direction:column; height:100%; background:transparent;
    color:var(--text-primary); font-size:var(--font-size); overflow:hidden;
  }
  .git-header {
    display:flex; justify-content:space-between; align-items:center;
    padding:8px 12px; border-bottom:1px solid var(--border-subtle);
    background:var(--bg-secondary); flex-shrink:0;
  }
  .git-branch-info { display:flex; align-items:center; gap:6px; color:var(--accent-blue); font-weight:500; }
  .branch-icon { flex-shrink:0; }
  .branch-name { font-size:var(--fs-11); white-space:nowrap; overflow:hidden; text-overflow:ellipsis; max-width:120px; }
  .git-header-actions { display:flex; gap:2px; align-items:center; }
  .git-header-btn {
    background:none; border:none; color:var(--text-muted); cursor:pointer;
    padding:4px; border-radius:4px; display:flex; align-items:center; justify-content:center; transition:all 0.12s;
  }
  .git-header-btn:hover { color:var(--text-primary); background:var(--bg-hover); }
  .git-header-btn:disabled { opacity:0.4; cursor:not-allowed; }
  .spinning { animation:spin 0.8s linear infinite; }
  @keyframes spin { to { transform:rotate(360deg); } }

  .git-body {
    flex:1; overflow-y:auto; display:flex; flex-direction:column; padding:10px 12px; gap:12px;
  }
  .git-body::-webkit-scrollbar { width:4px; }
  .git-body::-webkit-scrollbar-thumb { background:var(--bg-hover); border-radius:2px; }

  .no-repo {
    display:flex; flex-direction:column; align-items:center; justify-content:center;
    padding:32px 16px; gap:8px; text-align:center;
  }
  .no-repo p { font-size:var(--font-size); font-weight:500; color:var(--text-secondary); margin:0; }
  .no-repo span { font-size:var(--fs-10); color:var(--text-muted); }
  .init-btn {
    margin-top:8px; background:var(--accent-blue); color:var(--bg-primary);
    border:none; border-radius:6px; padding:8px 20px; font-size:var(--fs-11);
    font-weight:600; cursor:pointer; transition:filter 0.12s;
  }
  .init-btn:hover:not(:disabled) { filter:brightness(1.15); }
  .init-btn:disabled { opacity:0.5; cursor:not-allowed; }

  .commit-section {
    display:flex; flex-direction:column; gap:6px;
    background:var(--bg-surface); border:1px solid var(--border-subtle);
    border-radius:8px; padding:8px; flex-shrink:0;
  }
  .commit-textarea {
    flex:1; background:var(--bg-primary); color:var(--text-primary);
    border:1px solid var(--border-subtle); border-radius:5px;
    padding:6px; font-family:inherit; font-size:var(--fs-11);
    resize:none; outline:none; transition:border-color 0.12s; min-width:0;
  }
  .commit-textarea:focus { border-color:var(--accent-blue); }
  .ai-commit-btn {
    display:flex; align-items:center; justify-content:center;
    background:color-mix(in srgb, var(--accent-purple, #a855f7) 12%, transparent);
    color:var(--accent-purple, #a855f7); border:1px solid color-mix(in srgb, var(--accent-purple, #a855f7) 25%, transparent);
    border-radius:5px; width:32px; cursor:pointer; flex-shrink:0; transition:all 0.12s;
  }
  .ai-commit-btn:hover:not(:disabled) { filter:brightness(1.2); }
  .ai-commit-btn:disabled { opacity:0.4; cursor:not-allowed; }
  .spinner-tiny {
    width:12px; height:12px; border:1.5px solid var(--accent-purple, #a855f7);
    border-top-color:transparent; border-radius:50%; animation:spin 0.5s linear infinite;
  }
  .commit-btn {
    flex:1; background:var(--accent-blue); color:var(--bg-primary);
    border:none; border-radius:5px; padding:5px 10px; font-size:var(--fs-11);
    font-weight:600; cursor:pointer; transition:opacity 0.12s;
  }
  .commit-btn:hover:not(:disabled) { filter:brightness(1.1); }
  .commit-btn:disabled { opacity:0.4; cursor:not-allowed; }
  .pr-btn {
    display:inline-flex; align-items:center; gap:4px;
    background:color-mix(in srgb, var(--accent-green) 12%, transparent);
    color:var(--accent-green); border:1px solid color-mix(in srgb, var(--accent-green) 25%, transparent);
    border-radius:5px; padding:5px 10px; font-size:var(--fs-10);
    font-weight:600; cursor:pointer; transition:all 0.12s; white-space:nowrap;
  }
  .pr-btn:hover:not(:disabled) { filter:brightness(1.15); }
  .pr-btn:disabled { opacity:0.4; cursor:not-allowed; }
  .commit-message-log {
    font-size:10.5px; color:var(--accent-green); margin-top:2px;
    padding:3px 6px; background:color-mix(in srgb, var(--accent-green) 10%, transparent);
    border-radius:4px; word-break:break-all;
  }
  .commit-message-log.error {
    color:var(--accent-red); background:color-mix(in srgb, var(--accent-red) 10%, transparent);
  }

  .changes-section { flex:1; display:flex; flex-direction:column; gap:14px; }
  .empty-state {
    display:flex; flex-direction:column; align-items:center; justify-content:center;
    padding:40px 10px; color:var(--text-muted); text-align:center;
  }
  .empty-state svg { color:var(--accent-green); margin-bottom:8px; }
  .empty-state p { font-size:var(--font-size); font-weight:500; margin:0; color:var(--text-secondary); }
  .empty-state span { font-size:var(--fs-10); opacity:0.8; }

  .changes-group { display:flex; flex-direction:column; gap:4px; }
  .group-header { display:flex; justify-content:space-between; align-items:center; padding-right:2px; }
  .group-title { font-size:var(--fs-10); font-weight:600; color:var(--text-muted); text-transform:uppercase; letter-spacing:0.5px; padding-left:2px; }
  .bulk-action-btn { background:none; border:none; color:var(--accent-blue); font-size:var(--fs-10); font-weight:600; cursor:pointer; padding:2px 6px; border-radius:3px; transition:all 0.12s; }
  .bulk-action-btn:hover:not(:disabled) { background:var(--bg-hover); }
  .bulk-action-btn:disabled { opacity:0.5; cursor:not-allowed; }
  .changes-list { display:flex; flex-direction:column; gap:2px; }
  .change-item {
    position:relative; display:flex; align-items:center; gap:6px;
    padding:5px 8px; background:var(--bg-surface);
    border:1px solid var(--border-subtle); border-radius:5px; transition:all 0.1s;
  }
  .change-item:hover { border-color:var(--border-primary); background:var(--bg-hover); }
  .change-badge { font-size:var(--fs-9); font-weight:700; width:15px; height:15px; display:inline-flex; align-items:center; justify-content:center; border-radius:3px; }
  .badge-staged { background:color-mix(in srgb, var(--accent-green) 15%, transparent); color:var(--accent-green); }
  .badge-modified { background:color-mix(in srgb, var(--accent-blue) 15%, transparent); color:var(--accent-blue); }
  .badge-untracked { background:color-mix(in srgb, var(--text-muted) 25%, transparent); color:var(--text-muted); }
  .change-path { font-weight:500; color:var(--text-primary); }
  .change-fullpath { font-size:var(--fs-9-5); color:var(--text-muted); overflow:hidden; text-overflow:ellipsis; white-space:nowrap; flex:1; text-align:right; margin-left:6px; direction:rtl; transition:opacity 0.12s; }
  .change-item:hover .change-fullpath { opacity:0.1; }
  .change-actions {
    display:none; position:absolute; right:6px; top:50%; transform:translateY(-50%);
    gap:2px; background:var(--bg-surface); padding:1px; border-radius:4px;
    border:1px solid var(--border-subtle); box-shadow:0 2px 6px rgba(0,0,0,0.2);
  }
  .change-item:hover .change-actions { display:flex; }
  .action-btn {
    display:flex; align-items:center; justify-content:center;
    background:none; border:none; color:var(--text-muted);
    width:18px; height:18px; cursor:pointer; border-radius:3px; transition:all 0.1s;
  }
  .action-btn:hover:not(:disabled) { color:var(--text-primary); background:var(--bg-hover); }
  .action-btn:disabled { opacity:0.5; cursor:not-allowed; }

  .history-section { border-top:1px solid var(--border-subtle); padding-top:8px; }
  .history-header { display:flex; align-items:center; gap:6px; font-size:var(--fs-10); font-weight:600; color:var(--text-muted); text-transform:uppercase; letter-spacing:0.5px; margin-bottom:6px; }
  .history-list { display:flex; flex-direction:column; gap:0; position:relative; padding-left:16px; }
  .history-list::before { content:""; position:absolute; left:5px; top:6px; bottom:6px; width:1px; background:var(--border-subtle); }
  .history-item { display:flex; align-items:flex-start; gap:8px; padding:4px 0; position:relative; }
  .history-dot { width:7px; height:7px; border-radius:50%; background:var(--accent-blue); flex-shrink:0; margin-top:5px; margin-left:-12px; border:1px solid var(--bg-secondary); }
  .history-content { display:flex; flex-direction:column; gap:1px; min-width:0; flex:1; }
  .history-msg { font-size:var(--fs-11); color:var(--text-primary); white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }
  .history-meta { display:flex; gap:8px; font-size:var(--fs-9-5); color:var(--text-muted); }
  .history-hash { font-family:monospace; color:var(--accent-blue); font-weight:500; }
</style>
