<script lang="ts">
  import { addToast } from "../stores.svelte";

  type Command = {
    id: string;
    label: string;
    desc?: string;
    icon?: string;
    action: () => void;
  };

  let {
    open = false,
    onclose = () => {},
    commands = [] as Command[],
  } = $props();

  let query = $state("");
  let activeIdx = $state(0);

  let filtered = $derived(
    query
      ? commands.filter((c) => c.label.toLowerCase().includes(query.toLowerCase()) || (c.desc?.toLowerCase().includes(query.toLowerCase())))
      : commands
  );

  $effect(() => {
    if (open) { query = ""; activeIdx = 0; }
  });

  function onKeydown(e: KeyboardEvent) {
    if (!open) return;
    if (e.key === "Escape") { e.preventDefault(); onclose(); return; }
    if (e.key === "ArrowDown") { e.preventDefault(); activeIdx = Math.min(activeIdx + 1, filtered.length - 1); return; }
    if (e.key === "ArrowUp") { e.preventDefault(); activeIdx = Math.max(activeIdx - 1, 0); return; }
    if (e.key === "Enter" && filtered[activeIdx]) {
      e.preventDefault();
      filtered[activeIdx].action();
      onclose();
    }
  }

  function runCmd(cmd: Command) {
    cmd.action();
    onclose();
  }

  function onBackdropClick() {
    onclose();
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if open}
  <div class="cp-backdrop" onclick={onBackdropClick} onkeydown={(e) => { if (e.key === "Escape") onclose(); }} role="presentation"></div>
  <div class="cp-modal" role="dialog" aria-label="Command Palette">
    <div class="cp-input-wrap">
      <svg class="cp-search-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
      <input class="cp-input" bind:value={query} placeholder="Type a command..." autofocus />
    </div>
    <div class="cp-results">
      {#each filtered as cmd, i}
        <button class="cp-item" class:cp-active={i === activeIdx} onclick={() => runCmd(cmd)} onmouseenter={() => (activeIdx = i)}>
          {#if cmd.icon}
            <span class="cp-item-icon">{@html cmd.icon}</span>
          {/if}
          <div class="cp-item-info">
            <span class="cp-item-label">{cmd.label}</span>
            {#if cmd.desc}
              <span class="cp-item-desc">{cmd.desc}</span>
            {/if}
          </div>
        </button>
      {:else}
        <div class="cp-empty">No commands found</div>
      {/each}
    </div>
  </div>
{/if}

<style>
  .cp-backdrop { position:fixed; inset:0; background:rgba(0,0,0,0.5); z-index:400; animation:fadeIn 0.1s ease; }
  .cp-modal {
    position:fixed; top:15%; left:50%; transform:translateX(-50%);
    width:480px; max-width:90vw; max-height:60vh;
    background:var(--bg-elevated); border:1px solid var(--border-primary);
    border-radius:12px; box-shadow:0 16px 48px rgba(0,0,0,0.6);
    display:flex; flex-direction:column; overflow:hidden;
    z-index:401; animation:cpIn 0.15s ease;
  }
  .cp-input-wrap { display:flex; align-items:center; gap:8px; padding:10px 14px; border-bottom:1px solid var(--border-subtle); }
  .cp-search-icon { color:var(--text-muted); flex-shrink:0; }
  .cp-input { flex:1; background:transparent; border:none; color:var(--text-primary); font-size:var(--fs-14); outline:none; font-family:inherit; }
  .cp-input::placeholder { color:var(--text-muted); }
  .cp-results { flex:1; overflow-y:auto; padding:4px; max-height:400px; }
  .cp-item { display:flex; align-items:center; gap:10px; width:100%; padding:8px 10px; border:none; background:transparent; color:var(--text-primary); font-size:var(--font-size); cursor:pointer; border-radius:6px; text-align:left; transition:all 0.08s ease; }
  .cp-item:hover, .cp-item.cp-active { background:var(--bg-hover); }
  .cp-item-icon { display:flex; width:18px; justify-content:center; flex-shrink:0; color:var(--text-muted); }
  .cp-item-info { display:flex; flex-direction:column; gap:1px; min-width:0; flex:1; }
  .cp-item-label { font-size:var(--fs-12); font-weight:500; }
  .cp-item-desc { font-size:var(--fs-10); color:var(--text-muted); }
  .cp-empty { padding:20px; text-align:center; color:var(--text-muted); font-size:var(--fs-11); }

  @keyframes fadeIn { from { opacity:0; } to { opacity:1; } }
  @keyframes cpIn { from { opacity:0; transform:translateX(-50%) translateY(-12px) scale(0.96); } to { opacity:1; transform:translateX(-50%) translateY(0) scale(1); } }
</style>
