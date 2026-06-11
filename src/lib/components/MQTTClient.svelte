<script lang="ts">
  import { addToast, currentDir } from "$lib/stores.svelte";
  import { loadNyxConfig, saveNyxConfig } from "$lib/nyxConfig";
  import { invoke } from "@tauri-apps/api/core";
  import { save as saveDialog } from "@tauri-apps/plugin-dialog";

  type MqttConnection = {
    id: string;
    name: string;
    host: string;
    port: number;
    path: string;
    clientId: string;
    username?: string;
    password?: string;
    protocol: "ws" | "wss";
    subTopic: string;
    pubTopic: string;
    pubPayload: string;
    qos: number;
    retain: boolean;
  };

  let connections = $state<MqttConnection[]>([]);
  let activeConnectionId = $state<string | null>(null);
  let workspaceDir = $state("");
  let isInitialLoad = $state(true);

  // Local binding fields for active configuration
  let host = $state("broker.emqx.io");
  let port = $state(8083);
  let path = $state("/mqtt");
  let clientId = $state("");
  let username = $state("");
  let password = $state("");
  let protocol = $state<"ws" | "wss">("ws");
  let subTopic = $state("test/topic");
  let pubTopic = $state("test/topic");
  let pubPayload = $state("");
  let qos = $state(0);
  let retain = $state(false);

  let isUpdatingLocal = false;
  let editingId = $state<string | null>(null);
  let editingNameVal = $state("");

  $effect(() => {
    const unsub = currentDir.subscribe(async (val) => {
      workspaceDir = val;
      isInitialLoad = true;
      await loadConfig();
    });
    return unsub;
  });

  const defaultConnections: MqttConnection[] = [
    {
      id: "conn-default",
      name: "Default EMQX Public Broker",
      host: "broker.emqx.io",
      port: 8083,
      path: "/mqtt",
      clientId: "nyx-client-" + Math.random().toString(36).substring(2, 8),
      username: "",
      password: "",
      protocol: "ws",
      subTopic: "test/topic",
      pubTopic: "test/topic",
      pubPayload: "Hello EMQX Broker",
      qos: 0,
      retain: false,
    },
  ];

  async function loadConfig() {
    if (!workspaceDir) return;
    try {
      const config = await loadNyxConfig("mqtt.json", {
        connections: defaultConnections,
      });
      connections = config.connections || defaultConnections;
      if (connections.length > 0) {
        selectConnection(connections[0].id);
      } else {
        activeConnectionId = null;
      }
    } catch (e) {
      console.error("Failed to load MQTT config:", e);
      connections = defaultConnections;
      selectConnection(connections[0].id);
    } finally {
      isInitialLoad = false;
    }
  }

  function saveConfig() {
    if (workspaceDir && !isInitialLoad) {
      saveNyxConfig("mqtt.json", { connections });
    }
  }

  function selectConnection(id: string) {
    const conn = connections.find((c) => c.id === id);
    if (!conn) return;

    isUpdatingLocal = true;
    activeConnectionId = id;
    host = conn.host;
    port = conn.port;
    path = conn.path;
    clientId = conn.clientId;
    username = conn.username || "";
    password = conn.password || "";
    protocol = conn.protocol || "ws";
    subTopic = conn.subTopic || "test/topic";
    pubTopic = conn.pubTopic || "test/topic";
    pubPayload = conn.pubPayload || "";
    qos = conn.qos !== undefined ? conn.qos : 0;
    retain = conn.retain !== undefined ? conn.retain : false;

    // Disconnect if we switched profiles
    if (ws && isConnected) {
      disconnect();
    }

    setTimeout(() => {
      isUpdatingLocal = false;
    }, 0);
  }

  $effect(() => {
    const data = {
      host,
      port,
      path,
      clientId,
      username,
      password,
      protocol,
      subTopic,
      pubTopic,
      pubPayload,
      qos,
      retain,
    };
    if (activeConnectionId && !isUpdatingLocal && !isInitialLoad) {
      const idx = connections.findIndex((c) => c.id === activeConnectionId);
      if (idx >= 0) {
        connections[idx] = {
          ...connections[idx],
          ...data,
        };
        connections = [...connections];
        saveConfig();
      }
    }
  });

  function addConnection() {
    const id = "conn-" + Date.now().toString(36);
    const newConn: MqttConnection = {
      id,
      name: "New Connection",
      host: "broker.emqx.io",
      port: 8083,
      path: "/mqtt",
      clientId: "nyx-client-" + Math.random().toString(36).substring(2, 8),
      username: "",
      password: "",
      protocol: "ws",
      subTopic: "test/topic",
      pubTopic: "test/topic",
      pubPayload: "Hello Broker",
      qos: 0,
      retain: false,
    };
    connections = [...connections, newConn];
    saveConfig();
    selectConnection(id);
    startRenaming(newConn);
  }

  function deleteConnection(id: string) {
    connections = connections.filter((c) => c.id !== id);
    saveConfig();
    if (activeConnectionId === id) {
      if (connections.length > 0) {
        selectConnection(connections[0].id);
      } else {
        activeConnectionId = null;
      }
    }
  }

  function startRenaming(conn: MqttConnection) {
    editingId = conn.id;
    editingNameVal = conn.name;
  }

  function finishRename(conn: MqttConnection) {
    if (!editingNameVal.trim()) return;
    const idx = connections.findIndex((c) => c.id === conn.id);
    if (idx >= 0) {
      connections[idx].name = editingNameVal.trim();
      connections = [...connections];
      saveConfig();
    }
    editingId = null;
  }

  function generateClientId() {
    clientId = "nyx-client-" + Math.random().toString(36).substring(2, 10);
  }

  async function exportMQTTX() {
    // Generate MQTTX import format
    const exportData = {
      connections: connections.map((c) => ({
        id: c.id,
        name: c.name,
        clientId: c.clientId,
        clean: true,
        connectTimeout: 4000,
        keepalive: 60,
        username: c.username || "",
        password: c.password || "",
        path: c.path,
        port: c.port,
        protocol: c.protocol,
        host: c.host,
        qos: c.qos || 0,
        retain: c.retain || false,
        ssl: c.protocol === "wss",
        mqttVersion: "3.1.1",
        subscriptions: [
          {
            id: "sub-" + Math.random().toString(36).substring(2, 8),
            topic: c.subTopic,
            qos: c.qos || 0,
          },
        ],
      })),
    };
    const json = JSON.stringify(exportData, null, 2);
    try {
      const path = await saveDialog({
        defaultPath: "nyx-mqttx-export.json",
        filters: [{ name: "MQTTX Export", extensions: ["json"] }],
      });
      if (path) {
        await invoke("fs_write_file", { path, content: json });
        addToast("Exported to " + path, "success");
      }
    } catch {
      // Fallback
      const blob = new Blob([json], { type: "application/json" });
      const a = document.createElement("a");
      a.href = URL.createObjectURL(blob);
      a.download = "nyx-mqttx-export.json";
      a.click();
      addToast("Exported to download file", "success");
    }
  }

  // ── MQTT Packet Encoders ──────────────────────────────────────────────────
  function buildConnectPacket(
    clientId: string,
    username?: string,
    password?: string,
  ): Uint8Array {
    const encoder = new TextEncoder();
    const clientIdBytes = encoder.encode(clientId);
    const usernameBytes = username
      ? encoder.encode(username)
      : new Uint8Array(0);
    const passwordBytes = password
      ? encoder.encode(password)
      : new Uint8Array(0);

    let connectFlags = 0x02; // Clean session
    if (username) {
      connectFlags |= 0x80;
      if (password) {
        connectFlags |= 0x40;
      }
    }

    let payloadLen = 2 + clientIdBytes.length;
    if (username) {
      payloadLen += 2 + usernameBytes.length;
      if (password) {
        payloadLen += 2 + passwordBytes.length;
      }
    }

    const varHeaderLen = 10;
    const remainingLen = varHeaderLen + payloadLen;

    const remLenBytes = [];
    let temp = remainingLen;
    do {
      let encodedByte = temp % 128;
      temp = Math.floor(temp / 128);
      if (temp > 0) {
        encodedByte |= 128;
      }
      remLenBytes.push(encodedByte);
    } while (temp > 0);

    const packet = new Uint8Array(1 + remLenBytes.length + remainingLen);
    let offset = 0;
    packet[offset++] = 0x10; // CONNECT

    for (const b of remLenBytes) {
      packet[offset++] = b;
    }

    packet[offset++] = 0;
    packet[offset++] = 4; // protocol name length
    packet[offset++] = 0x4d;
    packet[offset++] = 0x51;
    packet[offset++] = 0x54;
    packet[offset++] = 0x54; // "MQTT"
    packet[offset++] = 4; // level
    packet[offset++] = connectFlags;
    packet[offset++] = 0;
    packet[offset++] = 60; // Keep Alive (60s)

    // Client ID
    packet[offset++] = (clientIdBytes.length >> 8) & 0xff;
    packet[offset++] = clientIdBytes.length & 0xff;
    packet.set(clientIdBytes, offset);
    offset += clientIdBytes.length;

    // Username
    if (username) {
      packet[offset++] = (usernameBytes.length >> 8) & 0xff;
      packet[offset++] = usernameBytes.length & 0xff;
      packet.set(usernameBytes, offset);
      offset += usernameBytes.length;
    }

    // Password
    if (username && password) {
      packet[offset++] = (passwordBytes.length >> 8) & 0xff;
      packet[offset++] = passwordBytes.length & 0xff;
      packet.set(passwordBytes, offset);
      offset += passwordBytes.length;
    }

    return packet;
  }

  function buildSubscribePacket(topic: string, subQos: number): Uint8Array {
    const topicBytes = new TextEncoder().encode(topic);
    const remainingLen = 2 + 2 + topicBytes.length + 1; // Packet ID (2) + Topic Len (2) + Topic + QoS (1)

    const remLenBytes = [];
    let temp = remainingLen;
    do {
      let encodedByte = temp % 128;
      temp = Math.floor(temp / 128);
      if (temp > 0) {
        encodedByte |= 128;
      }
      remLenBytes.push(encodedByte);
    } while (temp > 0);

    const packet = new Uint8Array(1 + remLenBytes.length + remainingLen);
    let offset = 0;
    packet[offset++] = 0x82; // SUBSCRIBE

    for (const b of remLenBytes) {
      packet[offset++] = b;
    }

    packet[offset++] = 0;
    packet[offset++] = 1; // Message ID (1)
    packet[offset++] = (topicBytes.length >> 8) & 0xff;
    packet[offset++] = topicBytes.length & 0xff;
    packet.set(topicBytes, offset);
    offset += topicBytes.length;
    packet[offset] = subQos;

    return packet;
  }

  function buildPublishPacket(
    topic: string,
    payload: string,
    pubQos: number,
    pubRetain: boolean,
  ): Uint8Array {
    const topicBytes = new TextEncoder().encode(topic);
    const payloadBytes = new TextEncoder().encode(payload);

    let remainingLen = 2 + topicBytes.length + payloadBytes.length;
    if (pubQos > 0) {
      remainingLen += 2; // Packet ID (2)
    }

    const remLenBytes = [];
    let temp = remainingLen;
    do {
      let encodedByte = temp % 128;
      temp = Math.floor(temp / 128);
      if (temp > 0) {
        encodedByte |= 128;
      }
      remLenBytes.push(encodedByte);
    } while (temp > 0);

    const packet = new Uint8Array(1 + remLenBytes.length + remainingLen);
    let offset = 0;

    let headerByte = 0x30;
    headerByte |= pubQos << 1;
    if (pubRetain) {
      headerByte |= 0x01;
    }
    packet[offset++] = headerByte;

    for (const b of remLenBytes) {
      packet[offset++] = b;
    }

    // Topic Name
    packet[offset++] = (topicBytes.length >> 8) & 0xff;
    packet[offset++] = topicBytes.length & 0xff;
    packet.set(topicBytes, offset);
    offset += topicBytes.length;

    // Packet ID
    if (pubQos > 0) {
      packet[offset++] = 0;
      packet[offset++] = 1;
    }

    // Payload
    packet.set(payloadBytes, offset);

    return packet;
  }

  // ── Connection Logic ──────────────────────────────────────────────────────
  let ws = $state<WebSocket | null>(null);
  let isConnected = $state(false);
  let messages = $state<
    {
      time: string;
      topic: string;
      payload: string;
      type: "pub" | "sub" | "sys";
    }[]
  >([]);

  function addLog(topic: string, payload: string, type: "pub" | "sub" | "sys") {
    const t = new Date().toLocaleTimeString();
    messages = [{ time: t, topic, payload, type }, ...messages];
  }

  function connect() {
    if (ws) {
      ws.close();
    }
    const proto = protocol || "ws";
    const url = `${proto}://${host}:${port}${path}`;
    addLog("System", `Connecting to WS Broker: ${url}`, "sys");

    try {
      ws = new WebSocket(url, ["mqtt"]);
      ws.binaryType = "arraybuffer";

      ws.onopen = () => {
        isConnected = true;
        addLog(
          "System",
          "WebSocket Connection opened! Sending CONNECT packet...",
          "sys",
        );

        const connectPacket = buildConnectPacket(
          clientId,
          username || undefined,
          password || undefined,
        );
        ws?.send(connectPacket.buffer);
      };

      ws.onmessage = (event) => {
        if (event.data instanceof ArrayBuffer) {
          const bytes = new Uint8Array(event.data);
          const packetType = bytes[0] >> 4;

          if (packetType === 2) {
            // CONNACK
            addLog("System", "MQTT Connection Accepted! Connected.", "sys");
            addToast("MQTT Connected", "success");
            // Auto-subscribe
            subscribeTopic();
          } else if (packetType === 3) {
            // PUBLISH
            // Decode simple MQTT Publish packet
            const remLen = bytes[1];
            const topicLen = (bytes[2] << 8) + bytes[3];
            const topic = new TextDecoder().decode(
              bytes.subarray(4, 4 + topicLen),
            );
            const payload = new TextDecoder().decode(
              bytes.subarray(4 + topicLen),
            );
            addLog(topic, payload, "sub");
          }
        }
      };

      ws.onclose = () => {
        isConnected = false;
        ws = null;
        addLog("System", "WebSocket Connection closed.", "sys");
        addToast("MQTT Disconnected");
      };

      ws.onerror = (err) => {
        addLog("System", "Connection Error occurred.", "sys");
      };
    } catch (err: any) {
      addLog("System", `Error: ${err.message}`, "sys");
    }
  }

  function disconnect() {
    if (ws) {
      ws.close();
    }
  }

  function subscribeTopic() {
    if (!ws || !isConnected || !subTopic) return;

    const packet = buildSubscribePacket(subTopic, qos);
    ws.send(packet.buffer);
    addLog("System", `Subscribed to topic: ${subTopic} (QoS ${qos})`, "sys");
  }

  function publishMessage() {
    if (!ws || !isConnected || !pubTopic) return;

    const packet = buildPublishPacket(pubTopic, pubPayload, qos, retain);
    ws.send(packet.buffer);
    addLog(pubTopic, pubPayload, "pub");
    pubPayload = "";
  }

  let activeSubTab = $state<"listen" | "send">("listen");
