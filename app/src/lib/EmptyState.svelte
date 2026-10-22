<script lang="ts">
  let {
    hasDevices, searching, onrefresh, onscan, onaddpeer,
  }: {
    hasDevices: boolean;
    searching: boolean;
    onrefresh: () => void;
    onscan: () => void;
    onaddpeer: (addr: string) => Promise<void>;
  } = $props();

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

<section class="empty">
  {#if hasDevices}
    <h2>Pick a device on the left to start.</h2>
    <p>Then drop files anywhere in this window to send them.</p>
  {:else}
    <h2>{searching ? 'Looking for devices…' : 'No devices yet'}</h2>
    <p>Devices on the same Wi-Fi show up here. Open LocalSend or sendit on your phone or another computer.</p>
    <div class="row">
      <button class="primary" onclick={onrefresh} disabled={searching}>Refresh</button>
      <button class="ghost" onclick={onscan} disabled={searching}>Scan network</button>
    </div>
  {/if}
  <form onsubmit={add}>
    <label for="peer-addr">Add by IP address</label>
    <div class="row">
      <input id="peer-addr" class="mono" bind:value={addr} placeholder="192.168.1.24" autocomplete="off" />
      <button class="ghost" type="submit" disabled={adding || !addr.trim()}>Add</button>
    </div>
    {#if addError}<p class="error mono">{addError}</p>{/if}
  </form>
</section>

<style>
  .empty { max-width: 460px; padding: 56px 40px; display: flex; flex-direction: column; gap: 14px; }
  h2 { margin: 0; font-size: 26px; letter-spacing: -0.03em; }
  p { margin: 0; color: var(--muted); line-height: 1.5; }
  .row { display: flex; gap: 8px; }
  form { margin-top: 18px; display: flex; flex-direction: column; gap: 8px; }
  label { font-size: 13px; color: var(--muted); }
  input { flex: 1; background: var(--surface); border: 1px solid var(--line); border-radius: 10px; padding: 9px 12px; font-size: 13px; }
  .primary { background: var(--accent); color: var(--accent-ink); border: 0; border-radius: 10px; padding: 9px 16px; font-weight: 500; }
  .ghost { background: var(--surface-2); border: 0; border-radius: 10px; padding: 9px 16px; }
  .error { color: var(--danger-text); font-size: 12px; }
</style>
