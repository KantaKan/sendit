<script lang="ts">
  import { avatarUri } from './avatar';
  import { modKey, primaryIp, shortFp } from './format';
  import type { AppInfo } from './types';

  let {
    me, hasDevices, searching, onrefresh, onscan, onaddpeer,
  }: {
    me: AppInfo | null;
    hasDevices: boolean;
    searching: boolean;
    onrefresh: () => void;
    onscan: () => void;
    onaddpeer: (addr: string) => Promise<void>;
  } = $props();

  const mod = modKey();
  const ip = $derived(me ? primaryIp(me.addresses) : null);
  let showAdd = $state(false);
  let addr = $state('');
  let addError = $state<string | null>(null);
  let adding = $state(false);

  async function add(e: SubmitEvent) {
    e.preventDefault();
    adding = true;
    addError = null;
    try {
      await onaddpeer(addr.trim());
      addr = '';
    } catch (err) {
      addError = String(err);
    } finally {
      adding = false;
    }
  }
</script>

<section class="backdrop">
  <div class="rings" aria-hidden="true">
    <span class="ring"></span><span class="ring"></span><span class="ring"></span>
    {#if me}<img class="me" src={avatarUri(me.fingerprint)} alt="" />{/if}
  </div>
  <div class="content">
    {#if hasDevices}
      <h2>Pick a device on the left</h2>
      <p class="tip">Then drop files, or <kbd>{mod}</kbd><kbd>V</kbd> to send anything you copied.</p>
    {:else}
      <h2>Looking on <br />this Wi-Fi</h2>
      {#if me}
        <div class="pass">
          <div class="l"><b>{me.alias}</b><span class="mono meta">{shortFp(me.fingerprint)}</span></div>
          <div class="r">
            <span class="live mono"><i></i>VISIBLE</span>
            {#if ip}<span class="mono meta">{ip}</span>{/if}
          </div>
        </div>
      {/if}
      <ol class="steps">
        <li><span>1</span>Open LocalSend or sendit on the other device</li>
        <li><span>2</span>Join the same Wi-Fi as this computer</li>
        <li><span>3</span>On iPhone, keep LocalSend on screen</li>
      </ol>
      <div class="row">
        <button class="primary" onclick={onrefresh} disabled={searching}>Refresh</button>
        <button class="ghost" onclick={onscan} disabled={searching}>Scan network</button>
        <button class="link mono" onclick={() => (showAdd = !showAdd)} aria-expanded={showAdd}>add by IP</button>
      </div>
      {#if showAdd}
        <form onsubmit={add}>
          <label for="peer-addr">Add by IP address</label>
          <div class="row">
            <input id="peer-addr" class="mono" bind:value={addr} placeholder="192.168.1.24" autocomplete="off" />
            <button class="ghost" type="submit" disabled={adding || !addr.trim()}>Add</button>
          </div>
          {#if addError}<p class="error mono">{addError}</p>{/if}
        </form>
      {/if}
      <p class="tip">Then drop files, or <kbd>{mod}</kbd><kbd>V</kbd> to send anything you copied.</p>
    {/if}
  </div>
</section>

<style>
  .backdrop { position: relative; flex: 1; min-height: 0; overflow: hidden; display: flex; align-items: center; }
  .rings { position: absolute; right: -160px; top: 50%; width: 760px; height: 760px; margin-top: -380px; pointer-events: none; }
  .ring { position: absolute; inset: 0; border-radius: 50%; border: 1px solid #3ecf8e33; animation: pulse 3.6s ease-out infinite; }
  .ring:nth-child(2) { animation-delay: 1.2s; }
  .ring:nth-child(3) { animation-delay: 2.4s; }
  @keyframes pulse { from { transform: scale(0.2); opacity: 1; } to { transform: scale(1); opacity: 0; } }
  .me { position: absolute; left: 50%; top: 50%; width: 84px; height: 84px; margin: -42px; border-radius: 26px; box-shadow: 0 0 0 8px var(--bg); background: var(--surface); }
  .content { position: relative; display: flex; flex-direction: column; gap: 20px; padding: 0 56px; max-width: 540px; }
  h2 { margin: 0; font-size: 40px; line-height: 1.04; letter-spacing: -0.04em; }
  .pass { display: grid; grid-template-columns: 1fr auto; background: var(--surface); border: 1px solid var(--line); border-radius: 16px; overflow: hidden; }
  .pass .l { padding: 16px 18px; display: flex; flex-direction: column; min-width: 0; }
  .pass .l b { font-size: 18px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .pass .r { border-left: 1px solid var(--line); display: flex; flex-direction: column; justify-content: center; padding: 0 16px; gap: 4px; }
  .meta { font-size: 11.5px; color: var(--faint); }
  .live { color: var(--accent); font-size: 11px; font-weight: 700; display: flex; align-items: center; gap: 6px; }
  .live i { width: 7px; height: 7px; border-radius: 50%; background: var(--accent); animation: blink 1.6s ease-in-out infinite; }
  @keyframes blink { 50% { opacity: 0.25; } }
  .steps { margin: 0; padding: 0; list-style: none; display: grid; gap: 10px; font-size: 14px; color: #cfcfcc; }
  .steps li { display: grid; grid-template-columns: 22px 1fr; }
  .steps span { color: var(--accent); font: 700 12px var(--mono); }
  .row { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; }
  .primary { background: var(--accent); color: var(--accent-ink); border: 0; border-radius: 12px; padding: 11px 20px; font-weight: 500; }
  .ghost { background: var(--surface-2); border: 0; border-radius: 12px; padding: 11px 20px; }
  .primary:active, .ghost:active { transform: translateY(1px); }
  .link { background: none; border: 0; color: var(--muted); font-size: 12px; text-decoration: underline; text-underline-offset: 3px; }
  form { display: flex; flex-direction: column; gap: 8px; }
  label { font-size: 13px; color: var(--muted); }
  input { flex: 1; min-width: 0; background: var(--surface); border: 1px solid var(--line); border-radius: 10px; padding: 9px 12px; font-size: 13px; }
  .error { color: var(--danger-text); font-size: 12px; margin: 0; }
  .tip { margin: 0; font-size: 13px; color: var(--muted); }
  kbd { font: 11px var(--mono); background: var(--surface-2); color: var(--text); border: 1px solid #3a3a40; border-bottom-width: 2px; border-radius: 6px; padding: 2px 7px; margin: 0 1px; }
</style>
