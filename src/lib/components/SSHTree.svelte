<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { addToast, activeSshProfile } from "$lib/stores.svelte";
  import { onMount } from "svelte";
  import { loadGlobalFile, saveGlobalFile } from "$lib/nyxConfig";

  type SshProfile = {
    id: string;
    name: string;
    host: string;
    port: number;
    username: string;
  };

  let { onConnect = (profile: SshProfile) => {} } = $props();

  let profiles = $state<SshProfile[]>([]);
  let activeProfileId = $state<string | null>(null);
  let showForm = $state(false);
  let editProfileId = $state<string | null>(null);

  // Form fields
  let formName = $state("");
  let formHost = $state("");
  let formPort = $state(22);
  let formUsername = $state("");
  let formPassword = $state("");

  // Edit form fields
  let editFormName = $state("");
  let editFormHost = $state("");
  let editFormPort = $state(22);
  let editFormUsername = $state("");
  let editFormPassword = $state("");

  onMount(() => {
    loadProfiles();
  });

  async function loadProfiles() {
    let loaded = await loadGlobalFile<SshProfile[]>("ssh_profiles.json", []);
    if (loaded.length > 0) {
      profiles = loaded;
    } else {
      // Migrate from localStorage
      try {
        const raw = localStorage.getItem("nyxedit-ssh-profiles");
        if (raw) {
          profiles = JSON.parse(raw);
          await saveGlobalFile("ssh_profiles.json", profiles);
          localStorage.removeItem("nyxedit-ssh-profiles");
        }
      } catch {}
    }
  }

  async function saveProfiles() {
    await saveGlobalFile("ssh_profiles.json", profiles);
  }

  async function handleAddProfile() {
    if (!formName || !formHost || !formUsername) return;
    const id = "ssh-" + Date.now();
    const profile: SshProfile = {
      id,
      name: formName,
      host: formHost,
      port: formPort,
      username: formUsername,
    };

    if (formPassword) {
      try {
        await invoke("secrets_set", {
          service: "nyxedit-ssh",
          account: id,
          password: formPassword,
        });
      } catch (err) {
        console.error("Failed to save secure password:", err);
      }
    }

    profiles = [...profiles, profile];
    saveProfiles();
    resetForm();
    addToast("SSH Profile added", "success");
  }

  function resetForm() {
    formName = "";
    formHost = "";
    formPort = 22;
    formUsername = "";
    formPassword = "";
    showForm = false;
  }

  async function deleteProfile(id: string) {
    profiles = profiles.filter((p) => p.id !== id);
    saveProfiles();
    try {
      await invoke("secrets_delete", { service: "nyxedit-ssh", account: id });
    } catch {}
    if (activeProfileId === id) activeProfileId = null;
    addToast("Profile deleted");
  }

  async function openSshExplorer(profile: SshProfile) {
    activeSshProfile.set(profile);
    addToast(`SSH Explorer loaded for ${profile.name}`, "success");
  }

  async function connectSsh(profile: SshProfile) {
    activeProfileId = profile.id;
    onConnect(profile);
  }

  function disconnectSsh() {
    activeProfileId = null;
  }

  function startEdit(profile: SshProfile) {
    editProfileId = profile.id;
    editFormName = profile.name;
    editFormHost = profile.host;
    editFormPort = profile.port;
    editFormUsername = profile.username;
    editFormPassword = "";
  }

  function cancelEdit() {
    editProfileId = null;
  }

  async function saveEdit(profile: SshProfile) {
    if (!editFormName || !editFormHost || !editFormUsername) return;
    const idx = profiles.findIndex((p) => p.id === profile.id);
    if (idx < 0) return;
    profiles[idx] = {
      ...profiles[idx],
      name: editFormName,
      host: editFormHost,
      port: editFormPort,
      username: editFormUsername,
    };
    if (editFormPassword) {
      try {
        await invoke("secrets_set", {
          service: "nyxedit-ssh",
          account: profile.id,
          password: editFormPassword,
        });
      } catch (err) {
        console.error("Failed to update password:", err);
      }
    }
    profiles = [...profiles];
    saveProfiles();
    editProfileId = null;
    addToast("SSH Profile updated", "success");
  }
</script>

