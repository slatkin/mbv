<script lang="ts">
  import { onMount } from 'svelte';
  import { frames } from '$lib/bridge/frames';
  import { installInput } from '$lib/bridge/input';
  import { reportContainerSize } from '$lib/bridge/layout';
  import AttachScreen from '$lib/components/AttachScreen.svelte';
  import QueuePanel from '$lib/components/QueuePanel.svelte';
  import QueuePlaybackPanel from '$lib/components/QueuePlaybackPanel.svelte';
  import StatusBar from '$lib/components/StatusBar.svelte';

  // The webview's own context menu is turned off (add-tauri-gui-client D11).
  // Tauri has no config key for it on WebKitGTK.
  function preventContextMenu(event: MouseEvent) {
    event.preventDefault();
  }

  // The panel container the layout report carries the size of (design D10):
  // the queue column's own element, the box the container queries of the
  // panel states observe. Its ResizeObserver keeps the reported size in
  // step with the painted layout.
  let containerEl: HTMLElement | undefined = $state();

  $effect(() => {
    const el = containerEl;
    if (!el || typeof ResizeObserver === 'undefined') {
      return;
    }
    const observer = new ResizeObserver((entries) => {
      const rect = entries[0]?.contentRect;
      if (rect) {
        reportContainerSize({ width: rect.width, height: rect.height });
      }
    });
    observer.observe(el);
    return () => observer.disconnect();
  });

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
      panel, the Status bar floating over the placement's bottom
      pad row. The column carries the queue column's backdrop
      fill, the TUI's `QueueColumn` gutter the panels' padding
      sits on — focus-reactive like the TUI's
      `panel_appearance_focus` picks it, so the chrome's
      `queueColumn` pair follows the frame's Panel-focus fact. -->
    {@const panelFocused = $frames.slice.focus.panel === 'queue'}
    <div
      bind:this={containerEl}
      class="relative flex h-full flex-col"
      style:background={panelFocused
        ? 'var(--mbv-queueColumnFocused)'
        : 'var(--mbv-queueColumn)'}
    >
      <QueuePlaybackPanel
        playback={$frames.slice.playback}
        overlay={$frames.slice.overlay}
      />
      <div class="min-h-0 flex-1">
        <QueuePanel
          queue={$frames.slice.queue}
          footer={$frames.slice.footer}
          focused={$frames.slice.focus.surface === 'queue'}
          {panelFocused}
        />
      </div>
      <StatusBar status={$frames.slice.status} />
    </div>
  {:else}
    <!-- The pre-attach states (design D9): the wait before the first
      frame, the Owner attach outcomes, and the mismatch prompt. -->
    <AttachScreen
      attach={$frames && 'attach' in $frames ? $frames.attach.state : null}
    />
  {/if}
</main>
