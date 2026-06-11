<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { addToast } from "$lib/stores.svelte";

  type DbConnectionInfo = {
    id: string;
    db_type: "postgres" | "mysql" | "sqlite" | "mongodb";
    host: string;
    port: number;
    user: string;
    password?: string;
    database?: string;
    label: string;
  };

  type QueryResult = {
    columns: string[];
    rows: Array<Array<any>>;
    row_count: number;
    duration_ms: number;
  };

  type DbTableInfo = {
    name: string;
    schema?: string;
  };

  type ColumnInfo = {
    name: string;
    data_type: string;
    nullable: boolean;
    is_pk: boolean;
  };

  type TreeNode = {
    conn: DbConnectionInfo;
    expanded: boolean;
    loading: boolean;
    databases: string[];
  };

  let {
    isSidebar = true,
    activeConnectionId = null as string | null,
    onOpenQuery = (_connId: string, _label: string) => {},
  } = $props();

  let connections = $state<DbConnectionInfo[]>([]);
  let loading = $state(false);
  let showNewForm = $state(false);

  let formType = $state<DbConnectionInfo["db_type"]>("postgres");
  let formHost = $state("localhost");
  let formPort = $state(5432);
  let formUser = $state("root");
  let formPass = $state("");
  let formDb = $state("");
  let formLabel = $state("");
  let formBusy = $state(false);

  let treeNodes = $state<Map<string, TreeNode>>(new Map());

  let expandedDbs = $state<Map<string, string[]>>(new Map());
  let tableList = $state<Map<string, DbTableInfo[]>>(new Map());
  let columnList = $state<Map<string, ColumnInfo[]>>(new Map());
  let tableExpanded = $state<Map<string, boolean>>(new Map());
  let columnExpanded = $state<Map<string, boolean>>(new Map());

  // Tab mode state
  let querySql = $state("");
  let queryResult = $state<QueryResult | null>(null);
  let queryLoading = $state(false);
  let queryError = $state<string | null>(null);
  let tabConnections = $state<DbConnectionInfo[]>([]);
  let tabActiveConnId = $state<string | null>(null);

  async function loadConnections() {
    try {
      connections = await invoke<DbConnectionInfo[]>("db_list_connections");
      treeNodes = new Map();
      for (const c of connections) {
        treeNodes.set(c.id, { conn: c, expanded: false, loading: false, databases: [] });
      }
    } catch (err: any) {
      addToast(`Failed to list connections: ${err}`, "error");
    }
  }

  $effect(() => {
    if (isSidebar) loadConnections();
  });

  $effect(() => {
    if (!isSidebar && activeConnectionId) {
      tabActiveConnId = activeConnectionId;
    }
  });

  async function connect() {
    formBusy = true;
    try {
      const result = await invoke<DbConnectionInfo>("db_connect", {
        dbType: formType,
        host: formHost,
        port: formPort,
        user: formUser,
        password: formPass || null,
        database: formDb || null,
        label: formLabel || `${formType}-${formHost}`,
      });
      addToast(`Connected to ${result.label}`, "success");
      showNewForm = false;
      resetForm();
      await loadConnections();
    } catch (err: any) {
      addToast(`Connection failed: ${err}`, "error");
    } finally {
      formBusy = false;
    }
  }

  function resetForm() {
    formType = "postgres";
    formHost = "localhost";
    formPort = 5432;
    formUser = "root";
    formPass = "";
    formDb = "";
    formLabel = "";
  }

  function getDefaultPort(t: string): number {
    switch (t) {
      case "postgres": return 5432;
      case "mysql": return 3306;
      case "sqlite": return 0;
      case "mongodb": return 27017;
      default: return 5432;
    }
  }

  async function disconnect(id: string) {
    try {
      await invoke("db_disconnect", { connectionId: id });
      addToast("Disconnected", "info");
      treeNodes.delete(id);
      treeNodes = new Map(treeNodes);
      connections = connections.filter(c => c.id !== id);
    } catch (err: any) {
      addToast(`Disconnect failed: ${err}`, "error");
    }
  }

  async function toggleNode(connId: string) {
    const node = treeNodes.get(connId);
    if (!node) return;
    if (node.expanded) {
      node.expanded = false;
      treeNodes = new Map(treeNodes);
      return;
    }
    node.loading = true;
    node.expanded = true;
    treeNodes = new Map(treeNodes);
    try {
      const dbs = await invoke<string[]>("db_list_databases", { connectionId: connId });
      node.databases = dbs;
    } catch (err: any) {
      addToast(`List databases failed: ${err}`, "error");
      node.expanded = false;
    } finally {
      node.loading = false;
      treeNodes = new Map(treeNodes);
    }
  }

  async function toggleDbTables(connId: string, dbName: string) {
    const key = `${connId}:${dbName}`;
    const current = expandedDbs.get(key);
    if (current !== undefined) {
      expandedDbs.delete(key);
      expandedDbs = new Map(expandedDbs);
      return;
    }
    try {
      const tables = await invoke<DbTableInfo[]>("db_list_tables", { connectionId: connId, database: dbName });
      expandedDbs.set(key, tables.map(t => t.name));
      tableList = new Map(tableList.set(key, tables));
      expandedDbs = new Map(expandedDbs);
    } catch (err: any) {
      addToast(`List tables failed: ${err}`, "error");
    }
  }

  async function toggleTableColumns(connId: string, tableName: string) {
    const key = `${connId}:${tableName}`;
    const current = columnExpanded.get(key);
    if (current) {
      columnExpanded.delete(key);
      columnExpanded = new Map(columnExpanded);
      return;
    }
    try {
      const cols = await invoke<ColumnInfo[]>("db_get_columns", { connectionId: connId, table: tableName });
      columnList.set(key, cols);
      columnExpanded.set(key, true);
      columnExpanded = new Map(columnExpanded);
      columnList = new Map(columnList);
    } catch (err: any) {
      addToast(`Get columns failed: ${err}`, "error");
    }
  }

  function openQueryTab(connId: string, label: string) {
    onOpenQuery(connId, label);
  }

  // Tab mode query functions
  async function loadTabConnections() {
    try {
      tabConnections = await invoke<DbConnectionInfo[]>("db_list_connections");
    } catch (_) {}
  }

  $effect(() => {
    if (!isSidebar) loadTabConnections();
  });

  async function runQuery() {
    if (!tabActiveConnId || !querySql.trim()) return;
    queryLoading = true;
    queryResult = null;
    queryError = null;
    try {
      const result = await invoke<QueryResult>("db_query", {
        connectionId: tabActiveConnId,
        sql: querySql.trim(),
      });
      queryResult = result;
    } catch (err: any) {
      queryError = typeof err === "string" ? err : err.message || "Query failed";
    } finally {
      queryLoading = false;
    }
  }
