<script lang="ts" module>
  export interface MenuItem {
    label: string;
    run: () => void;
    danger?: boolean;
  }
</script>

<script lang="ts">
  import { onMount } from 'svelte';

  let { x, y, items, onclose }: { x: number; y: number; items: MenuItem[]; onclose: () => void } = $props();

  let menu: HTMLElement;
  let left = $state(0);
  let top = $state(0);
  const buttons = () => [...menu.querySelectorAll<HTMLButtonElement>('[role=menuitem]')];

  onMount(() => {
    // Open at the pointer, but never past the window edge.
    const r = menu.getBoundingClientRect();
    left = Math.max(8, Math.min(x, window.innerWidth - r.width - 8));
    top = Math.max(8, Math.min(y, window.innerHeight - r.height - 8));
    buttons()[0]?.focus();
    const outside = (e: Event) => {
      if (!menu.contains(e.target as Node)) onclose();
    };
    window.addEventListener('pointerdown', outside, true);
    window.addEventListener('resize', onclose);
    window.addEventListener('blur', onclose);
    return () => {
      window.removeEventListener('pointerdown', outside, true);
      window.removeEventListener('resize', onclose);
      window.removeEventListener('blur', onclose);
    };
  });

  function pick(item: MenuItem) {
    onclose();
    item.run();
  }

  function onkeydown(e: KeyboardEvent) {
    const all = buttons();
    const i = all.indexOf(document.activeElement as HTMLButtonElement);
    if (e.key === 'Escape') onclose();
    else if (e.key === 'ArrowDown') all[(i + 1) % all.length]?.focus();
    else if (e.key === 'ArrowUp') all[(i - 1 + all.length) % all.length]?.focus();
    else if (e.key === 'Home') all[0]?.focus();
    else if (e.key === 'End') all.at(-1)?.focus();
    else if ((e.key === 'Enter' || e.key === ' ') && i >= 0) pick(items[i]);
    else return;
    e.preventDefault();
  }
</script>

<div class="menu" role="menu" tabindex="-1" bind:this={menu} style:left="{left}px" style:top="{top}px" {onkeydown}>
  {#each items as item (item.label)}
    <button role="menuitem" tabindex="-1" class:danger={item.danger} onclick={() => pick(item)}>{item.label}</button>
  {/each}
</div>

<style>
  .menu { position: fixed; z-index: 60; min-width: 190px; padding: 5px; display: flex; flex-direction: column; background: #1c1c20; border: 1px solid var(--line); border-radius: 12px; box-shadow: 0 16px 40px #000b; animation: open 0.12s ease-out; }
  button { text-align: left; background: none; border: 0; border-radius: 8px; padding: 8px 10px; font-size: 13.5px; color: var(--text); }
  button:hover, button:focus-visible { background: var(--surface-2); outline: none; }
  button.danger { color: var(--danger-text); }
  button.danger:first-child, button.danger { margin-top: 0; }
  button.danger:not(:first-child) { border-top: 1px solid var(--line); border-radius: 0 0 8px 8px; margin-top: 4px; padding-top: 10px; }
  @keyframes open { from { opacity: 0; transform: scale(0.97); } }
</style>
