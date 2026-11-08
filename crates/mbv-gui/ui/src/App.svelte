<script lang="ts">
	import { onMount } from 'svelte';
	import { frames } from '$lib/bridge/frames';
	import { installInput } from '$lib/bridge/input';
	import { reportContainerSize } from '$lib/bridge/layout';
	import type { LayoutSize } from '$lib/bridge/types';
	import { routeLibraryKey } from '$lib/library/keys';
	import { carryLibraryContent } from '$lib/library/state';
	import AttachScreen from '$lib/components/AttachScreen.svelte';
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
    <!-- The first slice's panel arrangement (spec gui-client): the panel's
      four states (add-tauri-gui-client row 6.6) over the queue column's
      backdrop fill — the TUI's `QueueColumn` gutter the panels' padding sits
      on. The fill is focus-reactive like the TUI's `panel_appearance_focus`
      picks it, so the chrome's `queueColumn` pair follows the frame's
      Panel-focus fact. -->
    {@const panelFocused = $frames.slice.focus.panel === 'queue'}
    <div
      bind:this={containerEl}
      class="relative flex h-full flex-col"
      style:background={panelFocused
        ? 'var(--mbv-queueColumnFocused)'
        : 'var(--mbv-queueColumn)'}
    >
      <Panel frame={$frames.slice} {container} />
    </div>
  {:else}
    <!-- The pre-attach states (design D9): the wait before the first
      frame, the Owner attach outcomes, and the mismatch prompt. -->
    <AttachScreen
      attach={$frames && 'attach' in $frames ? $frames.attach.state : null}
    />
  {/if}
</main>
