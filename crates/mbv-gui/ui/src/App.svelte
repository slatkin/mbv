<script lang="ts">
	import { onMount } from 'svelte';
	import { libraryArrangement, queueColumnWidth } from '$lib/bridge/arrangement';
	import { frames } from '$lib/bridge/frames';
	import { installInput } from '$lib/bridge/input';
	import { reportContainerSize } from '$lib/bridge/layout';
	import type { LayoutSize } from '$lib/bridge/types';
	import { routeLibraryKey } from '$lib/library/keys';
	import { carryLibraryContent, libraryContent, libraryShown } from '$lib/library/state';
	import AttachScreen from '$lib/components/AttachScreen.svelte';
	import LibraryPanel from '$lib/components/LibraryPanel.svelte';
	import Panel from '$lib/components/Panel.svelte';

	// The webview's own context menu is turned off (add-tauri-gui-client D11).
	// Tauri has no config key for it on WebKitGTK.
	function preventContextMenu(event: MouseEvent) {
		event.preventDefault();
	}

	// The panel container the layout report carries the size of (design D10):
	// the box the panel states are picked from. Its ResizeObserver reports the
	// size to the core and hands it to `Panel`, so the state the window paints
	// and the capacity and hit map the core applies come from one measurement.
	let containerEl: HTMLElement | undefined = $state();
	let container = $state<LayoutSize>({ width: 0, height: 0 });

	$effect(() => {
		const el = containerEl;
		if (!el || typeof ResizeObserver === 'undefined') {
			return;
		}
		const observer = new ResizeObserver((entries) => {
			const rect = entries[0]?.contentRect;
			if (rect) {
				container = { width: rect.width, height: rect.height };
				reportContainerSize(container);
			}
		});
		observer.observe(el);
		return () => observer.disconnect();
	});

	/** The last slice frame, or `null` before the first one and while the window
	 * shows an attach state. */
	function lastSlice() {
		const held = frames.frame;
		return held && 'slice' in held ? held.slice : null;
	}

	// The Library content is carried only by the frames whose revision changed
	// (add-gui-library-panel design D2), so the window keeps the last value the
	// core carried.
	$effect(() => {
		const held = $frames;
		carryLibraryContent(held && 'slice' in held ? held.slice.library : null);
	});

	onMount(() => {
		// Input forwarding and the frame subscription live for the window's
		// life (add-tauri-gui-client D4/D6); both clean up on unmount. Every
		// keydown meets the Library's router first, which takes the keys the
		// Library handles and lets the forwarder send the rest (add-gui-library-
		// panel design D3).
		const detachInput = installInput(window, (event) =>
			routeLibraryKey(event, lastSlice()),
		);
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
    <!-- The two-panel arrangement (add-gui-library-panel design D4): the Queue
      panel beside the Library panel, picked from the measured Library column
      width — the window width minus the Queue panel's current width. Mini
      mounts only the panel with Panel focus, Narrow mounts both with the Hero
      as an overlay, and Wide mounts both with the Hero pane beside the list.
      The Queue panel's four states (add-tauri-gui-client row 6.6) stay inside
      every arrangement, and its backdrop fill stays focus-reactive like the
      TUI's `panel_appearance_focus` picks it. Until Music content arrives the
      Library panel does not mount and the Queue panel paints as before. -->
    {@const frame = $frames.slice}
    {@const arrangement = libraryArrangement(container, frame.panel)}
    {@const content = $libraryContent}
    {@const music = content?.music ?? null}
    {@const queueFocused = frame.focus.panel === 'queue'}
    <!-- The Library panel's one presence fact (design D2, D10): the frame
      carries the Library section and the window holds the Music content it
      paints. The key router gates on the same fact, so a destination that
      leaves unmounts the panel and stops its keys together. -->
    {@const panelShows = libraryShown(frame.library, music)}
    {@const queueVisible = !panelShows || arrangement !== 'mini' || queueFocused}
    {@const libraryVisible = panelShows && (arrangement !== 'mini' || !queueFocused)}
    {@const twoColumns = queueVisible && libraryVisible}
    <div
      bind:this={containerEl}
      class="relative flex h-full min-h-0"
      data-arrangement={arrangement}
      style:background={queueFocused
        ? 'var(--mbv-queueColumnFocused)'
        : 'var(--mbv-queueColumn)'}
    >
      {#if queueVisible}
        <div
          class="flex h-full min-h-0 flex-col {twoColumns ? 'flex-none' : 'flex-1'}"
          style:width={twoColumns ? `${queueColumnWidth(frame.panel)}px` : undefined}
        >
          <Panel frame={frame} {container} {libraryVisible} />
        </div>
      {/if}
      {#if libraryVisible && music}
        <LibraryPanel
          {music}
          tabs={content?.tabs ?? []}
          currentTab={content?.currentTab ?? null}
          status={frame.status}
          {arrangement}
        />
      {/if}
    </div>
  {:else}
    <!-- The pre-attach states (design D9): the wait before the first
      frame, the Owner attach outcomes, and the mismatch prompt. -->
    <AttachScreen
      attach={$frames && 'attach' in $frames ? $frames.attach.state : null}
    />
  {/if}
</main>
