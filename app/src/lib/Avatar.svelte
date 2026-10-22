<script lang="ts">
  import { avatarUri } from './avatar';

  let { fingerprint, size = 38 }: { fingerprint: string; size?: number } = $props();
</script>

<img src={avatarUri(fingerprint)} alt="" width={size} height={size} style:border-radius="{Math.round(size * 0.3)}px" />

<style>
  img { display: block; flex: none; background: var(--surface); }
</style>
