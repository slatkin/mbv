<script lang="ts">
  import { onMount } from 'svelte';
  import { frames } from '$lib/bridge/frames';
  import { installInput } from '$lib/bridge/input';
  import QueuePanel from '$lib/components/QueuePanel.svelte';
  import QueuePlaybackPanel from '$lib/components/QueuePlaybackPanel.svelte';
  import StatusBar from '$lib/components/StatusBar.svelte';

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
    <!-- The first slice's queue-only arrangement (spec
      gui-client): the Queue playback band above the Queue
      panel, the Status bar floating at the bottom. -->
    <div class="flex h-full flex-col">
      <QueuePlaybackPanel playback={$frames.slice.playback} />
      <div class="min-h-0 flex-1">
        <QueuePanel
          queue={$frames.slice.queue}
          footer={$frames.slice.footer}
          focused={$frames.slice.focus.surface === 'queue'}
        />
      </div>
      <StatusBar status={$frames.slice.status} />
    </div>
  {/if}
</main>
