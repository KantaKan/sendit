<script lang="ts">
  import { shortFp } from './format';

  let {
    alias, oldFingerprint, newFingerprint, onblock, ontrust,
  }: { alias: string; oldFingerprint?: string; newFingerprint: string; onblock: () => void; ontrust: () => void } = $props();
</script>

<aside class="warn" role="alert">
  <b>This isn't the {alias} you know</b>
  <p>Same name, different key. It could be a reinstall, or someone on this network pretending to be it. Nothing is sent until you decide.</p>
  <dl class="mono">
    <dt>before</dt><dd><s>{oldFingerprint ? shortFp(oldFingerprint) : 'unknown'}</s></dd>
    <dt>now</dt><dd>{shortFp(newFingerprint)}</dd>
  </dl>
  <div class="actions">
    <button class="ghost" onclick={onblock}>Block</button>
    <button class="danger" onclick={ontrust}>It's a reinstall, trust new key</button>
  </div>
</aside>

<style>
  .warn { background: var(--danger-bg); border: 1px solid #e5533d66; border-radius: 14px; padding: 14px 16px; max-width: 480px; }
  b { color: var(--danger-text); }
  p { margin: 6px 0 12px; font-size: 13.5px; color: #d6c9c6; line-height: 1.5; }
  dl { display: grid; grid-template-columns: 60px 1fr; margin: 0 0 12px; font-size: 11px; color: #a8a8ae; line-height: 1.7; }
  dd { margin: 0; }
  s { color: var(--faint); }
  .actions { display: flex; gap: 8px; }
  .ghost { background: #26262b; border: 0; border-radius: 10px; padding: 8px 14px; }
  .danger { background: var(--danger); color: #1a0d0b; border: 0; border-radius: 10px; padding: 8px 14px; font-weight: 500; }
</style>
