<script lang="ts">
  import Avatar from './Avatar.svelte';
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

  function sub(d: Device): string {
    if (d.trust === 'suspect') return 'key changed';
    if (d.trust === 'blocked') return 'blocked';
    if (d.trust === 'verified') return 'verified';
    return d.isSendit ? 'sendit' : 'localsend';
  }
</script>

<nav aria-label="Devices">
  <div class="brand">sendit</div>
  {#if nearby.length}
    <div class="grouprow">
      <p class="group mono">nearby</p>
      <button class="link mono" onclick={onrefresh} disabled={searching}>{searching ? 'Looking…' : 'Refresh'}</button>
    </div>
    <ul>
      {#each nearby as d (d.fingerprint)}
        <li>
          <button
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
        <li class="dev off">
          <Avatar fingerprint={k.fingerprint} />
          <span class="text"><b>{k.alias}</b><small class="mono">not nearby</small></span>
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
  nav { display: flex; flex-direction: column; padding: 20px 12px 12px; border-right: 1px solid var(--line); min-height: 0; overflow-y: auto; }
  .brand { font-weight: 700; font-size: 20px; letter-spacing: -0.02em; margin: 0 8px 18px; }
  .group { font-size: 11px; color: var(--faint); margin: 12px 8px 6px; }
  ul { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 2px; }
  .dev { display: grid; grid-template-columns: 38px 1fr auto; gap: 10px; align-items: center; width: 100%; padding: 8px; border: 0; border-radius: 12px; background: none; text-align: left; transition: background 0.15s; }
  button.dev:hover { background: #18181b; }
  .dev.on { background: var(--surface); }
  .dev.off { opacity: 0.45; }
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
