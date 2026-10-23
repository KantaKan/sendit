<script lang="ts">
  import { bytesOf, eta, extLabel } from './format';
  import type { Ticket } from './state';

  let {
    ticket, peerAlias, oncancel, thumb = null,
  }: { ticket: Ticket; peerAlias: string; oncancel: () => void; thumb?: string | null } = $props();

  let broken = $state(false);
  const isMessage = $derived(ticket.text !== undefined);
  const out = $derived(ticket.direction === 'outgoing');
  const title = $derived(
    isMessage ? (ticket.text ?? '') : ticket.files.length === 1 ? ticket.files[0].fileName : `${ticket.files.length} files`,
  );
  const pct = $derived(ticket.total === 0 ? 0 : Math.min(100, Math.floor((ticket.bytes * 100) / ticket.total)));
  const speed = $derived(ticket.speeds?.at(-1) ?? 0);
  const meta = $derived.by(() => {
    if (ticket.status === 'sending') return 'Sending…';
    if (ticket.bytes === 0) return out ? `Waiting for ${peerAlias} to accept` : 'Starting…';
    return `${bytesOf(ticket.bytes, ticket.total)} · ${(speed / 1e6).toFixed(1)} MB/s · ${eta((ticket.total - ticket.bytes) / speed)} left`;
  });
  // ring around the thumbnail: r = 31 -> circumference 194.8
  const C = 194.8;
  const spark = $derived.by(() => {
    const v = ticket.speeds ?? [];
    if (v.length < 2) return '';
    const max = Math.max(...v) || 1;
    return v.map((x, i) => `${i ? 'L' : 'M'}${((i / (v.length - 1)) * 120).toFixed(1)} ${(26 - (x / max) * 22).toFixed(1)}`).join(' ');
  });
</script>

<article class="now" aria-label={title}>
  {#if isMessage}
    <div class="tile quote" aria-hidden="true">“</div>
  {:else}
    <div class="tile" role="progressbar" aria-valuenow={pct} aria-valuemin={0} aria-valuemax={100} aria-label="{title} progress">
      {#if thumb && !broken}<img src={thumb} alt="" onerror={() => (broken = true)} />{:else}<span>{extLabel(ticket.files[0]?.fileName ?? '')}</span>{/if}
      <svg viewBox="0 0 66 66" aria-hidden="true">
        <circle class="track" cx="33" cy="33" r="31" />
        <circle class="arc" cx="33" cy="33" r="31" stroke-dasharray={C} stroke-dashoffset={C * (1 - pct / 100)} />
      </svg>
    </div>
  {/if}
  <div class="text">
    <b>{title}</b>
    <span class="meta">
      <span class="dir" class:out aria-label={out ? 'sent' : 'received'}>{out ? '↗' : '↙'}</span>{meta}
    </span>
  </div>
  <div class="side">
    {#if spark}
      <svg class="spark" viewBox="0 0 120 28" aria-hidden="true"><path class="area" d="{spark} L120 28 L0 28 Z" /><path d={spark} /></svg>
    {/if}
    {#if !isMessage}<button class="link" onclick={oncancel}>Cancel</button>{/if}
  </div>
</article>

<style>
  .now { display: grid; grid-template-columns: 56px 1fr auto; gap: 14px; align-items: center; background: var(--surface); border-radius: 16px; padding: 12px 14px; animation: enter 0.4s cubic-bezier(0.2, 0.9, 0.3, 1.15); }
  .tile { position: relative; width: 56px; height: 56px; }
  .tile img, .tile span, .quote { width: 56px; height: 56px; border-radius: 14px; object-fit: cover; display: grid; place-items: center; background: #2a2a30; font: 600 11px var(--mono); color: var(--muted); }
  .quote { font: 22px var(--sans); }
  .tile svg { position: absolute; inset: -5px; width: 66px; height: 66px; transform: rotate(-90deg); }
  circle { fill: none; stroke-width: 2.5; }
  .track { stroke: #ffffff10; }
  .arc { stroke: var(--accent); stroke-linecap: round; transition: stroke-dashoffset 0.4s ease-out; }
  .text { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .text b { font-weight: 500; font-size: 15px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .meta { font-size: 12.5px; color: var(--muted); font-variant-numeric: tabular-nums; }
  .dir { font-family: var(--mono); font-size: 11px; margin-right: 6px; color: var(--in); }
  .dir.out { color: var(--accent); }
  .side { display: flex; flex-direction: column; align-items: flex-end; gap: 4px; }
  .spark { width: 120px; height: 28px; }
  .spark path { fill: none; stroke: var(--accent); stroke-width: 1.5; }
  .spark .area { fill: #3ecf8e14; stroke: none; }
  .link { background: none; border: 0; padding: 0; font-size: 12px; color: var(--muted); }
  .link:hover { color: var(--text); }
  @keyframes enter { from { opacity: 0; transform: translateY(6px); } }
</style>
