<script lang="ts">
  import { extLabel, formatBytes, summarize } from './format';
  import type { Ticket } from './state';

  let {
    ticket, peerAlias, destLabel, onaccept, ondecline, onelsewhere,
  }: {
    ticket: Ticket;
    peerAlias: string;
    destLabel: string;
    onaccept: () => void;
    ondecline: () => void;
    onelsewhere: () => void;
  } = $props();

  const count = $derived(ticket.files.length);
  const what = $derived(count === 1 ? ticket.files[0].fileName : `${count} items`);
  const busy = $derived(ticket.status === 'accepting');
</script>

<article class="req" aria-label="Incoming files">
  <div class="fan" aria-hidden="true">
    {#each ticket.files.slice(0, 3) as f (f.id)}<span>{extLabel(f.fileName)}</span>{/each}
  </div>
  <div class="text">
    <b>{peerAlias} wants to send {what}</b>
    <span class="meta">{summarize(ticket.files)} · {formatBytes(ticket.total)}</span>
    {#if !ticket.senderVerified}<span class="warn">Unverified sender</span>{/if}
    {#if ticket.fitsOnDisk === false}<span class="warn">Not enough free space in {destLabel}</span>{/if}
  </div>
  <div class="actions">
    <div class="buttons">
      <button class="primary" onclick={onaccept} disabled={busy}>{busy ? 'Starting…' : 'Accept'}</button>
      <button class="ghost" onclick={ondecline} disabled={busy}>Decline</button>
    </div>
    <button class="dest" onclick={onelsewhere} disabled={busy} title="Save to another folder">to {destLabel}</button>
  </div>
</article>

<style>
  .req { display: grid; grid-template-columns: auto 1fr auto; gap: 14px; align-items: center; background: var(--surface); border: 1px solid #3ecf8e33; border-radius: 16px; padding: 12px 14px; animation: enter 0.35s cubic-bezier(0.22, 1, 0.36, 1); }
  .fan { position: relative; width: 72px; height: 52px; }
  .fan span { position: absolute; top: 4px; width: 44px; height: 44px; border-radius: 10px; border: 2px solid var(--surface); background: #2a2a30; display: grid; place-items: center; font: 600 10px var(--mono); color: var(--muted); transition: transform 0.35s cubic-bezier(0.22, 1, 0.36, 1); }
  .fan span:nth-child(1) { left: 0; transform: rotate(-8deg); }
  .fan span:nth-child(2) { left: 14px; }
  .fan span:nth-child(3) { left: 28px; transform: rotate(8deg); }
  .req:hover .fan span:nth-child(1) { transform: translateX(-6px) rotate(-14deg); }
  .req:hover .fan span:nth-child(3) { transform: translateX(6px) rotate(14deg); }
  .text { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .text b { font-weight: 500; font-size: 15px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .meta { font-size: 12.5px; color: var(--muted); }
  .warn { font-size: 12px; color: var(--danger-text); }
  .actions { display: flex; flex-direction: column; align-items: flex-end; gap: 6px; }
  .buttons { display: flex; gap: 8px; }
  .primary { background: var(--accent); color: var(--accent-ink); border: 0; border-radius: 10px; padding: 8px 14px; font-weight: 500; font-size: 13px; }
  .ghost { background: #2a2a30; border: 0; border-radius: 10px; padding: 8px 14px; font-size: 13px; }
  .primary:active, .ghost:active { transform: translateY(1px); }
  .dest { background: none; border: 0; padding: 0; font: 11px var(--mono); color: var(--faint); text-decoration: underline; text-underline-offset: 3px; max-width: 220px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  @keyframes enter { from { opacity: 0; transform: translateY(-6px); } }
</style>
