<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { addToast } from "$lib/stores.svelte";
  import Terminal from "./Terminal.svelte";

  let {
    profile = null as any,
    onOpenFile = (sid: string, path: string, name: string) => {},
    onClose = () => {},
  } = $props();

  type SftpEntry = {
    name: string;
    path: string;
    is_dir: boolean;
    size: number;
    permissions: string;
    modified: string;
  };

  type SshExecResult = { stdout: string; stderr: string; exit_code: number };

  // ── Session state ────────────────────────────────────────────────────────
  let sessionId = $state<string | null>(null);
  let isConnecting = $state(false);
  let isConnected = $state(false);
  let connectError = $state("");

  // ── Inner tabs ───────────────────────────────────────────────────────────
  let activeTab = $state<"terminal" | "sftp" | "tunnels">("terminal");

  // ── SFTP state ───────────────────────────────────────────────────────────
  let sftpPath = $state("/home");
  let sftpEntries = $state<SftpEntry[]>([]);
  let sftpLoading = $state(false);
  let sftpError = $state("");
  let sftpSelected = $state<string | null>(null);
  let breadcrumbs = $derived(sftpPath.split("/").filter(Boolean));

  // ── Tunnel state ─────────────────────────────────────────────────────────
  let tunnelLocalPort = $state(8080);
  let tunnelRemoteHost = $state("localhost");
  let tunnelRemotePort = $state(80);
  let tunnelType = $state<"local" | "remote" | "dynamic">("local");
  let tunnelStatus = $state("");

  // ── PTY terminal session ─────────────────────────────────────────────────
  let ptySessionId = $state<string | null>(null);
  let storedPassword = $state<string | null>(null);

  async function connect() {
    if (!profile) return;
    isConnecting = true;
    connectError = "";
    try {
      // Get password from secrets if stored
      let password: string | null = null;
      try {
        password = await invoke<string | null>("secrets_get", {
          service: "nyxedit-ssh",
          account: profile.id,
        });
      } catch {}

      storedPassword = password;
      const info = await invoke<{ id: string }>("ssh_connect", {
        host: profile.host,
        port: profile.port,
        username: profile.username,
        password: password || undefined,
        privateKeyPath: null,
      });
      sessionId = info.id;
      isConnected = true;
      addToast(`Connected to ${profile.name}`, "success");

      // Load initial SFTP listing
      await loadSftp(".");
    } catch (e: any) {
      connectError = String(e);
      addToast(`SSH connection failed: ${e}`, "error");
    }
    isConnecting = false;
  }

  async function disconnect() {
    if (!sessionId) return;
    try {
      await invoke("ssh_disconnect", { sessionId });
    } catch {}
    sessionId = null;
    isConnected = false;
    sftpEntries = [];
    addToast("SSH session closed");
  }

  async function loadSftp(path: string) {
    if (!sessionId) return;
    sftpLoading = true;
    sftpError = "";
    try {
      const res = await invoke<{ path: string; entries: SftpEntry[] }>(
        "sftp_list_dir",
        {
          sessionId,
          remotePath: path,
        },
      );
      sftpEntries = res.entries;
      sftpPath = res.path;
      sftpSelected = null;
    } catch (e: any) {
      sftpError = String(e);
    }
    sftpLoading = false;
  }

  async function openRemoteFile(entry: SftpEntry) {
    if (!sessionId || entry.is_dir) return;
    onOpenFile(sessionId, entry.path, entry.name);
  }

  function navigateTo(entry: SftpEntry) {
    if (entry.is_dir) {
      loadSftp(entry.path);
    } else {
      openRemoteFile(entry);
    }
  }

  function navigateUp() {
    const parts = sftpPath.split("/").filter(Boolean);
    if (parts.length === 0) return;
    parts.pop();
    loadSftp("/" + parts.join("/") || "/");
  }

  function navigateToBreadcrumb(index: number) {
    const parts = sftpPath.split("/").filter(Boolean);
    const newPath = "/" + parts.slice(0, index + 1).join("/");
    loadSftp(newPath);
  }

  async function deleteSftpEntry(entry: SftpEntry) {
    if (!sessionId) return;
    if (!confirm(`Delete "${entry.name}"?`)) return;
    try {
      await invoke("sftp_delete", {
        sessionId,
        remotePath: entry.path,
        isDir: entry.is_dir,
      });
      addToast(`Deleted ${entry.name}`, "success");
      await loadSftp(sftpPath);
    } catch (e: any) {
      addToast(`Delete failed: ${e}`, "error");
    }
  }

  async function createFolder() {
    const name = prompt("Folder name:");
    if (!name || !sessionId) return;
    try {
      await invoke("sftp_mkdir", {
        sessionId,
        remotePath: `${sftpPath}/${name}`,
      });
      addToast("Folder created", "success");
      await loadSftp(sftpPath);
    } catch (e: any) {
      addToast(`Create folder failed: ${e}`, "error");
    }
  }

  async function readRemoteFile(entry: SftpEntry) {
    if (!sessionId || entry.is_dir) return;
    openRemoteFile(entry);
  }

  function formatSize(bytes: number): string {
    if (bytes === 0) return "—";
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  }

  async function setupTunnel() {
    if (!sessionId) return;
    tunnelStatus = "Setting up tunnel...";
    try {
      const cmd =
        tunnelType === "local"
          ? `ssh -L ${tunnelLocalPort}:${tunnelRemoteHost}:${tunnelRemotePort} -N ${profile.username}@${profile.host} -p ${profile.port}`
          : tunnelType === "remote"
            ? `ssh -R ${tunnelRemotePort}:localhost:${tunnelLocalPort} -N ${profile.username}@${profile.host} -p ${profile.port}`
            : `ssh -D ${tunnelLocalPort} -N ${profile.username}@${profile.host} -p ${profile.port}`;
      tunnelStatus = `Tunnel command: ${cmd}\n(Run in a separate terminal to activate)`;
    } catch (e: any) {
      tunnelStatus = `Error: ${e}`;
    }
  }

  $effect(() => {
    if (profile && !isConnected && !isConnecting) {
      connect();
    }
  });
