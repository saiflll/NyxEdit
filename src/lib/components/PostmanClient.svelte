<script lang="ts">
  import { addToast } from "$lib/stores.svelte";
  import { loadNyxConfig, saveNyxConfig } from "$lib/nyxConfig";
  import { currentDir } from "$lib/stores.svelte";
  import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
  import { invoke } from "@tauri-apps/api/core";
  import { save as saveDialog } from "@tauri-apps/plugin-dialog";

  // ── Postman v2.1 compatible types ────────────────────────────────────────
  type RequestHeader = { key: string; value: string; disabled?: boolean };
  type RequestBody = { mode: "raw" | "urlencoded" | "none"; raw?: string };
  type RequestAuth = {
    type: "none" | "bearer" | "basic" | "apikey";
    bearer?: { token: string }[];
    basic?: { username: string; password: string }[];
    apikey?: { key: string; value: string; in: "header" | "query" }[];
  };
  type CollectionRequest = {
    id: string;
    name: string;
    request: {
      method: string;
      url: string;
      header: RequestHeader[];
      body: RequestBody;
      auth: RequestAuth;
    };
    response?: {
      status: number;
      body: string;
      headers: Record<string, string>;
      time?: number;
      size?: number;
    };
  };
  type Collection = {
    version: string;
    info: { name: string; schema: string };
    item: CollectionRequest[];
    variable?: { key: string; value: string }[];
  };

  // ── Props ─────────────────────────────────────────────────────────────────
  let {
    isSidebar = true,
    activeRequestId = null as string | null,
    onOpenRequest = (_id: string) => {},
  } = $props();

  // ── State ─────────────────────────────────────────────────────────────────
  let collections = $state<CollectionRequest[]>([]);
  let activeId = $state<string | null>(null);
  let activeReq = $derived(collections.find((r) => r.id === activeId) ?? null);
  let reqTab = $state<"params" | "auth" | "headers" | "body">("auth");
  let respTab = $state<"body" | "headers">("body");
  let isLoading = $state(false);
  let envVars = $state<{ key: string; value: string }[]>([]);
  let envOpen = $state(false);

  // Editing local copy of request
  let editMethod = $state("GET");
  let editUrl = $state("https://jsonplaceholder.typicode.com/todos/1");
  let editHeaders = $state<RequestHeader[]>([]);
  let editBody = $state("");
  let editAuthType = $state<"none" | "bearer" | "basic" | "apikey">("none");
  let editAuthToken = $state("");
  let editAuthUser = $state("");
  let editAuthPass = $state("");
  let editAuthKey = $state("");
  let editAuthKeyVal = $state("");
  let editAuthKeyIn = $state<"header" | "query">("header");

  let responseStatus = $state<number | null>(null);
  let responseBody = $state("");
  let responseHeaders = $state<Record<string, string>>({});
  let responseTime = $state<number | null>(null);
  let responseSize = $state<number | null>(null);
  let isDirty = $state(false);

  $effect(() => {
    if (!isSidebar && activeRequestId) {
      activeId = activeRequestId;
    }
  });

  let workspaceDir = $state("");
  let isInitialLoad = $state(true);

  const METHODS = ["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS", "HEAD"];
  const METHOD_COLORS: Record<string, string> = {
    GET: "var(--accent-green)",
    POST: "#f59e0b",
    PUT: "#8b5cf6",
    PATCH: "#06b6d4",
    DELETE: "var(--accent-red)",
    OPTIONS: "#6b7280",
    HEAD: "#6b7280",
  };

  $effect(() => {
    const unsub = currentDir.subscribe(async (val) => {
      workspaceDir = val;
      isInitialLoad = true;
      await loadCollections();
    });
    return unsub;
  });

  // Sync active request to editor fields
  $effect(() => {
    if (activeReq) {
      editMethod = activeReq.request.method;
      editUrl = activeReq.request.url;
      editHeaders = [...activeReq.request.header];
      editBody = activeReq.request.body.raw ?? "";
      editAuthType = activeReq.request.auth.type;
      editAuthToken = activeReq.request.auth.bearer?.[0]?.token ?? "";
      editAuthUser = activeReq.request.auth.basic?.[0]?.username ?? "";
      editAuthPass = activeReq.request.auth.basic?.[0]?.password ?? "";
      editAuthKey = activeReq.request.auth.apikey?.[0]?.key ?? "";
      editAuthKeyVal = activeReq.request.auth.apikey?.[0]?.value ?? "";
      editAuthKeyIn = activeReq.request.auth.apikey?.[0]?.in ?? "header";
      responseStatus = activeReq.response?.status ?? null;
      responseBody = activeReq.response?.body ?? "";
      responseHeaders = activeReq.response?.headers ?? {};
      responseTime = activeReq.response?.time ?? null;
      responseSize = activeReq.response?.size ?? null;
      isDirty = false;
    }
  });

  async function loadCollections() {
    if (!workspaceDir) return;
    try {
      const col = await loadNyxConfig<Collection>("rest_api.json", {
        version: "2.1",
        info: {
          name: "NyxEdit Collection",
          schema:
            "https://schema.getpostman.com/json/collection/v2.1.0/collection.json",
        },
        item: [],
        variable: [],
      });
      collections = col.item ?? [];
      envVars = col.variable ?? [];
      if (collections.length > 0 && !activeId) {
        activeId = collections[0].id;
      }
    } catch (e) {
      console.error("Load collections failed:", e);
    } finally {
      isInitialLoad = false;
    }
  }

  async function saveCollections() {
    if (!workspaceDir || isInitialLoad) return;
    const col: Collection = {
      version: "2.1",
      info: {
        name: "NyxEdit Collection",
        schema:
          "https://schema.getpostman.com/json/collection/v2.1.0/collection.json",
      },
      item: $state.snapshot(collections) as CollectionRequest[],
      variable: $state.snapshot(envVars) as { key: string; value: string }[],
    };
    await saveNyxConfig("rest_api.json", col);
  }

  function buildNewRequest(): CollectionRequest {
    return {
      id: crypto.randomUUID(),
      name: "New Request",
      request: {
        method: "GET",
        url: "",
        header: [],
        body: { mode: "raw", raw: "" },
        auth: { type: "none" },
      },
    };
  }

  async function addRequest() {
    const req = buildNewRequest();
    collections = [...collections, req];
    activeId = req.id;
    isDirty = false;
    await saveCollections();
  }

  async function deleteRequest(id: string) {
    collections = collections.filter((r) => r.id !== id);
    if (activeId === id) {
      activeId = collections[0]?.id ?? null;
    }
    await saveCollections();
  }

  async function duplicateRequest(req: CollectionRequest) {
    const dup: CollectionRequest = JSON.parse(JSON.stringify(req));
    dup.id = crypto.randomUUID();
    dup.name = req.name + " (copy)";
    const idx = collections.findIndex((r) => r.id === req.id);
    collections = [
      ...collections.slice(0, idx + 1),
      dup,
      ...collections.slice(idx + 1),
    ];
    activeId = dup.id;
    await saveCollections();
  }

  function resolveEnvVars(str: string): string {
    return str.replace(/\{\{(\w+)\}\}/g, (_, key) => {
      const v = envVars.find((e) => e.key === key);
      return v ? v.value : `{{${key}}}`;
    });
  }

  async function applyChanges() {
    if (!activeId) return;
    const idx = collections.findIndex((r) => r.id === activeId);
    if (idx < 0) return;
    const auth: RequestAuth = { type: editAuthType };
    if (editAuthType === "bearer") auth.bearer = [{ token: editAuthToken }];
    if (editAuthType === "basic")
      auth.basic = [{ username: editAuthUser, password: editAuthPass }];
    if (editAuthType === "apikey")
      auth.apikey = [
        { key: editAuthKey, value: editAuthKeyVal, in: editAuthKeyIn },
      ];
    collections[idx] = {
      ...collections[idx],
      request: {
        method: editMethod,
        url: editUrl,
        header: [...editHeaders],
        body: { mode: editBody ? "raw" : "none", raw: editBody },
        auth,
      },
    };
    collections = [...collections];
    isDirty = false;
    await saveCollections();
    addToast("Saved", "success");
  }

  async function sendRequest() {
    if (!editUrl) return;
    isLoading = true;
    responseBody = "";
    responseStatus = null;
    responseHeaders = {};
    const t0 = Date.now();
    const resolvedUrl = resolveEnvVars(editUrl);
    try {
      const headersObj: Record<string, string> = {};
      for (const h of editHeaders) {
        if (h.key.trim() && !h.disabled)
          headersObj[h.key.trim()] = resolveEnvVars(h.value.trim());
      }
      if (!headersObj["Content-Type"] && editBody)
        headersObj["Content-Type"] = "application/json";
      // Auth
      if (editAuthType === "bearer" && editAuthToken) {
        headersObj["Authorization"] = `Bearer ${resolveEnvVars(editAuthToken)}`;
      } else if (editAuthType === "basic" && editAuthUser) {
        headersObj["Authorization"] =
          "Basic " + btoa(`${editAuthUser}:${editAuthPass}`);
      } else if (
        editAuthType === "apikey" &&
        editAuthKey &&
        editAuthKeyIn === "header"
      ) {
        headersObj[editAuthKey] = resolveEnvVars(editAuthKeyVal);
      }
      let reqUrl = resolvedUrl;
      if (
        editAuthType === "apikey" &&
        editAuthKey &&
        editAuthKeyIn === "query"
      ) {
        reqUrl +=
          (reqUrl.includes("?") ? "&" : "?") +
          `${encodeURIComponent(editAuthKey)}=${encodeURIComponent(editAuthKeyVal)}`;
      }
      const options: RequestInit = { method: editMethod, headers: headersObj };
      if (!["GET", "HEAD"].includes(editMethod) && editBody)
        options.body = resolveEnvVars(editBody);
      const res = await tauriFetch(reqUrl, options);
      responseStatus = res.status;
      // Capture response headers
      res.headers.forEach((v, k) => {
        responseHeaders[k] = v;
      });
      const text = await res.text();
      let pretty = text;
      try {
        pretty = JSON.stringify(JSON.parse(text), null, 2);
      } catch {}
      responseBody = pretty;
      const elapsed = Date.now() - t0;
      responseTime = elapsed;
      responseSize = new TextEncoder().encode(text).length;
      // Save response to collection item
      if (activeId) {
        const idx = collections.findIndex((r) => r.id === activeId);
        if (idx >= 0) {
          collections[idx].response = {
            status: res.status,
            body: pretty,
            headers: { ...responseHeaders },
            time: elapsed,
            size: responseSize,
          };
          collections = [...collections];
          await saveCollections();
        }
      }
      addToast(
        `${editMethod} ${res.status}`,
        res.status < 400 ? "success" : "error",
      );
    } catch (err: any) {
      responseBody = `Error: ${err.message}`;
      addToast("Request failed", "error");
    } finally {
      isLoading = false;
    }
  }

  async function exportPostman() {
    const col: Collection = {
      version: "2.1",
      info: {
        name: "NyxEdit Collection",
        schema:
          "https://schema.getpostman.com/json/collection/v2.1.0/collection.json",
      },
      item: $state.snapshot(collections) as CollectionRequest[],
      variable: $state.snapshot(envVars) as { key: string; value: string }[],
    };
    const json = JSON.stringify(col, null, 2);
    try {
      const path = await saveDialog({
        defaultPath: "collection.postman_collection.json",
        filters: [{ name: "Postman Collection", extensions: ["json"] }],
      });
      if (path) {
        await invoke("fs_write_file", { path, content: json });
        addToast("Exported to " + path, "success");
      }
    } catch {
      // Fallback: download via blob
      const blob = new Blob([json], { type: "application/json" });
      const a = document.createElement("a");
      a.href = URL.createObjectURL(blob);
      a.download = "collection.postman_collection.json";
      a.click();
    }
  }

  function markDirty() {
    isDirty = true;
  }
  function addHeader() {
    editHeaders = [...editHeaders, { key: "", value: "" }];
    markDirty();
  }
  function removeHeader(i: number) {
    editHeaders = editHeaders.filter((_, j) => j !== i);
    markDirty();
  }
  function addEnvVar() {
    envVars = [...envVars, { key: "", value: "" }];
  }
  function removeEnvVar(i: number) {
    envVars = envVars.filter((_, j) => j !== i);
  }

  function statusColor(s: number): string {
    if (s < 200) return "var(--text-muted)";
    if (s < 300) return "var(--accent-green)";
    if (s < 400) return "var(--accent-yellow,#f59e0b)";
    return "var(--accent-red)";
  }

  let editingName = $state<string | null>(null);
  let editingNameVal = $state("");
  function startRenaming(req: CollectionRequest) {
    editingName = req.id;
    editingNameVal = req.name;
  }
  async function finishRename(req: CollectionRequest) {
    const idx = collections.findIndex((r) => r.id === req.id);
    if (idx >= 0) {
      collections[idx].name = editingNameVal;
      collections = [...collections];
    }
    editingName = null;
    await saveCollections();
  }
