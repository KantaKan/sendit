<script lang="ts">
  let { visible, target }: { visible: boolean; target: string | null } = $props();
</script>

{#if visible}
  <div class="overlay" aria-live="polite">
    <p>{target ? `Drop to send to ${target}` : 'Pick a device first'}</p>
  </div>
{/if}

<style>
  .overlay { position: fixed; inset: 10px; border: 2px dashed var(--accent); border-radius: 18px; background: #111113d9; display: grid; place-items: center; pointer-events: none; animation: show 0.15s ease-out; }
  p { font-size: 24px; font-weight: 700; letter-spacing: -0.02em; }
  @keyframes show { from { opacity: 0; } }
</style>