</script>

<div class="ssh-session">
  <!-- Connection header -->
  <div class="ssh-header">
    <div class="host-info">
      <div
        class="host-dot"
        class:connected={isConnected}
        class:connecting={isConnecting}
      ></div>
      <span class="host-name"
        >{profile?.username}@{profile?.host}:{profile?.port}</span
      >
      <span class="profile-label">{profile?.name}</span>
    </div>
    <div class="header-actions">
      {#if isConnected}
        <button class="action-btn danger" onclick={disconnect}
          >Disconnect</button
        >
      {:else if isConnecting}
        <span class="connecting-text">Connecting...</span>
      {:else}
        <button class="action-btn primary" onclick={connect}>Connect</button>
      {/if}
    </div>
  </div>

  {#if connectError}
    <div class="error-banner">{connectError}</div>
  {/if}

  {#if isConnected}
    <!-- Sub-tab bar -->
    <div class="sub-tab-bar">
      <button
        class="sub-tab"
        class:active={activeTab === "terminal"}
        onclick={() => (activeTab = "terminal")}
      >
        <svg
          width="12"
          height="12"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          ><polyline points="4 17 10 11 4 5" /><line
            x1="12"
            y1="19"
            x2="20"
            y2="19"
          /></svg
        >
        SSH Terminal
      </button>
      <button
        class="sub-tab"
        class:active={activeTab === "sftp"}
        onclick={() => {
          activeTab = "sftp";
          if (sftpEntries.length === 0) loadSftp(sftpPath);
        }}
      >
        <svg
          width="12"
          height="12"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          ><path
            d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"
          /></svg
        >
        SFTP Browser
      </button>
      <button
        class="sub-tab"
        class:active={activeTab === "tunnels"}
        onclick={() => (activeTab = "tunnels")}
      >
        <svg
          width="12"
          height="12"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          ><path d="M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01" /></svg
        >
        Tunnels
      </button>
    </div>

    <!-- SSH Terminal -->
    {#if activeTab === "terminal"}
      <div class="sub-panel terminal-panel">
        <Terminal
          shell={`ssh -o StrictHostKeyChecking=no ${profile.username}@${profile.host} -p ${profile.port}`}
          password={storedPassword ?? undefined}
          onReady={(sid: string) => {
            ptySessionId = sid;
          }}
          onCommand={() => {}}
        />
      </div>
    {/if}

    <!-- SFTP Browser -->
    {#if activeTab === "sftp"}
      <div class="sub-panel sftp-panel">
        <!-- SFTP toolbar -->
        <div class="sftp-toolbar">
          <button
            class="sftp-btn"
            onclick={navigateUp}
            title="Go up"
            disabled={sftpPath === "/"}
          >
            <svg
              width="12"
              height="12"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              ><polyline points="17 11 12 6 7 11" /><polyline
                points="17 18 12 13 7 18"
              /></svg
            >
          </button>
          <!-- Breadcrumb -->
          <div class="breadcrumb">
            <span class="bc-item" onclick={() => loadSftp("/")} role="button"
              >/</span
            >
            {#each breadcrumbs as crumb, i}
              <span class="bc-sep">/</span>
              <span
                class="bc-item"
                onclick={() => navigateToBreadcrumb(i)}
                role="button">{crumb}</span
              >
            {/each}
          </div>
          <button class="sftp-btn" onclick={createFolder} title="New Folder">
            <svg
              width="12"
              height="12"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              ><path
                d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"
              /><line x1="12" y1="11" x2="12" y2="17" /><line
                x1="9"
                y1="14"
                x2="15"
                y2="14"
              /></svg
            >
          </button>
          <button
            class="sftp-btn"
            onclick={() => loadSftp(sftpPath)}
            title="Refresh"
          >
            <svg
              width="12"
              height="12"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              ><polyline points="23 4 23 10 17 10" /><path
                d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"
              /></svg
            >
          </button>
        </div>

        {#if sftpError}
          <div class="sftp-error">{sftpError}</div>
        {/if}

        <!-- File list -->
        <div class="sftp-list">
          {#if sftpLoading}
            <div class="sftp-empty">Loading...</div>
          {:else if sftpEntries.length === 0}
            <div class="sftp-empty">Empty directory</div>
          {:else}
            <div class="sftp-table-header">
              <span class="col-name">Name</span>
              <span class="col-size">Size</span>
              <span class="col-perm">Permissions</span>
              <span class="col-date">Modified</span>
              <span class="col-actions"></span>
            </div>
            {#each sftpEntries as entry}
              <div
                class="sftp-row"
                class:selected={sftpSelected === entry.path}
                class:is-dir={entry.is_dir}
                onclick={() => (sftpSelected = entry.path)}
                ondblclick={() => navigateTo(entry)}
                role="row"
              >
                <span class="col-name">
                  {#if entry.is_dir}
                    <svg
                      width="13"
                      height="13"
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="currentColor"
                      stroke-width="2"
                      ><path
                        d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"
                      /></svg
                    >
                  {:else}
                    <svg
                      width="13"
                      height="13"
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="currentColor"
                      stroke-width="2"
                      ><path
                        d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5 2z"
                      /><polyline points="14 2 14 8 20 8" /></svg
                    >
                  {/if}
                  {entry.name}
                </span>
                <span class="col-size"
                  >{entry.is_dir ? "—" : formatSize(entry.size)}</span
                >
                <span class="col-perm">{entry.permissions}</span>
                <span class="col-date">{entry.modified}</span>
                <span class="col-actions">
                  {#if !entry.is_dir}
                    <button
                      class="row-btn"
                      onclick={() => readRemoteFile(entry)}
                      title="View"
                    >
                      <svg
                        width="10"
                        height="10"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2"
                        ><path
                          d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"
                        /><circle cx="12" cy="12" r="3" /></svg
                      >
                    </button>
                  {/if}
                  <button
                    class="row-btn danger"
                    onclick={() => deleteSftpEntry(entry)}
                    title="Delete"
                  >
                    <svg
                      width="10"
                      height="10"
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="currentColor"
                      stroke-width="2"
                      ><polyline points="3 6 5 6 21 6" /><path
                        d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"
                      /></svg
                    >
                  </button>
                </span>
              </div>
            {/each}
          {/if}
        </div>
      </div>
    {/if}

    <!-- Tunnels -->
    {#if activeTab === "tunnels"}
      <div class="sub-panel tunnels-panel">
        <div class="tunnel-section">
          <div class="tunnel-title">PORT FORWARDING</div>
          <div class="tunnel-form">
            <label class="form-label">Type</label>
            <div class="tunnel-type-row">
              {#each ["local", "remote", "dynamic"] as t}
                <button
                  class="type-btn"
                  class:active={tunnelType === t}
                  onclick={() => (tunnelType = t as any)}>{t}</button
                >
              {/each}
            </div>
            {#if tunnelType !== "dynamic"}
              <div class="form-row">
                <div class="form-group">
                  <label class="form-label">Local Port</label>
                  <input
                    type="number"
                    bind:value={tunnelLocalPort}
                    class="form-input"
                  />
                </div>
                <div class="form-group">
                  <label class="form-label">Remote Host</label>
                  <input bind:value={tunnelRemoteHost} class="form-input" />
                </div>
                <div class="form-group">
                  <label class="form-label">Remote Port</label>
                  <input
                    type="number"
                    bind:value={tunnelRemotePort}
                    class="form-input"
                  />
                </div>
              </div>
            {:else}
              <div class="form-row">
                <div class="form-group">
                  <label class="form-label">SOCKS Port</label>
                  <input
                    type="number"
                    bind:value={tunnelLocalPort}
                    class="form-input"
                  />
                </div>
              </div>
            {/if}
            <button class="tunnel-setup-btn" onclick={setupTunnel}
              >Generate Tunnel Command</button
            >
          </div>
          {#if tunnelStatus}
            <pre class="tunnel-status">{tunnelStatus}</pre>
          {/if}
        </div>

        <div class="tunnel-info">
          <div class="tunnel-title">QUICK REFERENCE</div>
          <div class="tunnel-tips">
            <div class="tip">
              <strong>Local:</strong> Forward local port → remote service
            </div>
            <div class="tip">
              <strong>Remote:</strong> Expose local port on remote server
            </div>
            <div class="tip">
              <strong>Dynamic:</strong> SOCKS5 proxy via SSH
            </div>
          </div>
        </div>
      </div>
    {/if}
  {:else if !isConnecting}
    <div class="disconnected-state">
      <svg
        width="32"
        height="32"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="1.5"
        ><rect x="2" y="2" width="20" height="20" rx="4" /><line
          x1="6"
          y1="6"
          x2="18"
          y2="6"
        /><line x1="6" y1="12" x2="18" y2="12" /><line
          x1="6"
          y1="18"
          x2="18"
          y2="18"
        /></svg
      >
      <p>Not connected to {profile?.name}</p>
      <button class="action-btn primary" onclick={connect}>Connect</button>
    </div>
  {:else}
    <div class="disconnected-state">
      <div class="spinner"></div>
      <p>Connecting to {profile?.host}...</p>
    </div>
  {/if}
</div>

<style>
  .ssh-session {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--glass-bg, var(--bg-secondary));
    backdrop-filter: blur(var(--glass-blur, 12px));
    -webkit-backdrop-filter: blur(var(--glass-blur, 12px));
    font-size: var(--fs-11);
  }

  .ssh-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 10px;
    border-bottom: 1px solid var(--border-subtle);
    flex-shrink: 0;
    background: var(--bg-secondary);
  }
  .host-info {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .host-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--text-muted);
    transition: background 0.2s;
  }
  .host-dot.connected {
    background: var(--accent-green);
    box-shadow: 0 0 6px color-mix(in srgb, var(--accent-green) 50%, transparent);
  }
  .host-dot.connecting {
    background: var(--accent-yellow);
    animation: pulse 1s infinite;
  }
  @keyframes pulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.3;
    }
  }
  .host-name {
    font-weight: 600;
    color: var(--text-primary);
    font-family: monospace;
  }
  .profile-label {
    color: var(--text-muted);
    font-size: var(--fs-10);
  }
  .header-actions {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .connecting-text {
    color: var(--text-muted);
    font-size: var(--fs-10);
  }

  .action-btn {
    border: none;
    border-radius: 4px;
    padding: 4px 10px;
    font-size: var(--fs-10);
    font-weight: 600;
    cursor: pointer;
    transition: all 0.12s ease;
  }
  .action-btn.primary {
    background: var(--accent-blue);
    color: var(--bg-primary);
  }
  .action-btn.primary:hover {
    filter: brightness(1.15);
  }
  .action-btn.danger {
    background: color-mix(in srgb, var(--accent-red) 15%, transparent);
    color: var(--accent-red);
    border: 1px solid var(--accent-red);
  }
  .action-btn.danger:hover {
    background: var(--accent-red);
    color: var(--bg-primary);
  }

  .error-banner {
    background: color-mix(in srgb, var(--accent-red) 12%, transparent);
    color: var(--accent-red);
    padding: 6px 10px;
    font-size: var(--fs-10);
    border-bottom: 1px solid var(--border-subtle);
  }

  /* Sub-tabs */
  .sub-tab-bar {
    display: flex;
    gap: 0;
    border-bottom: 1px solid var(--border-subtle);
    flex-shrink: 0;
    background: var(--bg-secondary);
  }
  .sub-tab {
    display: flex;
    align-items: center;
    gap: 5px;
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    padding: 6px 12px;
    color: var(--text-muted);
    font-size: var(--fs-10);
    font-weight: 500;
    cursor: pointer;
    transition: all 0.12s ease;
  }
  .sub-tab:hover {
    color: var(--text-primary);
  }
  .sub-tab.active {
    color: var(--accent-blue);
    border-bottom-color: var(--accent-blue);
  }

  .sub-panel {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .terminal-panel {
  }

  /* SFTP */
  .sftp-toolbar {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 8px;
    border-bottom: 1px solid var(--border-subtle);
    background: var(--bg-surface);
    flex-shrink: 0;
  }
  .sftp-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: none;
    color: var(--text-muted);
    padding: 3px 5px;
    border-radius: 3px;
    cursor: pointer;
  }
  .sftp-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }
  .sftp-btn:disabled {
    opacity: 0.3;
    cursor: not-allowed;
  }
  .breadcrumb {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 2px;
    font-family: monospace;
    font-size: var(--fs-10);
    overflow: hidden;
  }
  .bc-item {
    color: var(--accent-blue);
    cursor: pointer;
    white-space: nowrap;
  }
  .bc-item:hover {
    text-decoration: underline;
  }
  .bc-sep {
    color: var(--text-muted);
  }
  .sftp-error {
    padding: 6px 8px;
    background: color-mix(in srgb, var(--accent-red) 10%, transparent);
    color: var(--accent-red);
    font-size: var(--fs-10);
    border-bottom: 1px solid var(--border-subtle);
  }
  .sftp-list {
    flex: 1;
    overflow-y: auto;
  }
  .sftp-table-header {
    display: grid;
    grid-template-columns: 1fr 80px 110px 130px 60px;
    padding: 3px 8px;
    background: var(--bg-hover);
    border-bottom: 1px solid var(--border-subtle);
    font-size: var(--fs-9);
    color: var(--text-muted);
    text-transform: uppercase;
    font-weight: 600;
    letter-spacing: 0.5px;
  }
  .sftp-row {
    display: grid;
    grid-template-columns: 1fr 80px 110px 130px 60px;
    padding: 4px 8px;
    border-bottom: 1px solid var(--border-subtle);
    cursor: pointer;
    transition: background 0.1s;
    align-items: center;
  }
  .sftp-row:hover {
    background: var(--bg-hover);
  }
  .sftp-row.selected {
    background: color-mix(in srgb, var(--accent-blue) 10%, transparent);
  }
  .sftp-row.is-dir .col-name {
    color: var(--accent-blue);
    font-weight: 500;
  }
  .col-name {
    display: flex;
    align-items: center;
    gap: 6px;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .col-size,
  .col-perm,
  .col-date {
    font-family: monospace;
    font-size: var(--fs-9);
    color: var(--text-muted);
  }
  .col-actions {
    display: flex;
    gap: 2px;
    justify-content: flex-end;
    opacity: 0;
    transition: opacity 0.1s;
  }
  .sftp-row:hover .col-actions {
    opacity: 1;
  }
  .row-btn {
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    padding: 2px 3px;
    border-radius: 3px;
    display: flex;
    align-items: center;
  }
  .row-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }
  .row-btn.danger:hover {
    color: var(--accent-red);
  }
  .sftp-empty {
    padding: 24px;
    text-align: center;
    color: var(--text-muted);
  }

  /* Tunnels */
  .tunnels-panel {
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    overflow-y: auto;
  }
  .tunnel-section {
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: 8px;
    padding: 10px;
  }
  .tunnel-title {
    font-size: var(--fs-9);
    font-weight: 700;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.5px;
    margin-bottom: 8px;
  }
  .tunnel-form {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .tunnel-type-row {
    display: flex;
    gap: 4px;
  }
  .type-btn {
    flex: 1;
    background: var(--bg-hover);
    border: 1px solid var(--border-subtle);
    border-radius: 4px;
    padding: 4px;
    font-size: var(--fs-10);
    font-weight: 600;
    cursor: pointer;
    color: var(--text-muted);
    transition: all 0.12s;
    text-transform: capitalize;
  }
  .type-btn.active {
    background: var(--accent-blue);
    color: var(--bg-primary);
    border-color: var(--accent-blue);
  }
  .form-row {
    display: flex;
    gap: 8px;
  }
  .form-group {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .form-label {
    font-size: var(--fs-9);
    color: var(--text-muted);
    font-weight: 600;
  }
  .form-input {
    background: var(--bg-primary);
    color: var(--text-primary);
    border: 1px solid var(--border-subtle);
    border-radius: 4px;
    padding: 4px 6px;
    font-size: var(--fs-10);
    outline: none;
  }
  .form-input:focus {
    border-color: var(--accent-blue);
  }
  .tunnel-setup-btn {
    background: var(--accent-blue);
    color: var(--bg-primary);
    border: none;
    border-radius: 4px;
    padding: 6px;
    font-size: var(--fs-10);
    font-weight: 600;
    cursor: pointer;
  }
  .tunnel-setup-btn:hover {
    filter: brightness(1.15);
  }
  .tunnel-status {
    background: var(--bg-primary);
    border: 1px solid var(--border-subtle);
    border-radius: 4px;
    padding: 8px;
    font-size: var(--fs-10);
    white-space: pre-wrap;
    word-break: break-all;
    margin-top: 6px;
    color: var(--accent-green);
  }
  .tunnel-info {
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: 8px;
    padding: 10px;
  }
  .tunnel-tips {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .tip {
    font-size: var(--fs-10);
    color: var(--text-muted);
    line-height: 1.4;
  }
  .tip strong {
    color: var(--text-primary);
  }

  /* Disconnected state */
  .disconnected-state {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    color: var(--text-muted);
    padding: 24px;
    text-align: center;
  }
  .disconnected-state p {
    font-size: var(--fs-11);
  }
  .spinner {
    width: 24px;
    height: 24px;
    border: 2px solid var(--border-subtle);
    border-top-color: var(--accent-blue);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
