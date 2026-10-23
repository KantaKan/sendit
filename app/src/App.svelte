<script lang="ts">
  import { onMount } from 'svelte';
  import type { Bridge } from './lib/bridge';
  import Conversation from './lib/Conversation.svelte';
  import DropOverlay from './lib/DropOverlay.svelte';
  import EmptyState from './lib/EmptyState.svelte';
  import PortBanner from './lib/PortBanner.svelte';
  import Settings from './lib/Settings.svelte';
  import Sidebar from './lib/Sidebar.svelte';
  import { nearby, seenBefore, ticketsFor } from './lib/state';
  import { createStore } from './lib/store.svelte';

  let { bridge }: { bridge: Bridge } = $props();
  // The bridge is fixed for the app's lifetime; read it once.
  // svelte-ignore state_referenced_locally
  const store = createStore(bridge);
  // Fade the window in once it has something to show; never wait longer than this.
  let late = $state(false);
  onMount(() => {
    const lateTimer = setTimeout(() => (late = true), 600);
    const stop = store.start();
    // Ctrl/Cmd+V anywhere except text fields sends the clipboard.
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && store.page === 'settings') {
        store.closeSettings();
        return;
      }
      if (!(e.ctrlKey || e.metaKey) || e.key.toLowerCase() !== 'v') return;
      const t = e.target as HTMLElement | null;
      if (t && (t.isContentEditable || t.tagName === 'INPUT' || t.tagName === 'TEXTAREA')) return;
      e.preventDefault();
      void store.paste();
    };
    window.addEventListener('keydown', onKey);
    const onContext = (e: MouseEvent) => {
      const t = e.target as HTMLElement | null;
      if (!(t && (t.isContentEditable || t.tagName === 'INPUT' || t.tagName === 'TEXTAREA'))) e.preventDefault();
    };
    window.addEventListener('contextmenu', onContext);
    return () => {
      clearTimeout(lateTimer);
      stop();
      window.removeEventListener('keydown', onKey);
      window.removeEventListener('contextmenu', onContext);
    };
  });

  const near = $derived(nearby(store.state));
  // Each screen change replays a short entrance on the new content.
  const view = $derived(store.page === 'settings' ? 'settings' : (store.selectedDevice?.fingerprint ?? 'home'));
  const waiting = $derived(
    store.state.tickets.reduce<Record<string, number>>((acc, t) => {
      if (t.status === 'pending') acc[t.peer] = (acc[t.peer] ?? 0) + 1;
      return acc;
    }, {}),
  );
</script>

<div class="shell" class:ready={store.ready || late}>
  <Sidebar
    me={store.me}
    nearby={near}
    seenBefore={seenBefore(store.state)}
    selected={store.selected}
    searching={store.searching}
    {waiting}
    onselect={store.select}
    onrefresh={store.refresh}
    settingsOpen={store.page === 'settings'}
    onsettings={() => (store.page === 'settings' ? store.closeSettings() : store.openSettings())}
  />
  <main>
    {#if store.me?.portFallback}<PortBanner port={store.me.port} />{/if}
    {#if store.error}
      <div class="error" role="alert">
        <span>{store.error}</span>
        <button onclick={store.dismissError}>Dismiss</button>
      </div>
    {/if}
    {#key view}
    <div class="view">
    {#if store.page === 'settings'}
      <Settings {store} />
    {:else if store.selectedDevice}
      <Conversation
        device={store.selectedDevice}
        tickets={ticketsFor(store.state, store.selectedDevice.fingerprint)}
        me={store.me}
        oldFingerprint={store.state.identityChanges[store.selectedDevice.fingerprint]}
        {store}
        dragging={store.dragging}
      />
    {:else}
      <EmptyState
        me={store.me}
        hasDevices={near.length > 0}
        searching={store.searching}
        onrefresh={store.refresh}
        onscan={store.scan}
        onaddpeer={store.addPeer}
      />
    {/if}
    </div>
    {/key}
  </main>
  <DropOverlay visible={store.dragging} target={store.selectedDevice?.alias ?? null} />
</div>

<style>
  .shell { display: grid; grid-template-columns: 240px 1fr; height: 100%; opacity: 0; transition: opacity 0.24s var(--ease); }
  .shell.ready { opacity: 1; }
  .view { flex: 1; min-height: 0; display: flex; flex-direction: column; animation: rise 0.2s var(--ease); }
  main { display: flex; flex-direction: column; min-width: 0; min-height: 0; }
  .error { animation: drop 0.22s var(--ease); display: flex; justify-content: space-between; align-items: center; gap: 12px; margin: 12px 20px 0; padding: 10px 14px; border-radius: 12px; background: var(--danger-bg); border: 1px solid #e5533d66; font-size: 13.5px; }
  .error button { background: none; border: 0; color: var(--danger-text); }
</style>