<div class="ssh-container">
  <div class="ssh-header">
    <span class="title">SSH PROFILES</span>
    <button class="add-btn" onclick={() => (showForm = !showForm)}>
      {showForm ? "Cancel" : "+ Add"}
    </button>
  </div>

  {#if showForm}
    <div class="ssh-form">
      <input
        bind:value={formName}
        placeholder="Profile Name (e.g. Staging Server)"
      />
      <div class="row">
        <input bind:value={formHost} placeholder="Host / IP" style="flex: 3;" />
        <input
          type="number"
          bind:value={formPort}
          placeholder="Port"
          style="flex: 1;"
        />
      </div>
      <input bind:value={formUsername} placeholder="Username" />
      <input
        type="password"
        bind:value={formPassword}
        placeholder="Password (Securely Saved)"
      />
      <button class="save-btn" onclick={handleAddProfile}>Save Profile</button>
    </div>
  {/if}

  <div class="profiles-list">
    {#each profiles as profile}
      {#if editProfileId === profile.id}
        <div class="ssh-form" style="margin-bottom: 4px;">
          <input bind:value={editFormName} placeholder="Profile Name" />
          <div class="row">
            <input bind:value={editFormHost} placeholder="Host / IP" style="flex: 3;" />
            <input type="number" bind:value={editFormPort} placeholder="Port" style="flex: 1;" />
          </div>
          <input bind:value={editFormUsername} placeholder="Username" />
          <input type="password" bind:value={editFormPassword} placeholder="New Password (leave blank to keep)" />
          <div class="row" style="gap: 4px;">
            <button class="save-btn" style="flex: 1;" onclick={() => saveEdit(profile)}>Save</button>
            <button class="save-btn" style="flex: 1; background: var(--bg-hover); color: var(--text-primary);" onclick={cancelEdit}>Cancel</button>
          </div>
        </div>
      {:else}
        <div class="profile-card" class:active={activeProfileId === profile.id}>
          <div
            class="profile-info"
            onclick={() => connectSsh(profile)}
            role="presentation"
            title="Click to Connect in Terminal"
          >
            <span class="name">{profile.name}</span>
            <span class="detail"
              >{profile.username}@{profile.host}:{profile.port}</span
            >
          </div>
          <div class="profile-actions">
            {#if activeProfileId === profile.id}
              <button class="icon-btn" onclick={(e) => { e.stopPropagation(); disconnectSsh(); }} title="Disconnect">
                <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M10 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h4M17 16l4-4-4-4M11 12h10"/>
                </svg>
              </button>
            {:else}
              <button class="icon-btn" onclick={(e) => { e.stopPropagation(); connectSsh(profile); }} title="Connect">
                <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M5 12h14M12 5l7 7-7 7"/>
                </svg>
              </button>
            {/if}
            <button class="icon-btn" onclick={(e) => { e.stopPropagation(); startEdit(profile); }} title="Edit">
              <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/>
                <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/>
              </svg>
            </button>
            <button class="icon-btn" onclick={(e) => { e.stopPropagation(); openSshExplorer(profile); }} title="Open in Sidebar Explorer">
              <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
              </svg>
            </button>
            <button class="icon-btn danger" onclick={(e) => { e.stopPropagation(); deleteProfile(profile.id); }} title="Delete">
              <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
              </svg>
            </button>
          </div>
        </div>
      {/if}
    {:else}
      {#if !showForm}
        <div class="empty-state">No SSH profiles configured.</div>
      {/if}
    {/each}
  </div>
</div>

<style>
  .ssh-container {
    display: flex;
    flex-direction: column;
    height: 100%;
    padding: 8px;
    font-size: var(--fs-11);
  }

  .ssh-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 8px;
    padding-bottom: 6px;
    border-bottom: 1px solid var(--border-subtle);
  }

  .ssh-header .title {
    font-weight: 700;
    color: var(--text-muted);
    letter-spacing: 0.8px;
    text-transform: uppercase;
    font-size: var(--fs-9-5);
  }

  .add-btn {
    background: transparent;
    border: 1px solid var(--accent-blue);
    color: var(--accent-blue);
    border-radius: 4px;
    padding: 3px 10px;
    cursor: pointer;
    font-size: var(--fs-10);
    font-weight: 600;
    transition: all 0.12s ease;
  }

  .add-btn:hover {
    background: var(--accent-blue);
    color: var(--bg-primary);
  }

  .ssh-form {
    display: flex;
    flex-direction: column;
    gap: 6px;
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: 8px;
    padding: 10px;
    margin-bottom: 8px;
    box-shadow: 0 2px 8px rgba(0,0,0,0.08);
  }

  .ssh-form input {
    background: var(--bg-primary);
    color: var(--text-primary);
    border: 1px solid var(--border-subtle);
    border-radius: 4px;
    padding: 5px 7px;
    font-size: var(--fs-11);
    transition: border-color 0.1s;
  }

  .ssh-form input:focus {
    outline: none;
    border-color: var(--accent-blue);
    box-shadow: 0 0 0 1px var(--accent-blue);
  }

  .ssh-form .row {
    display: flex;
    gap: 4px;
  }

  .save-btn {
    background: var(--accent-blue);
    color: var(--bg-primary);
    border: none;
    border-radius: 5px;
    padding: 6px 12px;
    font-weight: 600;
    cursor: pointer;
    font-size: var(--fs-10);
    transition: filter 0.1s;
  }

  .save-btn:hover {
    filter: brightness(1.12);
  }

  .profiles-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
    overflow-y: auto;
    flex: 1;
  }

  .profile-card {
    display: flex;
    align-items: center;
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: 8px;
    padding: 7px 8px;
    transition: all 0.12s ease;
    position: relative;
  }

  .profile-card:hover {
    border-color: var(--border-primary);
    background: var(--bg-hover);
    box-shadow: 0 1px 4px rgba(0,0,0,0.06);
  }

  .profile-card.active {
    border-color: var(--accent-blue);
    background: color-mix(in srgb, var(--accent-blue) 4%, var(--bg-surface));
  }

  .profile-card.active::before {
    content: '';
    position: absolute;
    left: -1px;
    top: 4px;
    bottom: 4px;
    width: 2px;
    background: var(--accent-blue);
    border-radius: 0 2px 2px 0;
  }

  .profile-actions {
    display: flex;
    align-items: center;
    gap: 1px;
    margin-left: 6px;
    flex-shrink: 0;
  }

  .icon-btn {
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    padding: 4px;
    display: flex;
    align-items: center;
    border-radius: 4px;
    transition: all 0.12s ease;
    position: relative;
  }

  .icon-btn:hover {
    color: var(--text-primary);
    background: var(--bg-hover);
    transform: scale(1.05);
  }

  .icon-btn.danger:hover {
    color: var(--accent-red);
    background: color-mix(in srgb, var(--accent-red) 10%, transparent);
  }

  .profile-info {
    flex: 1;
    cursor: pointer;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .profile-info .name {
    font-weight: 600;
    color: var(--text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .profile-info .detail {
    font-size: var(--fs-9-5);
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .empty-state {
    color: var(--text-muted);
    text-align: center;
    padding: 20px 8px;
    font-size: var(--fs-10);
  }
</style>
