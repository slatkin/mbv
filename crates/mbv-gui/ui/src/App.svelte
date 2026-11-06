<script lang="ts">
  import { onMount } from 'svelte';
  import { frames } from '$lib/bridge/frames';
  import { installInput } from '$lib/bridge/input';
  import QueuePanel from '$lib/components/QueuePanel.svelte';

  // The webview's own context menu is turned off (add-tauri-gui-client D11).
  // Tauri has no config key for it on WebKitGTK.
  function preventContextMenu(event: MouseEvent) {
    event.preventDefault();
  }

  onMount(() => {
    // Input forwarding and the frame subscription live for the window's
    // life (add-tauri-gui-client D4/D6); both clean up on unmount.
    const detachInput = installInput();
    const unlisten = frames.listen();
    return () => {
      detachInput();
      void unlisten.then((off) => off());
    };
  });
</script>

<svelte:document oncontextmenu={preventContextMenu} />

<main class="h-dvh bg-background text-foreground">
  {#if $frames && 'slice' in $frames}
    <QueuePanel
      queue={$frames.slice.queue}
      footer={$frames.slice.footer}
      focused={$frames.slice.focus.surface === 'queue'}
    />
  {/if}
</main>
