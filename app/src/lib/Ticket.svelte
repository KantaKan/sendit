<script lang="ts">
  import { clock, formatBytes, percent } from './format';
  import { reasonText } from './reasons';
  import type { Ticket } from './state';

  let {
    ticket, destLabel, onaccept, ondecline, onelsewhere, oncancel, onretry, onreveal,
  }: {
    ticket: Ticket;
    destLabel: string;
    onaccept: () => void;
    ondecline: () => void;
    onelsewhere: () => void;
    oncancel: () => void;
    onretry: () => void;
    onreveal: (path: string) => void;
  } = $props();

  const out = $derived(ticket.direction === 'outgoing');
  let copied = $state(false);
  async function copy() {
    await navigator.clipboard.writeText(ticket.text ?? '');
    copied = true;
    setTimeout(() => (copied = false), 1500);
  }
  const count = $derived(ticket.files.length);
  const title = $derived(count === 1 ? ticket.files[0].fileName : `${count} files`);
  const pct = $derived(percent(ticket.bytes, ticket.total));
  const speed = $derived(ticket.bytes / Math.max(0.5, (Date.now() - ticket.at) / 1000));
  const meta = $derived.by(() => {
    if (ticket.status === 'live') return `${formatBytes(ticket.bytes)} / ${formatBytes(ticket.total)}  ${formatBytes(speed)}/s`;
    if (ticket.status === 'done') return `${formatBytes(ticket.total)}  ${out ? 'sent' : 'received'} ${clock(ticket.at)}`;
    return formatBytes(ticket.total);
  });
  const stub = $derived(
    ticket.status === 'live' ? `${pct}%` : ticket.status === 'done' ? 'DONE' : ticket.status.toUpperCase(),
  );
</script>