</script>

{#if isSidebar}
  <div class="db-sidebar">
    <div class="db-sidebar-header">
      <span class="db-sidebar-title">Database Client</span>
      <button class="db-add-btn" onclick={() => { showNewForm = !showNewForm; if (!showNewForm) resetForm(); }} title="New Connection">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
      </button>
    </div>

    {#if showNewForm}
      <div class="db-new-form">
        <div class="db-form-row">
          <label class="db-label">Type</label>
          <select class="db-input" bind:value={formType} onchange={() => { formPort = getDefaultPort(formType); }}>
            <option value="postgres">PostgreSQL</option>
            <option value="mysql">MySQL</option>
            <option value="sqlite">SQLite</option>
            <option value="mongodb">MongoDB</option>
          </select>
        </div>
        {#if formType !== "sqlite"}
          <div class="db-form-row">
            <label class="db-label">Host</label>
            <input class="db-input" bind:value={formHost} placeholder="localhost" />
          </div>
          <div class="db-form-row">
            <label class="db-label">Port</label>
            <input class="db-input" type="number" bind:value={formPort} />
          </div>
        {/if}
        <div class="db-form-row">
          <label class="db-label">User</label>
          <input class="db-input" bind:value={formUser} placeholder={formType === "sqlite" ? "-" : "root"} />
        </div>
        <div class="db-form-row">
          <label class="db-label">Password</label>
          <input class="db-input" type="password" bind:value={formPass} placeholder="(optional)" />
        </div>
        <div class="db-form-row">
          <label class="db-label">Database</label>
          <input class="db-input" bind:value={formDb} placeholder={formType === "sqlite" ? "path/to/db.sqlite" : "(default)"} />
        </div>
        <div class="db-form-row">
          <label class="db-label">Label</label>
          <input class="db-input" bind:value={formLabel} placeholder="My DB" />
        </div>
        <div class="db-form-actions">
          <button class="tool-btn db-connect-btn" onclick={connect} disabled={formBusy}>
            {formBusy ? "Connecting..." : "Connect"}
          </button>
          <button class="db-cancel-btn" onclick={() => { showNewForm = false; resetForm(); }}>Cancel</button>
        </div>
      </div>
    {/if}

    {#if connections.length === 0}
      <div class="db-empty">
        <p>No connections</p>
        <button class="db-empty-add-btn" onclick={() => { showNewForm = true; }}>+ New Connection</button>
      </div>
    {:else}
      <div class="db-conn-list">
        {#each connections as conn}
          {@const node = treeNodes.get(conn.id)}
          <div class="db-conn-item">
            <div class="db-conn-header" onclick={() => toggleNode(conn.id)} role="button" tabindex="0">
              <span class="db-chevron" class:db-chevron-open={node?.expanded}>
                <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="9 18 15 12 9 6"/></svg>
              </span>
              <span class="db-conn-icon">
                {#if conn.db_type === "postgres"}
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><ellipse cx="12" cy="5" rx="9" ry="3"/><path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"/><path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"/></svg>
                {:else if conn.db_type === "mysql"}
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="9"/><path d="M8 12l2 2 4-4"/></svg>
                {:else if conn.db_type === "sqlite"}
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="18" height="18" rx="2"/><line x1="9" y1="9" x2="15" y2="15"/><line x1="15" y1="9" x2="9" y2="15"/></svg>
                {:else}
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="5"/><line x1="12" y1="1" x2="12" y2="3"/><line x1="12" y1="21" x2="12" y2="23"/><line x1="4.22" y1="4.22" x2="5.64" y2="5.64"/><line x1="18.36" y1="18.36" x2="19.78" y2="19.78"/><line x1="1" y1="12" x2="3" y2="12"/><line x1="21" y1="12" x2="23" y2="12"/></svg>
                {/if}
              </span>
              <span class="db-conn-name">{conn.label}</span>
              <span class="db-conn-type">{conn.db_type}</span>
              <button class="db-disconnect-btn" onclick={(e) => { e.stopPropagation(); disconnect(conn.id); }} title="Disconnect">
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
              </button>
            </div>

            {#if node?.expanded}
              <div class="db-children">
                {#if node.loading}
                  <div class="db-loading-sm">Loading databases...</div>
                {:else}
                  {#each node.databases as db}
                    {@const tblKey2 = `${conn.id}:${db}`}
                    <div class="db-tree-item">
                      <div class="db-tree-header" onclick={() => toggleDbTables(conn.id, db)} role="button" tabindex="0">
                        <span class="db-chevron" class:db-chevron-open={expandedDbs.has(tblKey2)}>
                          <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="9 18 15 12 9 6"/></svg>
                        </span>
                        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><ellipse cx="12" cy="6" rx="7" ry="3"/><path d="M5 6v6c0 1.66 3.13 3 7 3s7-1.34 7-3V6"/><path d="M5 12v6c0 1.66 3.13 3 7 3s7-1.34 7-3v-6"/></svg>
                        <span>{db}</span>
                      </div>
                      {#if expandedDbs.has(tblKey2)}
                        {@const tables2 = tableList.get(tblKey2) ?? []}
                        <div class="db-children">
                          {#each tables2 as table}
                            {@const colKey2 = `${conn.id}:${table.name}`}
                            <div class="db-tree-item">
                              <div class="db-tree-header" onclick={() => toggleTableColumns(conn.id, table.name)} role="button" tabindex="0">
                                <span class="db-chevron" class:db-chevron-open={columnExpanded.get(colKey2)}>
                                  <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="9 18 15 12 9 6"/></svg>
                                </span>
                                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/><line x1="16" y1="13" x2="8" y2="13"/><line x1="16" y1="17" x2="8" y2="17"/></svg>
                                <span>{table.name}</span>
                              </div>
                              {#if columnExpanded.get(colKey2)}
                                {@const cols2 = columnList.get(colKey2) ?? []}
                                <div class="db-children">
                                  {#each cols2 as col}
                                    <div class="db-col-item">
                                      <span class="db-col-name">{col.name}</span>
                                      <span class="db-col-type">{col.data_type}</span>
                                      {#if col.is_pk}
                                        <span class="db-col-badge db-col-pk">PK</span>
                                      {/if}
                                    </div>
                                  {/each}
                                </div>
                              {/if}
                            </div>
                          {/each}
                        </div>
                      {/if}
                    </div>
                  {/each}
                {/if}
                <button class="db-query-btn" onclick={() => openQueryTab(conn.id, conn.label)}>
                  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polygon points="5 3 19 12 5 21 5 3"/></svg>
                  New Query
                </button>
              </div>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  </div>
{:else}
  <div class="db-tab">
    <div class="db-tab-toolbar">
      <select class="db-input db-conn-select" bind:value={tabActiveConnId}>
        <option value={null}>Select connection...</option>
        {#each tabConnections as conn}
          <option value={conn.id}>{conn.label} ({conn.db_type})</option>
        {/each}
      </select>
      <button class="db-run-btn" onclick={runQuery} disabled={queryLoading || !tabActiveConnId || !querySql.trim()}>
        {queryLoading ? "Running..." : "Run"}
      </button>
    </div>

    <div class="db-editor-wrap">
      <textarea
        class="db-editor"
        bind:value={querySql}
        placeholder="SELECT * FROM ..."
        onkeydown={(e) => { if ((e.ctrlKey || e.metaKey) && e.key === "Enter") { runQuery(); } }}
      ></textarea>
    </div>

    {#if queryLoading}
      <div class="db-loading">Running query...</div>
    {:else if queryError}
      <div class="db-error">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><line x1="15" y1="9" x2="9" y2="15"/><line x1="9" y1="9" x2="15" y2="15"/></svg>
        {queryError}
      </div>
    {:else if queryResult}
      <div class="db-result-info">
        <span>{queryResult.row_count} row(s) in {queryResult.duration_ms}ms</span>
      </div>
      <div class="db-result-table-wrap">
        <table class="db-result-table">
          <thead>
            <tr>
              {#each queryResult.columns as col}
                <th>{col}</th>
              {/each}
            </tr>
          </thead>
          <tbody>
            {#each queryResult.rows as row}
              <tr>
                {#each row as cell}
                  <td>{typeof cell === "object" && cell !== null ? JSON.stringify(cell) : cell ?? "NULL"}</td>
                {/each}
              </tr>
            {:else}
              <tr><td colspan={queryResult.columns.length} class="db-empty-row">No rows returned</td></tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </div>
{/if}

<style>
  .db-sidebar { display:flex; flex-direction:column; height:100%; overflow:hidden; }
  .db-sidebar-header { display:flex; align-items:center; justify-content:space-between; padding:8px 12px; border-bottom:1px solid var(--border-primary); }
  .db-sidebar-title { font-size:var(--font-size); font-weight:600; text-transform:uppercase; letter-spacing:0.5px; color:var(--text-muted); }
  .db-add-btn { display:flex; align-items:center; background:transparent; border:1px solid var(--accent-blue); color:var(--accent-blue); border-radius:4px; padding:3px 6px; cursor:pointer; font-size:var(--fs-10); font-weight:600; transition:all 0.12s ease; }
  .db-add-btn:hover { background:var(--accent-blue); color:var(--bg-primary); }

  .db-new-form { padding:8px 12px; border-bottom:1px solid var(--border-primary); display:flex; flex-direction:column; gap:6px; }
  .db-form-row { display:flex; flex-direction:column; gap:2px; }
  .db-label { font-size:var(--fs-10); color:var(--text-muted); text-transform:uppercase; letter-spacing:0.5px; font-weight:600; }
  .db-input { background:var(--bg-primary); border:1px solid var(--border-subtle); border-radius:4px; padding:4px 6px; font-size:var(--font-size); color:var(--text-primary); outline:none; transition:border-color 0.1s; }
  .db-input:focus { border-color:var(--accent-blue); box-shadow:0 0 0 1px var(--accent-blue); }
  .db-form-actions { display:flex; gap:6px; margin-top:4px; }
  .db-connect-btn { background:var(--accent-blue); color:var(--bg-primary); border:none; border-radius:5px; padding:6px 12px; font-size:var(--fs-10); font-weight:600; cursor:pointer; transition:filter 0.1s; }
  .db-connect-btn:disabled { opacity:0.4; cursor:not-allowed; }
  .db-connect-btn:hover:not(:disabled) { filter:brightness(1.1); }
  .db-cancel-btn { background:transparent; border:1px solid var(--border-subtle); color:var(--text-muted); border-radius:5px; padding:5px 12px; font-size:var(--fs-10); font-weight:500; cursor:pointer; transition:all 0.12s ease; }
  .db-cancel-btn:hover { color:var(--text-primary); border-color:var(--text-muted); }

  .db-empty { display:flex; flex-direction:column; align-items:center; gap:8px; padding:20px; color:var(--text-muted); text-align:center; font-size:var(--fs-10); }
  .db-empty-add-btn { background:transparent; border:1px solid var(--accent-blue); color:var(--accent-blue); border-radius:5px; padding:5px 12px; font-size:var(--fs-10); font-weight:600; cursor:pointer; transition:all 0.12s ease; }
  .db-empty-add-btn:hover { background:var(--accent-blue); color:var(--bg-primary); }
  .db-conn-list { flex:1; overflow-y:auto; }
  .db-conn-item { border-bottom:1px solid var(--border-primary); }
  .db-conn-header { display:flex; align-items:center; gap:4px; padding:6px 12px; cursor:pointer; font-size:var(--font-size); transition:background 0.1s; }
  .db-conn-header:hover { background:var(--bg-hover); }
  .db-chevron { display:flex; align-items:center; transition:transform 0.15s; color:var(--text-muted); }
  .db-chevron-open { transform:rotate(90deg); }
  .db-conn-icon { display:flex; align-items:center; color:var(--accent-blue); }
  .db-conn-name { flex:1; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .db-conn-type { font-size:var(--fs-9); color:var(--text-muted); background:var(--bg-surface); padding:1px 4px; border-radius:3px; }
  .db-disconnect-btn { display:flex; align-items:center; padding:2px; border-radius:3px; color:var(--text-muted); opacity:0; transition:opacity 0.15s; background:none; border:none; cursor:pointer; }
  .db-conn-header:hover .db-disconnect-btn { opacity:1; }
  .db-disconnect-btn:hover { color:var(--accent-red); }

  .db-children { padding-left:16px; }
  .db-tree-header { display:flex; align-items:center; gap:3px; padding:4px 8px; cursor:pointer; font-size:var(--font-size); transition:background 0.1s; border-radius:3px; }
  .db-tree-header:hover { background:var(--bg-hover); }
  .db-loading-sm { padding:8px; font-size:var(--fs-11); color:var(--text-muted); text-align:center; }
  .db-query-btn { display:flex; align-items:center; gap:4px; margin:4px 0 4px 8px; padding:5px 10px; font-size:var(--fs-10); font-weight:600; background:var(--accent-blue); color:var(--bg-primary); border:none; border-radius:5px; cursor:pointer; transition:filter 0.1s; }
  .db-query-btn:hover { filter:brightness(1.1); }

  .db-col-item { display:flex; align-items:center; gap:4px; padding:3px 8px; font-size:var(--fs-11); }
  .db-col-name { font-weight:500; }
  .db-col-type { color:var(--text-muted); font-size:var(--fs-10); }
  .db-col-badge { font-size:var(--fs-9); padding:1px 3px; border-radius:2px; }
  .db-col-pk { background:var(--accent-yellow); color:var(--bg-primary); }

  /* Tab mode */
  .db-tab { display:flex; flex-direction:column; height:100%; overflow:hidden; }
  .db-tab-toolbar { display:flex; align-items:center; gap:8px; padding:8px 12px; border-bottom:1px solid var(--border-primary); }
  .db-conn-select { min-width:200px; }
  .db-run-btn { background:var(--accent-blue); color:var(--bg-primary); border:none; border-radius:5px; padding:6px 14px; font-size:var(--font-size); font-weight:600; cursor:pointer; transition:filter 0.1s; }
  .db-run-btn:disabled { opacity:0.4; cursor:not-allowed; }
  .db-run-btn:hover:not(:disabled) { filter:brightness(1.1); }
  .db-editor-wrap { flex:0 0 auto; }
  .db-editor { width:100%; min-height:120px; background:var(--bg-primary); border:none; border-bottom:1px solid var(--border-primary); padding:12px; font-family:var(--font-mono, monospace); font-size:var(--font-size); color:var(--text-primary); resize:vertical; outline:none; tab-size:2; }
  .db-editor:focus { border-color:var(--accent-blue); }
  .db-loading { display:flex; align-items:center; justify-content:center; padding:24px; color:var(--text-muted); font-size:var(--font-size); }
  .db-error { display:flex; align-items:center; gap:6px; padding:8px 12px; background:var(--bg-hover); color:var(--accent-red); font-size:var(--font-size); border-bottom:1px solid var(--border-primary); }
  .db-result-info { padding:4px 12px; font-size:var(--fs-11); color:var(--text-muted); border-bottom:1px solid var(--border-primary); }
  .db-result-table-wrap { flex:1; overflow:auto; }
  .db-result-table { width:100%; border-collapse:collapse; font-size:var(--font-size); }
  .db-result-table th { position:sticky; top:0; background:var(--bg-secondary); padding:4px 8px; text-align:left; font-weight:600; border-bottom:1px solid var(--border-primary); white-space:nowrap; }
  .db-result-table td { padding:3px 8px; border-bottom:1px solid var(--border-primary); max-width:300px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .db-result-table tr:hover td { background:var(--bg-hover); }
  .db-empty-row { text-align:center; color:var(--text-muted); padding:24px !important; }
</style>
