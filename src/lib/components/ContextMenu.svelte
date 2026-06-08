<script lang="ts">
  let {
    x = 0,
    y = 0,
    open = false,
    items = [] as { label: string; icon?: string; danger?: boolean; action: () => void }[],
    onclose = () => {},
  } = $props();

  function onBackdropClick() {
    onclose();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") onclose();
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if open}
  <div class="cm-backdrop" onclick={onBackdropClick} oncontextmenu={(e) => e.preventDefault()} onkeydown={(e) => { if (e.key === "Escape") onclose(); }} role="presentation"></div>
  <div class="cm-menu" style="left: {x}px; top: {y}px" onclick={() => onclose()} role="menu">
    {#each items as item}
      <button class="cm-item" class:cm-danger={item.danger} onclick={() => { item.action(); onclose(); }}>
        {#if item.icon}
          <span class="cm-icon">{@html item.icon}</span>
        {/if}
        <span>{item.label}</span>
      </button>
    {/each}
  </div>
{/if}

<style>
  .cm-backdrop { position:fixed; inset:0; z-index:300; }
  .cm-menu {
    position:fixed; z-index:301; min-width:160px;
    background:var(--bg-elevated); border:1px solid var(--border-primary);
    border-radius:8px; padding:4px;
    box-shadow:0 8px 24px rgba(0,0,0,0.5);
    animation:cmIn 0.1s ease;
  }
  .cm-item {
    display:flex; align-items:center; gap:8px; width:100%;
    padding:6px 10px; border:none; background:transparent;
    color:var(--text-primary); font-size:var(--font-size); cursor:pointer;
    border-radius:5px; transition:all 0.1s ease; white-space:nowrap;
  }
  .cm-item:hover { background:var(--bg-hover); }
  .cm-item.cm-danger:hover { color:var(--accent-red); background:color-mix(in srgb, var(--accent-red) 12%, transparent); }
  .cm-icon { display:flex; width:16px; justify-content:center; flex-shrink:0; }
  @keyframes cmIn { from { opacity:0; transform:scale(0.95); } to { opacity:1; transform:scale(1); } }
</style>