{#if ticket.status === 'pending' || ticket.status === 'accepting'}
  <article class="request" aria-label="Incoming files">
    <p class="head">
      <b>wants to send {count === 1 ? '1 file' : `${count} files`}</b>
      <span class="mono dim">{formatBytes(ticket.total)}</span>
    </p>
    {#if !ticket.senderVerified}<p class="note mono">Unverified sender</p>{/if}
    <ul class="files mono">
      {#each ticket.files.slice(0, 4) as f (f.id)}
        <li><span class="name">{f.fileName}</span><span>{formatBytes(f.size)}</span></li>
      {/each}
      {#if count > 4}<li class="dim">and {count - 4} more</li>{/if}
    </ul>
    {#if ticket.fitsOnDisk === false}<p class="warn">Not enough free space in {destLabel}</p>{/if}
    <div class="actions">
      <button class="primary" onclick={onaccept} disabled={ticket.status === 'accepting'}>
        {ticket.status === 'accepting' ? 'Starting…' : 'Accept'}
      </button>
      <button class="ghost" onclick={ondecline} disabled={ticket.status === 'accepting'}>Decline</button>
      <button class="dest mono" onclick={onelsewhere} disabled={ticket.status === 'accepting'} title="Save to another folder">to {destLabel}</button>
    </div>
  </article>
{:else if ticket.status === 'message'}
  <div class="row">
    <article class="message" aria-label="Message">
      <p class="text">{ticket.text}</p>
      <p class="mono meta">{clock(ticket.at)}{ticket.senderVerified ? '' : '  unverified sender'}</p>
    </article>
    <div class="under"><button class="link" onclick={copy}>{copied ? 'Copied' : 'Copy'}</button></div>
  </div>
{:else}
  <div class="row" class:out>
    <article
      class="ticket"
      class:out
      class:live={ticket.status === 'live'}
      class:muted={ticket.status !== 'live' && ticket.status !== 'done'}
      style:--fill="{pct}%"
    >
      {#if ticket.status === 'live'}<div class="fill" aria-hidden="true"></div>{/if}
      <div class="body">
        <b class="name">{title}</b>
        <span class="mono meta">{meta}</span>
      </div>
      <div class="stub mono">{stub}</div>
    </article>
    {#if ticket.status === 'failed' && ticket.reason}<p class="reason">{reasonText(ticket.reason)}</p>{/if}
    <div class="under">
      {#if ticket.status === 'live'}<button class="link" onclick={oncancel}>Cancel</button>{/if}
      {#if ticket.status === 'done' && !out && ticket.paths[0]}
        <button class="link" onclick={() => onreveal(ticket.paths[0])}>Show in folder</button>
      {/if}
      {#if ticket.status === 'failed' && out && ticket.sources.length}
        <button class="link" onclick={onretry}>Retry</button>
      {/if}
    </div>
  </div>
{/if}

<style>
  .request { width: min(380px, 100%); border-radius: 14px; background: #1a1a1d; border: 1.5px dashed #3ecf8e88; padding: 14px 16px; animation: enter 0.25s ease-out; }
  .head { display: flex; justify-content: space-between; gap: 12px; margin: 0; }
  .head b { font-weight: 500; font-size: 15px; }
  .note { margin: 6px 0 0; font-size: 11px; color: var(--danger-text); }
  .files { list-style: none; margin: 8px 0 12px; padding: 0; font-size: 11px; color: #a8a8ae; line-height: 1.7; }
  .files li { display: flex; justify-content: space-between; gap: 12px; }
  .name { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .warn { margin: 0 0 10px; font-size: 12.5px; color: var(--danger-text); }
  .dim { color: var(--faint); }
  .actions { display: flex; gap: 8px; align-items: center; }
  .primary { background: var(--accent); color: var(--accent-ink); border: 0; border-radius: 10px; padding: 8px 14px; font-weight: 500; font-size: 13px; }
  .primary:active, .ghost:active { transform: translateY(1px); }
  .ghost { background: #26262b; border: 0; border-radius: 10px; padding: 8px 14px; font-size: 13px; }
  .dest { margin-left: auto; background: none; border: 0; font-size: 10.5px; color: var(--muted); text-decoration: underline; text-underline-offset: 3px; max-width: 45%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

  .row { display: flex; flex-direction: column; align-items: flex-start; gap: 4px; animation: enter 0.25s ease-out; }
  .row.out { align-items: flex-end; }
  .ticket {
    --cut: 76px;
    position: relative; width: min(340px, 100%); display: grid; grid-template-columns: 1fr var(--cut);
    border-radius: 14px; background: var(--surface-2); overflow: hidden;
    -webkit-mask: radial-gradient(circle 8px at calc(100% - var(--cut)) 0, #0000 98%, #000) top/100% 51% no-repeat,
      radial-gradient(circle 8px at calc(100% - var(--cut)) 100%, #0000 98%, #000) bottom/100% 51% no-repeat;
    mask: radial-gradient(circle 8px at calc(100% - var(--cut)) 0, #0000 98%, #000) top/100% 51% no-repeat,
      radial-gradient(circle 8px at calc(100% - var(--cut)) 100%, #0000 98%, #000) bottom/100% 51% no-repeat;
  }
  .ticket.out { background: var(--accent); color: var(--accent-ink); }
  .ticket.live { background: #1c2b24; color: var(--text); }
  .ticket.muted { opacity: 0.6; }
  .fill { position: absolute; inset: 0 auto 0 0; width: var(--fill); background: var(--accent); opacity: 0.22; transition: width 0.3s ease-out; }
  .body { position: relative; padding: 11px 14px; display: flex; flex-direction: column; min-width: 0; }
  .body b { font-weight: 500; font-size: 15px; }
  .meta { font-size: 10.5px; opacity: 0.65; white-space: pre; }
  .stub { position: relative; border-left: 1.5px dashed #ffffff22; display: grid; place-items: center; font-size: 11px; font-weight: 700; }
  .ticket.out .stub { border-color: #0d1f1633; }
  .ticket.live .stub { color: var(--accent); border-color: #3ecf8e44; }
  .message { max-width: min(420px, 100%); background: var(--surface-2); border-radius: 14px 14px 14px 4px; padding: 10px 14px; }
  .message .text { margin: 0; white-space: pre-wrap; overflow-wrap: anywhere; line-height: 1.45; user-select: text; }
  .message .meta { margin: 6px 0 0; }
  .reason { margin: 0; font-size: 12px; color: var(--danger-text); }
  .under { display: flex; gap: 12px; }
  .link { background: none; border: 0; padding: 0; font-size: 12px; color: var(--muted); text-decoration: underline; text-underline-offset: 3px; }
  @keyframes enter { from { opacity: 0; transform: translateY(6px); } }
</style>
