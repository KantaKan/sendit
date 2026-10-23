<script lang="ts">
  import Avatar from './Avatar.svelte';
  import IdentityPanel from './IdentityPanel.svelte';
  import { formatBytes, isImage, lastTwo, modKey } from './format';
  import { bytesToday, sections, type Ticket } from './state';
  import type { Store } from './store.svelte';
  import HistoryRow from './HistoryRow.svelte';
  import NowCard from './NowCard.svelte';
  import RequestCard from './RequestCard.svelte';
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
  let skipped = $state(new Set<string>());

  const sec = $derived(sections(tickets));
  const today = $derived(bytesToday(tickets, Date.now()));
  // Thumbnails for single images: our source when sending, the saved file when received.
  function thumbFor(t: Ticket): string | null {
    const name = t.files[0]?.fileName ?? '';
    const path = t.direction === 'outgoing' ? t.sources[0] : t.paths[0];
    return t.files.length === 1 && isImage(name) && path ? store.fileUrl(path) : null;
  }
</script>

<section class="conv" aria-label="Conversation with {device.alias}">
  <header>
    <Avatar fingerprint={device.fingerprint} size={44} />
    <div>
      <h2>{device.alias}</h2>
      <p class="mono">{sub}</p>
    </div>
    {#if today > 0}<p class="today">today<b>{formatBytes(today)}</b></p>{/if}
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
  <div class="list">
    {#if tickets.length === 0}
      <div class="zone" class:hot={dragging}>Drop here or <kbd>{mod}</kbd><kbd>V</kbd></div>
    {:else}
      {#each sec.requests as t (t.key)}
        <RequestCard
          ticket={t}
          peerAlias={device.alias}
          {destLabel}
          onaccept={() => t.requestId && store.accept(t.requestId)}
          ondecline={() => t.requestId && store.decline(t.requestId)}
          onelsewhere={() => t.requestId && store.acceptElsewhere(t.requestId)}
        />
      {/each}
      {#if sec.now.length}
        <p class="label"><span class="live"></span>Now</p>
        {#each sec.now as t (t.key)}
          <NowCard ticket={t} peerAlias={device.alias} thumb={thumbFor(t)} oncancel={() => t.transferId && store.cancel(t.transferId)} />
        {/each}
      {/if}
      {#if sec.history.length}
        <p class="label">Earlier</p>
        <div class="rows">
          {#each sec.history as t (t.key)}
            <HistoryRow ticket={t} thumb={thumbFor(t)} onretry={() => store.retry(t)} onreveal={store.reveal} />
          {/each}
        </div>
      {/if}
    {/if}
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
  .today { margin: 0 0 0 auto; text-align: right; font-size: 12px; color: var(--muted); }
  .today b { display: block; font: 600 15px var(--mono); color: var(--text); font-variant-numeric: tabular-nums; }
  .label { display: flex; align-items: center; gap: 8px; margin: 10px 0 0; font-size: 11.5px; color: var(--faint); }
  .live { width: 6px; height: 6px; border-radius: 50%; background: var(--accent); animation: ping 1.6s infinite; }
  @keyframes ping { 0% { box-shadow: 0 0 0 0 #3ecf8e88; } 70%, 100% { box-shadow: 0 0 0 7px #3ecf8e00; } }
  .rows { display: flex; flex-direction: column; }
  .list { flex: 1; min-height: 0; overflow-y: auto; display: flex; flex-direction: column; gap: 12px; padding: 4px 2px; }
  footer { display: flex; justify-content: space-between; align-items: center; font-size: 11px; color: var(--faint); border-top: 1px solid var(--line); padding-top: 12px; }
  .zone { margin: auto; width: min(360px, 100%); padding: 40px 22px; border: 1px dashed #ffffff1f; border-radius: 16px; text-align: center; color: var(--muted); font-size: 14px; transition: border-color 0.2s, color 0.2s, background 0.2s; }
  .zone.hot { border-color: var(--accent); color: var(--text); background: #3ecf8e0d; }
  kbd { font: 11px var(--mono); background: var(--surface-2); color: var(--text); border: 1px solid #3a3a40; border-bottom-width: 2px; border-radius: 6px; padding: 2px 7px; margin: 0 1px; }
  .notice { color: var(--text); }
  footer kbd { font-size: 10.5px; padding: 1px 6px; color: var(--muted); }
  footer button { background: var(--surface); border: 0; border-radius: 8px; padding: 6px 12px; color: var(--text); font-size: 12px; }
</style>
