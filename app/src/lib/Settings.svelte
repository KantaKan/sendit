<script lang="ts">
  import { onMount } from 'svelte';
  import Avatar from './Avatar.svelte';
  import { shortFp } from './format';
  import { isFinished } from './history';
  import type { Store } from './store.svelte';

  let { store }: { store: Store } = $props();

  const s = $derived(store.settings);
  let alias = $state('');
  let pinOn = $state(false);
  let pin = $state('');
  let aliasError = $state<string | null>(null);
  let dirError = $state<string | null>(null);
  let pinError = $state<string | null>(null);
  let autostart = $state(false);
  let confirming = $state(false);

  // Fields show the saved values whenever those change (load, or a save elsewhere).
  $effect(() => {
    if (!s) return;
    alias = s.alias;
    pinOn = s.pin !== null;
    pin = s.pin ?? '';
  });
  onMount(async () => {
    autostart = await store.autostart();
  });

  const finished = $derived(store.state.tickets.filter(isFinished).length);
  const devices = $derived(store.state.known.filter((k) => k.trust !== 'blocked'));

  async function saveAlias() {
    if (!s || alias === s.alias) return;
    aliasError = await store.updateSettings({ alias });
  }
  async function changeDir() {
    const dir = await store.pickFolder();
    if (dir) dirError = await store.updateSettings({ downloadDir: dir });
  }
  async function togglePin() {
    if (!pinOn) {
      pinOn = true; // saved once a PIN is typed
      return;
    }
    pinOn = false;
    pin = '';
    if (s?.pin !== null) pinError = await store.updateSettings({ pin: null });
    // A failed save leaves the PIN active: show it as on.
    if (pinError) pinOn = s?.pin !== null;
  }
  async function savePin() {
    const v = pin.trim() || null;
    if (!v) pinOn = false;
    if (v === (s?.pin ?? null)) return;
    pinError = await store.updateSettings({ pin: v });
    if (pinError) pinOn = s?.pin !== null || v !== null;
  }
  const enterBlurs = (e: KeyboardEvent) => {
    if (e.key === 'Enter') (e.currentTarget as HTMLInputElement).blur();
  };
</script>

