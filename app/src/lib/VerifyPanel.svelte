<script lang="ts">
  import { avatarUri } from './avatar';

  let { theirs, mine, onyes, onskip }: { theirs: string; mine: string; onyes: () => void; onskip: () => void } = $props();
</script>

<aside class="panel" aria-label="Verify this device">
  <div class="pair">
    <figure><img src={avatarUri(theirs)} alt="Their avatar" width="72" height="72" /><figcaption class="mono">them</figcaption></figure>
    <p class="q">Same face on both screens?</p>
    <figure><img src={avatarUri(mine)} alt="Your avatar" width="72" height="72" /><figcaption class="mono">you</figcaption></figure>
  </div>
  <p class="why">Check the other screen shows these two faces. If it does, nobody is in the middle.</p>
  <div class="actions">
    <button class="primary" onclick={onyes}>Yes, trust</button>
    <button class="ghost" onclick={onskip}>Skip</button>
  </div>
</aside>

<style>
  .panel { background: #1a1a1d; border: 1px solid var(--line); border-radius: 14px; padding: 16px 18px; max-width: 480px; }
  .pair { display: flex; align-items: center; gap: 24px; }
  figure { margin: 0; text-align: center; }
  img { border-radius: 20px; background: var(--surface); display: block; }
  figcaption { font-size: 11px; color: var(--faint); margin-top: 4px; }
  .q { font-weight: 700; font-size: 17px; margin: 0; }
  .why { color: var(--muted); font-size: 13px; margin: 10px 0 12px; }
  .actions { display: flex; gap: 8px; }
  .primary { background: var(--accent); color: var(--accent-ink); border: 0; border-radius: 10px; padding: 8px 14px; font-weight: 500; }
  .ghost { background: #26262b; border: 0; border-radius: 10px; padding: 8px 14px; }
</style>
