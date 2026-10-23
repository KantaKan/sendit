<script lang="ts">
  import { untrack } from 'svelte';
  import { flip } from 'svelte/animate';
  import { crossfade, fly } from 'svelte/transition';
  import Avatar from './Avatar.svelte';
  import { ease, move, reduce } from './motion';
  import { shortFp } from './format';
  import type { AppInfo, Device, KnownDevice } from './types';

  let {
    me, nearby, seenBefore, selected, searching, waiting, onselect, onrefresh, settingsOpen, onsettings,
  }: {
    me: AppInfo | null;
    nearby: Device[];
    seenBefore: KnownDevice[];
    selected: string | null;
    searching: boolean;
    /** fingerprint -> requests waiting for an answer */
    waiting: Record<string, number>;
    onselect: (fp: string) => void;
    onrefresh: () => void;
    settingsOpen: boolean;
    onsettings: () => void;
  } = $props();

  // A device moving between "nearby" and "seen before" slides across; new ones drop in.
  const [send, receive] = crossfade({
    duration: reduce ? 150 : 280,
    easing: ease,
    fallback: (node) => fly(node, { y: move(-6), duration: 220, easing: ease }),
  });
  const glide = { duration: reduce ? 0 : 280, easing: ease };

  // One highlight that slides to the selected row instead of jumping.
  const rows: Record<string, HTMLElement | null> = {};
  let pill = $state<{ y: number; h: number } | null>(null);
  let pillGlides = $state(false);
  $effect(() => {
    void [nearby.length, seenBefore.length];
    const el = selected && !settingsOpen ? rows[selected] : null;
    const next = el ? { y: el.offsetTop, h: el.offsetHeight } : null;
    pillGlides = untrack(() => pill) !== null && next !== null;
    pill = next;
  });

  function sub(d: Device): string {
    if (d.trust === 'suspect') return 'key changed';
    if (d.trust === 'blocked') return 'blocked';
    if (d.trust === 'verified') return 'verified';
    return d.isSendit ? 'sendit' : 'localsend';
  }
</script>

<nav aria-label="Devices">
  <span class="pill" class:show={pill} class:glide={pillGlides} aria-hidden="true"
    style:transform="translateY({pill?.y ?? 0}px)" style:height="{pill?.h ?? 0}px"></span>
  <div class="brand">sendit</div>
  {#if nearby.length}
    <div class="grouprow">
      <p class="group mono">nearby</p>
      <button class="link mono" onclick={onrefresh} disabled={searching}>{searching ? 'Looking…' : 'Refresh'}</button>
    </div>
    <ul>
      {#each nearby as d (d.fingerprint)}
        <li in:receive={{ key: d.fingerprint }} out:send={{ key: d.fingerprint }} animate:flip={glide}>
          <button
            bind:this={rows[d.fingerprint]}
            class="dev"
            class:on={d.fingerprint === selected}
            aria-current={d.fingerprint === selected ? 'true' : undefined}
            onclick={() => onselect(d.fingerprint)}
          >
            <Avatar fingerprint={d.fingerprint} />
            <span class="text">
              <b>{d.alias}</b>
              <small class="mono" class:danger={d.trust === 'suspect' || d.trust === 'blocked'}>{sub(d)}</small>
            </span>
            {#if waiting[d.fingerprint]}
              <span class="badge mono" aria-label="{waiting[d.fingerprint]} waiting">{waiting[d.fingerprint]}</span>
            {/if}
          </button>
        </li>
      {/each}
    </ul>
  {/if}
  {#if seenBefore.length}
    <p class="group mono">seen before</p>
    <ul>
      {#each seenBefore as k (k.fingerprint)}
        <li in:receive={{ key: k.fingerprint }} out:send={{ key: k.fingerprint }} animate:flip={glide}>
          <button
            bind:this={rows[k.fingerprint]}
            class="dev off"
            class:on={k.fingerprint === selected}
            aria-current={k.fingerprint === selected ? 'true' : undefined}
            onclick={() => onselect(k.fingerprint)}
          >
            <Avatar fingerprint={k.fingerprint} />
            <span class="text"><b>{k.alias}</b><small class="mono">not nearby</small></span>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
  <div class="me">
    {#if me}
      <Avatar fingerprint={me.fingerprint} size={28} />
      <span class="text"><b>{me.alias}</b><small class="mono">{shortFp(me.fingerprint)}</small></span>
    {/if}
    <button class="gear" class:on={settingsOpen} aria-pressed={settingsOpen} onclick={onsettings}>Settings</button>
  </div>
</nav>

<style>
  nav { position: relative; display: flex; flex-direction: column; padding: 20px 12px 12px; border-right: 1px solid var(--line); min-height: 0; overflow-y: auto; }
  .brand { font-weight: 700; font-size: 20px; letter-spacing: -0.02em; margin: 0 8px 18px; }
  .group { font-size: 11px; color: var(--faint); margin: 12px 8px 6px; }
  ul { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 2px; }
  .dev { display: grid; grid-template-columns: 38px 1fr auto; gap: 10px; align-items: center; width: 100%; padding: 8px; border: 0; border-radius: 12px; background: none; text-align: left; transition: background 0.15s; }
  .dev { position: relative; z-index: 1; }
  button.dev:not(.on):hover { background: #18181b; }
  .dev.off { opacity: 0.45; }
  .dev.off.on { opacity: 0.8; }
  .pill { position: absolute; left: 12px; right: 12px; top: 0; border-radius: 12px; background: var(--surface); opacity: 0; pointer-events: none; transition: opacity 0.18s var(--ease); }
  .pill.show { opacity: 1; }
  .pill.glide { transition: opacity 0.18s var(--ease), transform 0.28s var(--ease); }
  .text { min-width: 0; display: flex; flex-direction: column; }
  .text b { font-weight: 500; font-size: 14.5px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .text small { font-size: 10.5px; color: var(--muted); }
  .badge { min-width: 20px; height: 20px; padding: 0 6px; border-radius: 10px; background: var(--accent); color: var(--accent-ink); font-size: 11px; font-weight: 700; display: grid; place-items: center; }
  .danger { color: var(--danger-text) !important; }
  .me { margin-top: auto; display: flex; align-items: center; gap: 8px; padding: 12px 8px 0; border-top: 1px solid var(--line); }
  .me .text { flex: 1; }
  .grouprow { display: flex; align-items: center; justify-content: space-between; margin-right: 8px; }
  .link { background: none; border: 0; padding: 0; font-size: 11px; color: var(--faint); }
  .link:hover:not(:disabled) { color: var(--text); }
  .gear { font-size: 12px; background: var(--surface); border: 0; border-radius: 8px; padding: 6px 10px; color: var(--text); }
  .gear.on { background: var(--accent); color: var(--accent-ink); }
</style>
