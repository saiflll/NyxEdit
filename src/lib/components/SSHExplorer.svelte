<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { activeSshProfile, addToast } from "$lib/stores.svelte";

  type SftpEntry = {
    name: string;
    path: string;
    is_dir: boolean;
    size: number;
    permissions: string;
    modified: string;
  };

  type SftpListResult = {
    path: string;
    entries: SftpEntry[];
  };

  type SshSessionInfo = {
    id: string;
    host: string;
    port: number;
    username: string;
    connected: boolean;
  };

  let profile = $state<any>(null);
  let sessionId = $state<string | null>(null);
  let currentPath = $state<string>("");
  let entries = $state<SftpEntry[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let connected = $state(false);

  $effect(() => {
    const unsub = activeSshProfile.subscribe(val => {
      profile = val;
      if (val) {
        connectAndBrowse(val);
      }
    });
    return unsub;
  });

  async function connectAndBrowse(prof: any) {
    loading = true;
    error = null;
    entries = [];
    connected = false;
    try {
      let password: string | undefined;
      try {
        const stored = await invoke<string | null>("secrets_get", {
          service: "nyxedit-ssh",
          account: prof.id,
        });
        if (stored) password = stored;
      } catch {}

      const info = await invoke<SshSessionInfo>("ssh_connect", {
        host: prof.host,
        port: prof.port,
        username: prof.username,
        password: password ?? null,
        privateKeyPath: null,
      });
      sessionId = info.id;
      connected = true;
      addToast(`SSH Explorer connected to ${prof.name}`, "success");
      await listDir("/");
    } catch (err: any) {
      error = typeof err === "string" ? err : err.message || "Connection failed";
      addToast(`SSH Explorer: ${error}`, "error");
    } finally {
      loading = false;
    }
  }

  async function listDir(path: string) {
    if (!sessionId) return;
    loading = true;
    error = null;
    try {
      const result = await invoke<SftpListResult>("sftp_list_dir", {
        sessionId,
        remotePath: path,
      });
      currentPath = result.path;
      entries = result.entries;
    } catch (err: any) {
      error = typeof err === "string" ? err : err.message || "List failed";
      addToast(`SSH Explorer: ${error}`, "error");
    } finally {
      loading = false;
    }
  }

  function navigateToDir(dirPath: string) {
    listDir(dirPath);
  }

  function navigateUp() {
    if (currentPath === "/") return;
    const parent = currentPath === "" ? "/" : currentPath.substring(0, currentPath.lastIndexOf("/"));
    listDir(parent || "/");
  }

  function openFile(entry: SftpEntry) {
    addToast(`Opening remote file: ${entry.path} (SFTP/${entry.size} bytes)`);
  }

  function formatSize(bytes: number): string {
    if (bytes === 0) return "-";
    if (bytes < 1024) return `${bytes}B`;
    if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)}K`;
    return `${(bytes / 1048576).toFixed(1)}M`;
  }
</script>

<div class="ssh-explorer">
  {#if !profile}
    <div class="empty-state">
      <p>No active SSH connection.</p>
      <span>Select a profile from SSH Tree sidebar.</span>
    </div>
  {:else if loading && !sessionId}
    <div class="empty-state">
      <p>Connecting to {profile.name}...</p>
      <span class="loading-dots"></span>
    </div>
  {:else if error && !connected}
    <div class="empty-state error-state">
      <p>Connection failed</p>
      <span class="error-msg">{error}</span>
      <button class="retry-btn" onclick={() => connectAndBrowse(profile)}>Retry</button>
    </div>
  {:else}
    <div class="explorer-header" title="{profile.username}@{profile.host}:{profile.port}">
      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2"/></svg>
      <span class="host-badge">{profile.username}@{profile.host}</span>
      {#if loading}
        <span class="loading-spin"></span>
      {/if}
    </div>

    <div class="path-bar">
      <button class="path-btn" onclick={() => listDir("/")} title="Go to root">
        <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/></svg>
      </button>
      <button class="path-btn" onclick={navigateUp} title="Go up">
        <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 19V5m0 0l-7 7m7-7l7 7"/></svg>
      </button>
      <span class="path-text">{currentPath || "/"}</span>
    </div>

    <div class="explorer-list">
      {#if currentPath !== "/"}
        <div class="entry-row entry-dir" onclick={navigateUp} role="presentation">
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M4 20h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.93a2 2 0 0 1-1.66-.9l-.82-1.2A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13c0 1.1.9 2 2 2Z"/></svg>
          <span class="entry-name">..</span>
          <span class="entry-meta">parent</span>
        </div>
      {/if}
      {#each entries as entry}
        <div
          class="entry-row"
          class:entry-dir={entry.is_dir}
          onclick={() => entry.is_dir ? navigateToDir(entry.path) : openFile(entry)}
          role="presentation"
          title="{entry.permissions}  {entry.modified}"
        >
          {#if entry.is_dir}
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M4 20h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.93a2 2 0 0 1-1.66-.9l-.82-1.2A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13c0 1.1.9 2 2 2Z"/></svg>
          {:else}
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5 2z"/></svg>
          {/if}
          <span class="entry-name">{entry.name}</span>
          <span class="entry-meta">{entry.is_dir ? "" : formatSize(entry.size)}</span>
        </div>
      {:else}
        {#if !loading}
          <div class="empty-state">(empty directory)</div>
        {/if}
      {/each}
    </div>
  {/if}
</div>

<style>
  .ssh-explorer {
    display: flex;
    flex-direction: column;
    height: 100%;
    font-size: var(--fs-11);
    color: var(--text-primary);
  }

  .explorer-header {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 10px;
    border-bottom: 1px solid var(--border-subtle);
    flex-shrink: 0;
  }

  .host-badge {
    font-weight: 600;
    color: var(--accent-blue);
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .loading-spin {
    width: 12px;
    height: 12px;
    border: 2px solid var(--border-subtle);
    border-top-color: var(--accent-blue);
    border-radius: 50%;
    animation: spin 0.6s linear infinite;
    flex-shrink: 0;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .path-bar {
    display: flex;
    align-items: center;
    gap: 3px;
    padding: 4px 8px;
    background: var(--bg-surface);
    border-bottom: 1px solid var(--border-subtle);
    flex-shrink: 0;
  }

  .path-btn {
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    padding: 2px 4px;
    border-radius: 3px;
    display: flex;
    align-items: center;
    flex-shrink: 0;
    transition: all 0.1s;
  }

  .path-btn:hover {
    color: var(--text-primary);
    background: var(--bg-hover);
  }

  .path-text {
    flex: 1;
    color: var(--text-muted);
    font-family: monospace;
    font-size: var(--fs-9-5);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .explorer-list {
    flex: 1;
    overflow-y: auto;
    padding: 2px 0;
  }

  .entry-row {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 3px 10px;
    cursor: pointer;
    transition: background 0.1s;
    user-select: none;
  }

  .entry-row:hover {
    background: var(--bg-hover);
  }

  .entry-row svg {
    flex-shrink: 0;
    color: var(--text-muted);
  }

  .entry-dir svg {
    color: var(--accent-yellow);
  }

  .entry-name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-primary);
  }

  .entry-dir .entry-name {
    color: var(--accent-blue);
  }

  .entry-meta {
    color: var(--text-muted);
    font-size: var(--fs-9-5);
    font-family: monospace;
    flex-shrink: 0;
  }

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: var(--text-muted);
    text-align: center;
    padding: 24px 8px;
    gap: 6px;
  }

  .empty-state p {
    font-weight: 600;
  }

  .error-state p {
    color: var(--accent-red);
  }

  .error-msg {
    font-size: var(--fs-9-5);
    color: var(--text-muted);
    word-break: break-word;
  }

  .retry-btn {
    background: var(--accent-blue);
    color: var(--bg-primary);
    border: none;
    border-radius: 4px;
    padding: 4px 16px;
    font-weight: 600;
    cursor: pointer;
    font-size: var(--fs-10);
    margin-top: 4px;
  }

  .retry-btn:hover {
    filter: brightness(1.1);
  }
</style>
