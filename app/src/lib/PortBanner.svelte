<script lang="ts">
  let { port }: { port: number } = $props();
</script>

<p class="banner" role="status">
  Port 53317 is busy (is LocalSend open on this computer?). sendit is using port <span class="mono">{port}</span>. Other devices can still find it.
</p>

<style>
  .banner { margin: 12px 20px 0; padding: 10px 14px; border-radius: 12px; background: var(--surface); color: var(--muted); font-size: 13px; }
</style>
