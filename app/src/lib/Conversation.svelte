<script lang="ts">
  import Avatar from './Avatar.svelte';
  import IdentityPanel from './IdentityPanel.svelte';
  import { lastTwo, modKey } from './format';
  import type { Ticket } from './state';
  import type { Store } from './store.svelte';
  import TicketView from './Ticket.svelte';
  import VerifyPanel from './VerifyPanel.svelte';
  import type { AppInfo, Device } from './types';

  let {
    device, tickets, me, oldFingerprint, store, dragging = false,
  }: {
    device: Device;
    tickets: Ticket[];
    me: AppInfo | null;
    oldFingerprint?: string;
    store: Store;
    dragging?: boolean;
  } = $props();
  const mod = modKey();

  const destLabel = $derived(me ? lastTwo(me.downloadDir) : 'Downloads');
  const sub = $derived([device.addr.split(':')[0], device.deviceModel].filter(Boolean).join('  '));
  let list: HTMLElement | undefined = $state();
  let skipped = $state(new Set<string>());

  $effect(() => {
    void tickets.length;
    void tickets.at(-1)?.bytes;
    list?.scrollTo({ top: list.scrollHeight });
  });
</script>

<section class="conv" aria-label="Conversation with {device.alias}">
  <header>
    <Avatar fingerprint={device.fingerprint} size={44} />
    <div>
      <h2>{device.alias}</h2>
      <p class="mono">{sub}</p>
    </div>
  </header>
  {#if device.trust === 'suspect'}
    <IdentityPanel
      alias={device.alias}
      {oldFingerprint}
      newFingerprint={device.fingerprint}
      onblock={() => store.block(device.fingerprint)}
      ontrust={() => store.trustNewKey(device.fingerprint)}
    />
  {:else if device.isSendit && device.trust === 'known' && me && !skipped.has(device.fingerprint)}
    <VerifyPanel
      theirs={device.fingerprint}
      mine={me.fingerprint}
      onyes={() => store.verify(device.fingerprint)}
      onskip={() => (skipped = new Set(skipped).add(device.fingerprint))}
    />
  {/if}
  <div class="list" bind:this={list}>
    {#each tickets as t (t.key)}
      <TicketView
        ticket={t}
        {destLabel}
        peerAlias={device.alias}
        onaccept={() => t.requestId && store.accept(t.requestId)}
        ondecline={() => t.requestId && store.decline(t.requestId)}
        onelsewhere={() => t.requestId && store.acceptElsewhere(t.requestId)}
        oncancel={() => t.transferId && store.cancel(t.transferId)}
        onretry={() => store.retry(t)}
        onreveal={store.reveal}
      />
    {:else}
      <div class="zone" class:hot={dragging}>Drop here or <kbd>{mod}</kbd><kbd>V</kbd></div>
    {/each}
  </div>
  <footer class="mono">
    {#if store.notice}
      <span class="notice" role="status">{store.notice}</span>
    {:else}
      <span>drop or <kbd>{mod}</kbd> <kbd>V</kbd> to send files, screenshots or text</span>
    {/if}
    <button onclick={store.chooseFiles}>Choose files</button>
  </footer>
</section>

<style>
  .conv { display: flex; flex-direction: column; flex: 1; min-height: 0; padding: 20px 26px 16px; gap: 12px; }
  header { display: flex; align-items: center; gap: 12px; }
  h2 { margin: 0; font-size: 26px; font-weight: 700; letter-spacing: -0.03em; line-height: 1; }
  header p { margin: 4px 0 0; font-size: 11px; color: var(--muted); white-space: pre; }
  .list { flex: 1; min-height: 0; overflow-y: auto; display: flex; flex-direction: column; gap: 12px; padding: 4px 2px; }
  footer { display: flex; justify-content: space-between; align-items: center; font-size: 11px; color: var(--faint); border-top: 1px solid var(--line); padding-top: 12px; }
  .zone { margin: auto; width: min(360px, 100%); padding: 40px 22px; border: 1px dashed #ffffff1f; border-radius: 16px; text-align: center; color: var(--muted); font-size: 14px; transition: border-color 0.2s, color 0.2s, background 0.2s; }
  .zone.hot { border-color: var(--accent); color: var(--text); background: #3ecf8e0d; }
  kbd { font: 11px var(--mono); background: var(--surface-2); color: var(--text); border: 1px solid #3a3a40; border-bottom-width: 2px; border-radius: 6px; padding: 2px 7px; margin: 0 1px; }
  .notice { color: var(--text); }
  footer kbd { font-size: 10.5px; padding: 1px 6px; color: var(--muted); }
  footer button { background: var(--surface); border: 0; border-radius: 8px; padding: 6px 12px; color: var(--text); font-size: 12px; }
</style>