<section class="page" aria-labelledby="settings-title">
  <h2 id="settings-title">Settings</h2>
  {#if s}
    <div class="sec">
      <h3>This device</h3>
      <div class="field">
        <div class="row">
          <div>Name<small>What phones and computers see</small></div>
          <input
            class="txt"
            aria-label="Device name"
            aria-invalid={aliasError ? 'true' : undefined}
            maxlength="64"
            bind:value={alias}
            onblur={saveAlias}
            onkeydown={enterBlurs}
          />
        </div>
        {#if aliasError}<p class="err" role="alert">{aliasError}</p>{/if}
        {#if store.me}
          <div class="row"><div>Fingerprint</div><span class="mono dim">{shortFp(store.me.fingerprint)}</span></div>
        {/if}
      </div>
    </div>

    <div class="sec">
      <h3>Receiving</h3>
      <div class="field">
        <div class="row">
          <div>Save to<small class="mono">{s.downloadDir}</small></div>
          <button class="btn" onclick={changeDir}>Change…</button>
        </div>
        {#if dirError}<p class="err" role="alert">{dirError}</p>{/if}
        <div class="row">
          <div>Require a PIN<small>Senders must type it first</small></div>
          <button class="tog" class:on={pinOn} role="switch" aria-checked={pinOn} aria-label="Require a PIN" onclick={togglePin}></button>
        </div>
        {#if pinOn}
          <div class="row">
            <div>PIN</div>
            <input class="txt short mono" aria-label="PIN" bind:value={pin} onblur={savePin} onkeydown={enterBlurs} />
          </div>
        {/if}
        {#if pinError}<p class="err" role="alert">{pinError}</p>{/if}
      </div>
    </div>

    <div class="sec">
      <h3>Auto-accept</h3>
      <div class="field">
        {#each devices as k (k.fingerprint)}
          <div class="dev">
            <Avatar fingerprint={k.fingerprint} size={28} />
            <div class="grow">{k.alias}<small>{k.trust === 'verified' ? 'verified' : 'verify it first to allow'}</small></div>
            <button
              class="tog"
              class:on={k.autoAccept}
              role="switch"
              aria-checked={k.autoAccept}
              aria-label="Auto-accept {k.alias}"
              disabled={k.trust !== 'verified'}
              onclick={() => store.setAutoAccept(k.fingerprint, !k.autoAccept)}
            ></button>
          </div>
        {:else}
          <p class="dim">Devices you have connected to show up here.</p>
        {/each}
      </div>
    </div>

    <div class="sec">
      <h3>App</h3>
      <div class="field">
        <div class="row">
          <div>When I close the window</div>
          <div class="seg">
            <button aria-pressed={s.closeToTray} onclick={() => store.updateSettings({ closeToTray: true })}>Keep running in tray</button>
            <button aria-pressed={!s.closeToTray} onclick={() => store.updateSettings({ closeToTray: false })}>Quit</button>
          </div>
        </div>
        <div class="row">
          <div>Start sendit when I sign in</div>
          <button
            class="tog"
            class:on={autostart}
            role="switch"
            aria-checked={autostart}
            aria-label="Start sendit when I sign in"
            onclick={async () => (autostart = await store.setAutostart(!autostart))}
          ></button>
        </div>
      </div>
    </div>

    <div class="sec">
      <h3>History</h3>
      <div class="field">
        <div class="row">
          <div>Keeps your last 500 transfers<small>Files on disk are not touched</small></div>
          {#if confirming}
            <span class="pair">
              <button class="btn" onclick={() => (confirming = false)}>Keep</button>
              <button class="btn danger" onclick={() => { store.clearHistory(); confirming = false; }}>Yes, clear</button>
            </span>
          {:else}
            <button class="btn danger" disabled={finished === 0} onclick={() => (confirming = true)}>Clear history</button>
          {/if}
        </div>
      </div>
    </div>
  {/if}
</section>

<style>
  .page { padding: 24px 32px; overflow-y: auto; overflow-x: hidden; min-height: 0; }
  h2 { margin: 0 0 12px; font-size: 26px; letter-spacing: -0.03em; }
  .sec { display: grid; grid-template-columns: 170px 1fr; gap: 20px; padding: 16px 0; border-top: 1px solid var(--line); }
  .sec:first-of-type { border-top: 0; }
  h3 { margin: 2px 0 0; font-size: 13px; font-weight: 500; color: var(--muted); }
  .field { display: flex; flex-direction: column; gap: 12px; min-width: 0; }
  .row { display: flex; align-items: center; justify-content: space-between; gap: 12px; font-size: 14px; }
  .row > div { flex: 1; min-width: 0; }
  small { display: block; margin-top: 2px; font-size: 12px; color: var(--faint); overflow-wrap: anywhere; }
  .txt { flex: none; width: 240px; padding: 9px 12px; border-radius: 10px; border: 1px solid var(--line); background: var(--surface); color: var(--text); font: inherit; font-size: 14px; }
  .txt.short { width: 120px; }
  .txt:focus { outline: none; border-color: var(--accent); }
  .txt[aria-invalid='true'] { border-color: var(--danger); }
  .err { margin: -4px 0 0; font-size: 12.5px; color: var(--danger-text); text-align: right; }
  .btn { flex: none; padding: 8px 14px; border: 0; border-radius: 10px; background: var(--surface-2); color: var(--text); font: inherit; font-size: 13px; font-weight: 500; }
  .btn.danger { background: var(--danger-bg); color: var(--danger-text); }
  .btn:disabled { opacity: 0.4; }
  .pair { display: flex; gap: 8px; }
  .tog { flex: none; position: relative; width: 38px; height: 22px; padding: 0; border: 0; border-radius: 11px; background: var(--surface-2); transition: background 0.2s cubic-bezier(0.22, 1, 0.36, 1); }
  .tog::after { content: ''; position: absolute; left: 3px; top: 3px; width: 16px; height: 16px; border-radius: 50%; background: #cfcfcc; transition: transform 0.2s cubic-bezier(0.22, 1, 0.36, 1); }
  .tog.on { background: var(--accent); }
  .tog.on::after { transform: translateX(16px); background: var(--accent-ink); }
  .tog:disabled { opacity: 0.4; }
  .seg { display: inline-flex; flex: none; padding: 3px; border-radius: 10px; border: 1px solid var(--line); background: var(--surface); }
  .seg button { padding: 6px 12px; border: 0; border-radius: 8px; background: none; color: var(--muted); font: inherit; font-size: 13px; }
  .seg button[aria-pressed='true'] { background: var(--surface-2); color: var(--text); }
  .dev { display: flex; align-items: center; gap: 10px; font-size: 14px; }
  .grow { flex: 1; min-width: 0; }
  .dim { color: var(--faint); font-size: 13px; margin: 0; }
  .mono { font-family: var(--mono); }
  /* Narrow windows: section title above its fields. */
  @media (max-width: 860px) {
    .sec { grid-template-columns: 1fr; gap: 10px; }
  }
</style>