</script>

<div class="pm-root">
  {#if isSidebar}
    <!-- ── Sidebar Mode: Collection Only ──────────── -->
    <div class="pm-sidebar-collection">
      <div class="pm-sidebar-header">
        <span class="pm-sidebar-title">COLLECTION</span>
        <div class="pm-sidebar-actions">
          <button class="pm-icon-btn" onclick={addRequest} title="New Request">
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" /></svg>
          </button>
          <button class="pm-icon-btn" onclick={exportPostman} title="Export to Postman">
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" /><polyline points="7 10 12 15 17 10" /><line x1="12" y1="15" x2="12" y2="3" /></svg>
          </button>
          <button class="pm-icon-btn" onclick={() => (envOpen = !envOpen)} title="Environment Variables">
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z" /></svg>
          </button>
        </div>
      </div>

      {#if envOpen}
        <div class="pm-env-panel">
          <div class="pm-env-title">Environment Variables</div>
          {#each envVars as ev, i}
            <div class="pm-env-row">
              <input class="pm-env-input" placeholder="KEY" bind:value={ev.key} />
              <input class="pm-env-input" placeholder="VALUE" bind:value={ev.value} />
              <button class="pm-env-del" onclick={() => removeEnvVar(i)}>×</button>
            </div>
          {/each}
          <button class="pm-env-add" onclick={addEnvVar}>+ Add Variable</button>
        </div>
      {/if}

      <div class="pm-req-list">
        {#if collections.length === 0}
          <div class="pm-empty-list">No requests yet.<br />Click + to add one.</div>
        {:else}
          {#each collections as req}
            <div
              class="pm-req-item"
              class:active={activeId === req.id}
              onclick={() => onOpenRequest(req.id)}
              role="button"
            >
              <span class="pm-method-badge" style="color: {METHOD_COLORS[req.request.method] ?? 'var(--text-muted)'}">
                {req.request.method.slice(0, 3)}
              </span>
              {#if editingName === req.id}
                <input class="pm-rename-input" bind:value={editingNameVal}
                  onkeydown={(e) => { if (e.key === "Enter") finishRename(req); if (e.key === "Escape") editingName = null; }}
                  onblur={() => finishRename(req)} onclick={(e) => e.stopPropagation()}
                />
              {:else}
                <span class="pm-req-name" ondblclick={(e) => { e.stopPropagation(); startRenaming(req); }}>{req.name}</span>
              {/if}
              <div class="pm-req-actions">
                <button class="pm-icon-btn small" onclick={(e) => { e.stopPropagation(); duplicateRequest(req); }} title="Duplicate">
                  <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="9" y="9" width="13" height="13" rx="2" /><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" /></svg>
                </button>
                <button class="pm-icon-btn small danger" onclick={(e) => { e.stopPropagation(); deleteRequest(req.id); }} title="Delete">
                  <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="3 6 5 6 21 6" /><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" /></svg>
                </button>
              </div>
            </div>
          {/each}
        {/if}
      </div>
    </div>
  {:else}
    <!-- ── Tab Mode: Full Editor ──────────────────── -->
    {#if activeReq}
      <div class="pm-main">
        <div class="pm-request-line">
          <select bind:value={editMethod} class="pm-method-select" style="color: {METHOD_COLORS[editMethod] ?? 'var(--text-primary)'}" onchange={markDirty}>
            {#each METHODS as m}
              <option value={m} style="color: {METHOD_COLORS[m] ?? 'inherit'}">{m}</option>
            {/each}
          </select>
          <input class="pm-url-input" bind:value={editUrl} placeholder="https://api.example.com/endpoint" oninput={markDirty} />
          <button class="pm-send-btn" onclick={sendRequest} disabled={isLoading}>
            {#if isLoading}
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="spin"><path d="M21 12a9 9 0 1 1-6.219-8.56" /></svg>
            {:else}
              Send
            {/if}
          </button>
          {#if isDirty}
            <button class="pm-save-btn" onclick={applyChanges}>Save</button>
          {/if}
        </div>

        <div class="pm-tabs">
          {#each ["auth", "headers", "body", "params"] as const as tab}
            <button class="pm-tab" class:active={reqTab === tab} onclick={() => (reqTab = tab)}>
              {tab.charAt(0).toUpperCase() + tab.slice(1)}
              {#if tab === "headers" && editHeaders.length > 0}
                <span class="pm-tab-badge">{editHeaders.filter((h) => h.key).length}</span>
              {/if}
            </button>
          {/each}
        </div>

        <div class="pm-tab-content">
          {#if reqTab === "auth"}
            <div class="pm-auth-section">
              <select class="pm-select" bind:value={editAuthType} onchange={markDirty}>
                <option value="none">No Auth</option>
                <option value="bearer">Bearer Token</option>
                <option value="basic">Basic Auth</option>
                <option value="apikey">API Key</option>
              </select>
              {#if editAuthType === "bearer"}
                <div class="pm-field-row">
                  <label class="pm-field-label">Token</label>
                  <input type="password" class="pm-field-input" bind:value={editAuthToken} oninput={markDirty} />
                </div>
              {:else if editAuthType === "basic"}
                <div class="pm-field-row">
                  <label class="pm-field-label">Username</label>
                  <input class="pm-field-input" bind:value={editAuthUser} oninput={markDirty} />
                </div>
                <div class="pm-field-row">
                  <label class="pm-field-label">Password</label>
                  <input type="password" class="pm-field-input" bind:value={editAuthPass} oninput={markDirty} />
                </div>
              {:else if editAuthType === "apikey"}
                <div class="pm-field-row">
                  <label class="pm-field-label">Key</label>
                  <input class="pm-field-input" bind:value={editAuthKey} oninput={markDirty} />
                </div>
                <div class="pm-field-row">
                  <label class="pm-field-label">Value</label>
                  <input class="pm-field-input" bind:value={editAuthKeyVal} oninput={markDirty} />
                </div>
                <div class="pm-field-row">
                  <label class="pm-field-label">Add to</label>
                  <select class="pm-select" bind:value={editAuthKeyIn} onchange={markDirty}>
                    <option value="header">Header</option>
                    <option value="query">Query Param</option>
                  </select>
                </div>
              {/if}
            </div>
          {:else if reqTab === "headers"}
            <div class="pm-headers-section">
              <div class="pm-headers-list">
                {#each editHeaders as h, i}
                  <div class="pm-header-row">
                    <input class="pm-header-key" placeholder="Header-Name" bind:value={h.key} oninput={markDirty} />
                    <input class="pm-header-val" placeholder="value or {'{{ variable }}'}" bind:value={h.value} oninput={markDirty} />
                    <button class="pm-icon-btn small danger" onclick={() => removeHeader(i)}>×</button>
                  </div>
                {/each}
              </div>
              <button class="pm-add-header-btn" onclick={addHeader}>+ Add Header</button>
            </div>
          {:else if reqTab === "body"}
            <div class="pm-body-section">
              {#if !["GET", "HEAD"].includes(editMethod)}
                <textarea class="pm-body-textarea" bind:value={editBody} placeholder={'{\n  "key": "value"\n}'} oninput={markDirty}></textarea>
              {:else}
                <div class="pm-body-note">Body not applicable for {editMethod} requests</div>
              {/if}
            </div>
          {:else if reqTab === "params"}
            <div class="pm-params-note">
              <p>Add query parameters to the URL directly using <code>?key=value&amp;key2=value2</code></p>
              <p>Or use environment variables: <code>{"{{BASE_URL}}"}/endpoint</code></p>
            </div>
          {/if}
        </div>

        <div class="pm-response-panel">
          <div class="pm-response-header">
            <span class="pm-response-title">Response</span>
            <div class="pm-response-meta">
              {#if responseStatus !== null}
                <span class="pm-status-badge" style="color: {statusColor(responseStatus)}; border-color: {statusColor(responseStatus)}">{responseStatus}</span>
              {/if}
              {#if responseTime !== null}
                <span class="pm-meta-chip">{responseTime}ms</span>
              {/if}
              {#if responseSize !== null}
                <span class="pm-meta-chip">{responseSize < 1024 ? responseSize + "B" : (responseSize / 1024).toFixed(1) + "KB"}</span>
              {/if}
            </div>
            <div class="pm-resp-tabs">
              <button class="pm-tab small" class:active={respTab === "body"} onclick={() => (respTab = "body")}>Body</button>
              <button class="pm-tab small" class:active={respTab === "headers"} onclick={() => (respTab = "headers")}>Headers</button>
            </div>
          </div>
          <div class="pm-response-body">
            {#if isLoading}
              <div class="pm-loading">
                <div class="pm-spinner"></div>
                <span>Waiting for response…</span>
              </div>
            {:else if respTab === "body"}
              {#if responseBody}
                <pre class="pm-resp-pre">{responseBody}</pre>
              {:else}
                <div class="pm-resp-empty">Send a request to see the response</div>
              {/if}
            {:else if Object.keys(responseHeaders).length > 0}
              {#each Object.entries(responseHeaders) as [k, v]}
                <div class="pm-resp-header-row">
                  <span class="pm-resp-hk">{k}</span>
                  <span class="pm-resp-hv">{v}</span>
                </div>
              {/each}
            {:else}
              <div class="pm-resp-empty">No headers</div>
            {/if}
          </div>
        </div>
      </div>
    {:else}
      <div class="pm-no-selection">
        <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5 2z" /><polyline points="14 2 14 8 20 8" /></svg>
        <p>Select a request or create a new one</p>
        <button class="pm-new-btn" onclick={addRequest}>+ New Request</button>
      </div>
    {/if}
  {/if}
</div>

<style>
  .pm-root {
    display: flex;
    height: 100%;
    font-size: var(--fs-11);
    background: var(--glass-bg, var(--bg-secondary));
    backdrop-filter: blur(var(--glass-blur, 12px));
    -webkit-backdrop-filter: blur(var(--glass-blur, 12px));
    overflow: hidden;
  }

  /* ── Sidebar Mode ────────────────── */
  .pm-sidebar-collection {
    display: flex;
    flex-direction: column;
    height: 100%;
  }
  .pm-sidebar-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 8px;
    border-bottom: 1px solid var(--border-subtle);
    flex-shrink: 0;
  }
  .pm-sidebar-title {
    font-size: var(--fs-9);
    font-weight: 700;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }
  .pm-sidebar-actions {
    display: flex;
    gap: 2px;
  }
  .pm-icon-btn {
    background: none;
    border: none;
    color: var(--text-muted);
    padding: 3px 4px;
    border-radius: 3px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.1s;
  }
  .pm-icon-btn:hover {
    color: var(--text-primary);
    background: var(--bg-hover);
  }
  .pm-icon-btn.small {
    padding: 2px 3px;
  }
  .pm-icon-btn.danger:hover {
    color: var(--accent-red);
  }

  .pm-env-panel {
    padding: 8px;
    border-bottom: 1px solid var(--border-subtle);
    background: var(--bg-surface);
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .pm-env-title {
    font-size: var(--fs-9);
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    margin-bottom: 4px;
  }
  .pm-env-row {
    display: flex;
    gap: 4px;
    align-items: center;
  }
  .pm-env-input {
    flex: 1;
    background: var(--bg-primary);
    border: 1px solid var(--border-subtle);
    border-radius: 3px;
    padding: 2px 5px;
    font-size: var(--fs-10);
    color: var(--text-primary);
    outline: none;
    min-width: 0;
  }
  .pm-env-input:focus {
    border-color: var(--accent-blue);
  }
  .pm-env-del {
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    font-size: var(--fs-12);
  }
  .pm-env-del:hover {
    color: var(--accent-red);
  }
  .pm-env-add {
    background: none;
    border: 1px dashed var(--border-subtle);
    border-radius: 4px;
    color: var(--text-muted);
    font-size: var(--fs-10);
    padding: 3px;
    cursor: pointer;
  }
  .pm-env-add:hover {
    color: var(--text-primary);
    border-color: var(--text-muted);
  }

  .pm-req-list {
    flex: 1;
    overflow-y: auto;
    padding: 4px 0;
  }
  .pm-req-item {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 5px 8px;
    cursor: pointer;
    border-radius: 0;
    transition: background 0.1s;
    border-bottom: 1px solid var(--border-subtle);
    position: relative;
  }
  .pm-req-item:hover {
    background: var(--bg-hover);
  }
  .pm-req-item.active {
    background: color-mix(in srgb, var(--accent-blue) 10%, transparent);
    border-left: 2px solid var(--accent-blue);
  }
  .pm-method-badge {
    font-size: var(--fs-9);
    font-weight: 700;
    font-family: monospace;
    flex-shrink: 0;
    width: 28px;
  }
  .pm-req-name {
    flex: 1;
    font-size: var(--fs-10);
    color: var(--text-primary);
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .pm-rename-input {
    flex: 1;
    background: var(--bg-surface);
    border: 1px solid var(--accent-blue);
    border-radius: 3px;
    padding: 1px 4px;
    font-size: var(--fs-10);
    color: var(--text-primary);
    outline: none;
    min-width: 0;
  }
  .pm-req-actions {
    display: flex;
    gap: 1px;
    opacity: 0;
    transition: opacity 0.1s;
  }
  .pm-req-item:hover .pm-req-actions {
    opacity: 1;
  }
  .pm-empty-list {
    padding: 20px 10px;
    text-align: center;
    color: var(--text-muted);
    font-size: var(--fs-10);
    line-height: 1.6;
  }

  /* ── Main panel ───────────────────── */
  .pm-main {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .pm-request-line {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 10px;
    border-bottom: 1px solid var(--border-subtle);
    flex-shrink: 0;
    background: var(--bg-secondary);
  }
  .pm-method-select {
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: 4px;
    padding: 4px 6px;
    font-weight: 700;
    cursor: pointer;
    outline: none;
    font-size: var(--fs-10);
  }
  .pm-url-input {
    flex: 1;
    background: var(--bg-surface);
    color: var(--text-primary);
    border: 1px solid var(--border-subtle);
    border-radius: 4px;
    padding: 4px 8px;
    font-size: var(--fs-11);
    outline: none;
  }
  .pm-url-input:focus {
    border-color: var(--accent-blue);
  }
  .pm-send-btn {
    background: var(--accent-blue);
    color: var(--bg-primary);
    border: none;
    border-radius: 4px;
    padding: 5px 14px;
    font-weight: 700;
    cursor: pointer;
    font-size: var(--fs-11);
    transition: filter 0.1s;
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .pm-send-btn:hover:not(:disabled) {
    filter: brightness(1.15);
  }
  .pm-send-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
  .pm-save-btn {
    background: color-mix(in srgb, var(--accent-green) 15%, transparent);
    color: var(--accent-green);
    border: 1px solid var(--accent-green);
    border-radius: 4px;
    padding: 4px 10px;
    font-weight: 600;
    cursor: pointer;
    font-size: var(--fs-10);
    transition: all 0.12s;
  }
  .pm-save-btn:hover {
    background: var(--accent-green);
    color: var(--bg-primary);
  }
  .spin {
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .pm-tabs {
    display: flex;
    padding: 0 10px;
    border-bottom: 1px solid var(--border-subtle);
    flex-shrink: 0;
    background: var(--bg-secondary);
  }
  .pm-tab {
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    padding: 5px 10px;
    color: var(--text-muted);
    font-size: var(--fs-10);
    font-weight: 500;
    cursor: pointer;
    transition: all 0.1s;
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .pm-tab.active {
    color: var(--accent-blue);
    border-bottom-color: var(--accent-blue);
  }
  .pm-tab:hover:not(.active) {
    color: var(--text-primary);
  }
  .pm-tab.small {
    padding: 3px 7px;
    font-size: var(--fs-9);
  }
  .pm-tab-badge {
    background: var(--accent-blue);
    color: var(--bg-primary);
    border-radius: 3px;
    padding: 0 4px;
    font-size: var(--fs-9);
    font-weight: 700;
  }

  .pm-tab-content {
    padding: 10px;
    overflow-y: auto;
    max-height: 160px;
    flex-shrink: 0;
    border-bottom: 1px solid var(--border-subtle);
  }

  .pm-auth-section,
  .pm-headers-section {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .pm-select {
    background: var(--bg-surface);
    color: var(--text-primary);
    border: 1px solid var(--border-subtle);
    border-radius: 4px;
    padding: 4px 6px;
    font-size: var(--fs-10);
    cursor: pointer;
    outline: none;
  }
  .pm-field-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .pm-field-label {
    font-size: var(--fs-9);
    color: var(--text-muted);
    width: 70px;
    flex-shrink: 0;
    font-weight: 600;
  }
  .pm-field-input {
    flex: 1;
    background: var(--bg-surface);
    color: var(--text-primary);
    border: 1px solid var(--border-subtle);
    border-radius: 4px;
    padding: 4px 6px;
    font-size: var(--fs-10);
    outline: none;
  }
  .pm-field-input:focus {
    border-color: var(--accent-blue);
  }

  .pm-headers-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .pm-header-row {
    display: flex;
    gap: 4px;
    align-items: center;
  }
  .pm-header-key {
    width: 140px;
    background: var(--bg-surface);
    color: var(--text-primary);
    border: 1px solid var(--border-subtle);
    border-radius: 4px;
    padding: 3px 6px;
    font-size: var(--fs-10);
    outline: none;
  }
  .pm-header-val {
    flex: 1;
    background: var(--bg-surface);
    color: var(--text-primary);
    border: 1px solid var(--border-subtle);
    border-radius: 4px;
    padding: 3px 6px;
    font-size: var(--fs-10);
    outline: none;
  }
  .pm-header-key:focus,
  .pm-header-val:focus {
    border-color: var(--accent-blue);
  }
  .pm-add-header-btn {
    background: none;
    border: 1px dashed var(--border-subtle);
    border-radius: 4px;
    color: var(--text-muted);
    font-size: var(--fs-10);
    padding: 4px;
    cursor: pointer;
    margin-top: 2px;
  }
  .pm-add-header-btn:hover {
    color: var(--text-primary);
    border-color: var(--text-muted);
  }

  .pm-body-section {
    height: 100%;
  }
  .pm-body-textarea {
    width: 100%;
    height: 110px;
    background: var(--bg-surface);
    color: var(--text-primary);
    border: 1px solid var(--border-subtle);
    border-radius: 4px;
    padding: 6px;
    font-family: monospace;
    font-size: var(--fs-10);
    resize: none;
    outline: none;
    box-sizing: border-box;
  }
  .pm-body-textarea:focus {
    border-color: var(--accent-blue);
  }
  .pm-body-note,
  .pm-params-note {
    color: var(--text-muted);
    font-size: var(--fs-10);
    line-height: 1.6;
  }
  .pm-params-note code {
    background: var(--bg-surface);
    padding: 1px 4px;
    border-radius: 3px;
    font-family: monospace;
  }

  /* ── Response ─────────────────────── */
  .pm-response-panel {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .pm-response-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 10px;
    background: var(--bg-secondary);
    border-bottom: 1px solid var(--border-subtle);
    flex-shrink: 0;
  }
  .pm-response-title {
    font-size: var(--fs-10);
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
  }
  .pm-response-meta {
    display: flex;
    gap: 4px;
    align-items: center;
    flex: 1;
  }
  .pm-status-badge {
    font-size: var(--fs-10);
    font-weight: 700;
    padding: 1px 6px;
    border-radius: 4px;
    border: 1px solid;
    font-family: monospace;
  }
  .pm-meta-chip {
    font-size: var(--fs-9);
    color: var(--text-muted);
    background: var(--bg-surface);
    padding: 1px 5px;
    border-radius: 3px;
  }
  .pm-resp-tabs {
    display: flex;
    margin-left: auto;
  }
  .pm-response-body {
    flex: 1;
    overflow-y: auto;
    padding: 6px;
    font-family: monospace;
    font-size: var(--fs-10);
  }
  .pm-resp-pre {
    margin: 0;
    white-space: pre-wrap;
    word-break: break-all;
    color: var(--text-primary);
  }
  .pm-resp-empty {
    color: var(--text-muted);
    text-align: center;
    padding: 20px 0;
    font-size: var(--fs-11);
    font-family: initial;
  }
  .pm-loading {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 20px;
    color: var(--text-muted);
    font-family: initial;
  }
  .pm-spinner {
    width: 16px;
    height: 16px;
    border: 2px solid var(--border-subtle);
    border-top-color: var(--accent-blue);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }
  .pm-resp-header-row {
    display: flex;
    gap: 10px;
    padding: 3px 0;
    border-bottom: 1px solid var(--border-subtle);
  }
  .pm-resp-hk {
    color: var(--accent-blue);
    font-weight: 600;
    min-width: 140px;
  }
  .pm-resp-hv {
    color: var(--text-secondary);
    word-break: break-all;
  }

  .pm-no-selection {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    color: var(--text-muted);
  }
  .pm-no-selection p {
    font-size: var(--fs-11);
  }
  .pm-new-btn {
    background: var(--accent-blue);
    color: var(--bg-primary);
    border: none;
    border-radius: 4px;
    padding: 6px 16px;
    font-weight: 600;
    cursor: pointer;
    font-size: var(--fs-11);
  }
  .pm-new-btn:hover {
    filter: brightness(1.1);
  }
</style>
