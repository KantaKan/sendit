<script lang="ts">
  import { clock, extLabel, formatBytes } from './format';
  import { reasonText } from './reasons';
  import type { Ticket } from './state';

  let {
    ticket, thumb, onretry, onreveal,
  }: { ticket: Ticket; thumb: string | null; onretry: () => void; onreveal: (path: string) => void } = $props();

  const isMessage = $derived(ticket.text !== undefined);
  const out = $derived(ticket.direction === 'outgoing');
  const title = $derived(
    isMessage ? (ticket.text ?? '') : ticket.files.length === 1 ? ticket.files[0].fileName : `${ticket.files.length} files`,
  );
  const path = $derived(out ? ticket.sources[0] : ticket.paths[0]);
  // Fresh rows get a short check-mark flash.
  const fresh = $derived.by(() => {
    if (ticket.status !== 'done' || ticket.doneAt === undefined) return false;
    const age = Date.now() - ticket.doneAt;
    return age >= 0 && age < 1500;
  });
  const status = $derived.by(() => {
    if (ticket.status === 'failed' && ticket.reason) return reasonText(ticket.reason);
    if (ticket.status === 'declined') return 'Declined';
    if (ticket.status === 'cancelled') return 'Cancelled';
    if (ticket.status === 'expired') return 'Expired';
    return '';
  });
  let copied = $state(false);
  // A moved or deleted file must not leave a broken-image icon.
  let broken = $state(false);
  async function copy() {
    await navigator.clipboard.writeText(ticket.text ?? '');
    copied = true;
    setTimeout(() => (copied = false), 1500);
  }
</script>

<div class="row" class:in={!out} class:muted={!!status}>
  {#if isMessage}
    <div class="tile quote" aria-hidden="true">“</div>
  {:else if thumb && !broken}
    <img class="tile" src={thumb} alt="" onerror={() => (broken = true)} />
    <img class="peek" src={thumb} alt="" aria-hidden="true" />
  {:else}
    <div class="tile">{extLabel(ticket.files[0]?.fileName ?? '')}</div>
  {/if}
  {#if fresh}<span class="flash" aria-hidden="true">✓</span>{/if}
  <div class="text">
    <b>{title}</b>
    <span class="meta">
      <span class="dir" class:out aria-label={out ? 'sent' : 'received'}>{out ? '↗' : '↙'}</span>{isMessage
        ? `message${!out && !ticket.senderVerified ? ' · unverified sender' : ''}`
        : formatBytes(ticket.total)}
    </span>
  </div>
  <div class="right">
    <span class="acts">
      {#if ticket.status === 'done' && path}<button class="link" onclick={() => onreveal(path)}>Show</button>{/if}
      {#if isMessage}<button class="link" onclick={copy}>{copied ? 'Copied' : 'Copy'}</button>{/if}
      {#if ticket.status === 'failed' && out}<button class="link" onclick={onretry}>Retry</button>{/if}
    </span>
    {#if status}<span class="status" class:bad={ticket.status === 'failed'}>{status}</span>{/if}
    <span class="when">{clock(ticket.doneAt ?? ticket.at)}</span>
  </div>
</div>

<style>
  .row { position: relative; display: grid; grid-template-columns: 44px 1fr auto; gap: 14px; align-items: center; padding: 9px 8px; margin: 0 -8px; border-radius: 12px; transition: background 0.15s; animation: from-right 0.45s cubic-bezier(0.2, 0.9, 0.3, 1.15); }
  .row.in { animation-name: from-left; }
  .row:hover, .row:focus-within { background: var(--surface); }
  .tile { width: 44px; height: 44px; border-radius: 11px; object-fit: cover; display: grid; place-items: center; background: #2a2a30; font: 600 10.5px var(--mono); color: var(--muted); }
  .quote { font: 20px var(--sans); }
  .muted .tile { opacity: 0.6; }
  .flash { position: absolute; left: 8px; top: 9px; width: 44px; height: 44px; border-radius: 11px; display: grid; place-items: center; background: #3ecf8ecc; color: var(--accent-ink); font-weight: 700; animation: flash 1.2s ease-out forwards; pointer-events: none; }
  .peek { position: absolute; left: 60px; bottom: 52px; width: 220px; height: 150px; border-radius: 14px; object-fit: cover; box-shadow: 0 18px 40px #000a; opacity: 0; transform: translateY(6px) scale(0.96); transition: opacity 0.2s, transform 0.2s; pointer-events: none; z-index: 2; }
  .row:hover .peek { opacity: 1; transform: none; }
  .text { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .text b { font-weight: 500; font-size: 14.5px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .meta { font-size: 12.5px; color: var(--muted); }
  .dir { font-family: var(--mono); font-size: 11px; margin-right: 6px; color: var(--in); }
  .dir.out { color: var(--accent); }
  .right { display: flex; align-items: center; gap: 12px; font-size: 12.5px; color: var(--muted); }
  .acts { display: flex; gap: 12px; opacity: 0; transform: translateX(6px); transition: opacity 0.15s, transform 0.15s; }
  .row:hover .acts, .row:focus-within .acts { opacity: 1; transform: none; }
  .status.bad { color: var(--danger-text); }
  .when { font-variant-numeric: tabular-nums; min-width: 40px; text-align: right; }
  .link { background: none; border: 0; padding: 0; font-size: 12.5px; color: var(--muted); }
  .link:hover { color: var(--text); }
  @keyframes from-right { from { opacity: 0; transform: translateX(24px); } }
  @keyframes from-left { from { opacity: 0; transform: translateX(-24px); } }
  @keyframes flash { 0% { opacity: 0; transform: scale(0.6); } 25% { opacity: 1; transform: scale(1); } 75% { opacity: 1; } 100% { opacity: 0; } }
</style>