</script>

<div class="mqtt-root">
  {#if activeConnectionId}
    <!-- ── Connection Selector Dropdown ── -->
    <div class="mqtt-top-bar">
      <select class="mqtt-select" bind:value={activeConnectionId} onchange={(e) => selectConnection((e.target as HTMLSelectElement).value)}>
        {#each connections as conn}
          <option value={conn.id}>{conn.name} ({conn.host}:{conn.port})</option>
        {/each}
      </select>
      <div class="mqtt-top-actions">
        <button class="mqtt-icon-btn" onclick={addConnection} title="New Connection">
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" /></svg>
        </button>
        <button class="mqtt-icon-btn" onclick={exportMQTTX} title="Export to MQTTX">
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" /><polyline points="7 10 12 15 17 10" /><line x1="12" y1="15" x2="12" y2="3" /></svg>
        </button>
        <button class="mqtt-icon-btn" onclick={() => { const c = connections.find(x => x.id === activeConnectionId); if (c) deleteConnection(c.id); }} title="Delete Connection">
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="3 6 5 6 21 6" /><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" /></svg>
        </button>
      </div>
    </div>

    <!-- ── Connection Form ── -->
    <div class="mqtt-section">
      <div class="mqtt-section-title">CONNECTION</div>
      <div class="mqtt-form-grid">
        <div class="form-group">
          <span class="form-label">Profile Name</span>
          <div style="display: flex; gap: 4px;">
            <input class="form-input" value={connections.find((c) => c.id === activeConnectionId)?.name ?? ''} oninput={(e) => { const c = connections.find(x => x.id === activeConnectionId); if (c) { c.name = (e.target as HTMLInputElement).value; connections = [...connections]; }} } style="flex:1;" />
          </div>
        </div>
        <div class="form-group">
          <span class="form-label">Protocol</span>
          <select class="form-input" bind:value={protocol}>
            <option value="ws">ws://</option>
            <option value="wss">wss://</option>
          </select>
        </div>
        <div class="form-group">
          <span class="form-label">Host</span>
          <input class="form-input" bind:value={host} placeholder="broker.emqx.io" />
        </div>
        <div class="form-group">
          <span class="form-label">Port</span>
          <input type="number" class="form-input" bind:value={port} />
        </div>
        <div class="form-group">
          <span class="form-label">WS Path</span>
          <input class="form-input" bind:value={path} placeholder="/mqtt" />
        </div>
        <div class="form-group">
          <span class="form-label">Client ID</span>
          <div style="display: flex; gap: 4px;">
            <input class="form-input" bind:value={clientId} placeholder="nyx-client-xxx" style="flex:1;" />
            <button class="mqtt-sm-btn" onclick={generateClientId}>Gen</button>
          </div>
        </div>
        <div class="form-group">
          <span class="form-label">Username</span>
          <input class="form-input" bind:value={username} placeholder="(optional)" />
        </div>
        <div class="form-group">
          <span class="form-label">Password</span>
          <input type="password" class="form-input" bind:value={password} placeholder="(optional)" />
        </div>
      </div>
    </div>

    <!-- ── Connect/Disconnect Bar ── -->
    <div class="mqtt-status-bar">
      <div class="mqtt-status-indicator">
        <div class="mqtt-dot" class:mqtt-dot-connected={isConnected}></div>
        <span>{isConnected ? "Connected" : "Disconnected"}</span>
      </div>
      {#if isConnected}
        <button onclick={disconnect} class="mqtt-btn mqtt-btn-danger">Disconnect</button>
      {:else}
        <button onclick={connect} class="mqtt-btn mqtt-btn-primary">Connect</button>
      {/if}
    </div>

    <!-- ── Sub/Pub Tabs ── -->
    <div class="mqtt-sub-tabs">
      <button class="mqtt-sub-tab" class:active={activeSubTab === "listen"} onclick={() => (activeSubTab = "listen")}>Listen (Subscribe)</button>
      <button class="mqtt-sub-tab" class:active={activeSubTab === "send"} onclick={() => (activeSubTab = "send")}>Send (Publish)</button>
    </div>

    <div class="mqtt-section">
      {#if activeSubTab === "listen"}
        <div class="form-group">
          <span class="form-label">Subscribe Topic</span>
          <div class="mqtt-row">
            <input class="form-input" bind:value={subTopic} placeholder="test/topic" style="flex:1;" />
            <select class="form-input" bind:value={qos} style="width: 65px;">
              <option value={0}>QoS0</option>
              <option value={1}>QoS1</option>
              <option value={2}>QoS2</option>
            </select>
            <button onclick={subscribeTopic} disabled={!isConnected} class="mqtt-sm-btn">Sub</button>
          </div>
        </div>
      {:else}
        <div class="form-group">
          <span class="form-label">Publish Topic</span>
          <div class="mqtt-row">
            <input class="form-input" bind:value={pubTopic} placeholder="topic" style="flex:1;" />
            <select class="form-input" bind:value={qos} style="width: 65px;">
              <option value={0}>QoS0</option>
              <option value={1}>QoS1</option>
              <option value={2}>QoS2</option>
            </select>
          </div>
        </div>
        <div class="form-group" style="margin-top: 4px;">
          <div class="mqtt-row">
            <input class="form-input" bind:value={pubPayload} placeholder="Payload (JSON or Text)" style="flex:1;" />
            <label class="mqtt-checkbox-label">
              <input type="checkbox" bind:checked={retain} /> Retain
            </label>
            <button onclick={publishMessage} disabled={!isConnected || !pubPayload} class="mqtt-sm-btn">Pub</button>
          </div>
        </div>
      {/if}
    </div>

    <!-- ── Log Viewer ── -->
    <div class="mqtt-log-section">
      <span class="mqtt-log-title">Message Log</span>
      <div class="mqtt-log-viewer">
        {#each messages as msg}
          <div class="mqtt-log-item {msg.type}">
            <span class="log-time">[{msg.time}]</span>
            {#if msg.type === "sys"}
              <span class="log-sys">{msg.payload}</span>
            {:else}
              <span class="log-topic">{msg.topic}:</span>
              <span class="log-payload">{msg.payload}</span>
            {/if}
          </div>
        {:else}
          <div class="log-empty">No activity logged yet.</div>
        {/each}
      </div>
    </div>
  {:else}
    <div class="mqtt-empty">
      <p>No connection profiles.</p>
      <button class="mqtt-btn mqtt-btn-primary" onclick={addConnection}>+ New Connection</button>
    </div>
  {/if}
</div>

<style>
  .mqtt-root {
    display: flex;
    flex-direction: column;
    height: 100%;
    padding: 8px;
    gap: 6px;
    overflow-y: auto;
    background: transparent;
    color: var(--text-primary);
  }

  /* ── Top Bar ── */
  .mqtt-top-bar {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }
  .mqtt-select {
    flex: 1;
    background: var(--bg-surface);
    color: var(--text-primary);
    border: 1px solid var(--border-subtle);
    border-radius: 4px;
    padding: 4px 6px;
    font-size: var(--fs-10);
    cursor: pointer;
    min-width: 0;
  }
  .mqtt-select:focus {
    outline: none;
    border-color: var(--accent-blue);
  }
  .mqtt-top-actions {
    display: flex;
    gap: 2px;
    flex-shrink: 0;
  }
  .mqtt-icon-btn {
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    padding: 3px;
    border-radius: 4px;
    display: flex;
    align-items: center;
  }
  .mqtt-icon-btn:hover {
    color: var(--text-primary);
    background: var(--bg-hover);
  }

  /* ── Sections ── */
  .mqtt-section {
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: 6px;
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .mqtt-section-title {
    font-size: var(--fs-9);
    font-weight: 700;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }
  .mqtt-form-grid {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .form-group {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .form-label {
    font-size: var(--fs-9);
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
  }
  .form-input {
    background: var(--bg-primary);
    color: var(--text-primary);
    border: 1px solid var(--border-subtle);
    border-radius: 4px;
    padding: 4px 6px;
    font-size: var(--fs-10);
    width: 100%;
    box-sizing: border-box;
  }
  .form-input:focus {
    outline: none;
    border-color: var(--accent-blue);
  }
  select.form-input {
    height: 25px;
  }

  .mqtt-sm-btn {
    background: var(--bg-hover);
    color: var(--text-primary);
    border: 1px solid var(--border-subtle);
    border-radius: 4px;
    padding: 3px 8px;
    font-size: var(--fs-9);
    cursor: pointer;
    font-weight: 600;
    flex-shrink: 0;
    transition: all 0.1s;
  }
  .mqtt-sm-btn:hover:not(:disabled) {
    border-color: var(--accent-blue);
    background: var(--bg-primary);
  }
  .mqtt-sm-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* ── Status Bar ── */
  .mqtt-status-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: 6px;
    padding: 6px 8px;
  }
  .mqtt-status-indicator {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-10);
    font-weight: 600;
    color: var(--text-muted);
  }
  .mqtt-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--text-muted);
  }
  .mqtt-dot-connected {
    background: var(--accent-green);
    box-shadow: 0 0 6px color-mix(in srgb, var(--accent-green) 50%, transparent);
  }
  .mqtt-btn {
    border: none;
    border-radius: 4px;
    padding: 4px 12px;
    font-weight: 600;
    cursor: pointer;
    font-size: var(--fs-10);
    transition: filter 0.12s;
  }
  .mqtt-btn:hover {
    filter: brightness(1.1);
  }
  .mqtt-btn-primary {
    background: var(--accent-blue);
    color: var(--bg-primary);
  }
  .mqtt-btn-danger {
    background: var(--accent-red);
    color: var(--bg-primary);
  }

  /* ── Sub/Pub Tabs ── */
  .mqtt-sub-tabs {
    display: flex;
    gap: 4px;
    background: var(--bg-surface);
    padding: 2px;
    border-radius: 6px;
    border: 1px solid var(--border-subtle);
  }
  .mqtt-sub-tab {
    flex: 1;
    border: none;
    background: transparent;
    color: var(--text-muted);
    padding: 4px;
    border-radius: 4px;
    font-size: var(--fs-10);
    font-weight: 600;
    cursor: pointer;
    transition: all 0.12s ease;
  }
  .mqtt-sub-tab.active {
    background: var(--accent-blue);
    color: var(--bg-primary);
  }

  .mqtt-row {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .mqtt-checkbox-label {
    display: flex;
    align-items: center;
    gap: 4px;
    cursor: pointer;
    font-size: var(--fs-9);
    color: var(--text-muted);
    flex-shrink: 0;
  }

  /* ── Log Viewer ── */
  .mqtt-log-section {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-height: 100px;
  }
  .mqtt-log-title {
    font-size: var(--fs-9);
    font-weight: 700;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }
  .mqtt-log-viewer {
    flex: 1;
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: 6px;
    padding: 6px;
    overflow-y: auto;
    font-family: monospace;
    font-size: var(--fs-9-5);
  }
  .mqtt-log-item {
    margin-bottom: 4px;
    line-height: 1.3;
  }
  .mqtt-log-item.sys {
    color: var(--accent-cyan);
  }
  .mqtt-log-item.pub {
    color: var(--accent-green);
  }
  .mqtt-log-item.sub {
    color: var(--text-primary);
  }
  .log-time {
    color: var(--text-muted);
    margin-right: 4px;
  }
  .log-topic {
    font-weight: 600;
    margin-right: 4px;
  }
  .log-empty {
    color: var(--text-muted);
    text-align: center;
    padding: 20px 0;
  }
  .mqtt-empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    color: var(--text-muted);
    font-size: var(--fs-11);
  }
</style>
