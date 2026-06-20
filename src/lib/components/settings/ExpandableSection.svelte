<script lang="ts">
  let { title, defaultOpen = false, children }: { title: string; defaultOpen?: boolean; children?: import('svelte').Snippet } = $props();

  let open = $state(false);
  let storageKey = $state('');

  $effect(() => {
    const key = `setting-section-${title.toLowerCase().replace(/\s+/g, '-')}`;
    storageKey = key;
    const stored = localStorage.getItem(key);
    open = stored !== null ? stored === 'true' : defaultOpen;
  });

  function toggle() {
    open = !open;
    localStorage.setItem(storageKey, String(open));
  }
</script>

<div class="expandable-section" class:open>
  <button class="expandable-header" onclick={toggle}>
    <span class="expandable-title">{title}</span>
    <svg class="expandable-chevron" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
      <polyline points="9 18 15 12 9 6" />
    </svg>
  </button>
  {#if open}
    <div class="expandable-body">
      {#if children}{@render children()}{/if}
    </div>
  {/if}
</div>

<style>
  .expandable-section {
    border: 1px solid var(--border-subtle);
    border-radius: 8px;
    background: var(--bg-surface);
    overflow: hidden;
  }
  .expandable-section.open {
    border-color: var(--border-primary);
  }
  .expandable-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    padding: 10px 14px;
    background: none;
    border: none;
    color: var(--text-primary);
    font-size: var(--fs-11);
    font-weight: 600;
    cursor: pointer;
    transition: background 0.12s;
    text-align: left;
  }
  .expandable-header:hover {
    background: var(--bg-hover);
  }
  .expandable-title {
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-secondary);
  }
  .expandable-chevron {
    color: var(--text-muted);
    transition: transform 0.15s ease;
    flex-shrink: 0;
  }
  .open .expandable-chevron {
    transform: rotate(90deg);
  }
  .expandable-body {
    padding: 0 14px 12px;
    border-top: 1px solid var(--border-subtle);
    padding-top: 10px;
    animation: slideDown 0.15s ease-out;
  }
  @keyframes slideDown {
    from { opacity: 0; transform: translateY(-4px); }
    to { opacity: 1; transform: translateY(0); }
  }
</style>
