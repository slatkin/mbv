<script lang="ts">
	// The panel's four states (add-tauri-gui-client row 6.6, design D10): the
	// expanded canvas artboard, its queue folded to one line (`q`), the
	// compact strip for a container shorter than 520 px, and the 72 px rail
	// (`z`).
	//
	// The state comes from `panelState` — the measured panel container size
	// plus the Client core's two toggles — so the window picks the layout from
	// the panel's own size, never the window's, and never from interaction
	// state of its own (the `gui-client` spec). The measurement is the one
	// `App` reports to the core as `LayoutReport.containerSize`, so the
	// painted state and the core's capacity and hit map follow one size.
	//
	// The artwork is carried here, above the state branches (design D10): a
	// frame arms `playback.artwork` only when the art changed, so a state that
	// mounts on a later frame (the rail after `z`, the strip after a resize)
	// paints the value the expanded artboard already adopted instead of an
	// empty slot.
	import { nextItemTitle, panelState } from '$lib/bridge/panel';
	import type { GuiCardArtwork, GuiFrame, LayoutSize } from '$lib/bridge/types';
	import CollapsedRail from './CollapsedRail.svelte';
	import CompactStrip from './CompactStrip.svelte';
	import FoldedQueue from './FoldedQueue.svelte';
	import QueueFooter from './QueueFooter.svelte';
	import QueuePanel from './QueuePanel.svelte';
	import QueuePlaybackPanel from './QueuePlaybackPanel.svelte';

	interface Props {
		/** The attached slice's frame (design D4): every section the state
		 * paints. */
		frame: GuiFrame;
		/** The panel container's measured painted size (design D10), as `App`
		 * measures and reports it. */
		container: LayoutSize;
	}
	const { frame, container }: Props = $props();

	/** The artwork the window carries for the active item (design D10). */
	let carriedArt = $state<GuiCardArtwork | null>(null);

	/** The state the panel paints: the toggles and the container size (design
	 * D10). */
	const layout = $derived(panelState(container, frame.panel));

	/** Whether the queue Panel holds the frame's appearance focus
	 * (`GuiFocus::panel`): the TUI's `panel_appearance_focus`, which picks the
	 * focused fills of the focus-reactive queue surfaces. */
	const panelFocused = $derived(frame.focus.panel === 'queue');

	/** The compact strip's next-item title, read from the frame's viewport
	 * rows (`nextItemTitle`). */
	const next = $derived(nextItemTitle(frame.queue));

	// The artwork adoption (design D10): a frame's carried value replaces the
	// last one; a frame without one keeps painting it.
	$effect(() => {
		if (frame.playback.artwork) {
			carriedArt = frame.playback.artwork;
		}
	});
</script>

{#if layout === 'rail'}
	<!-- The collapsed rail (`z`): the ring, the transport, the remaining
		time, and the control that expands the panel again. -->
	<CollapsedRail playback={frame.playback} art={carriedArt} />
{:else if layout === 'compactStrip'}
	<!-- The compact strip: the container is shorter than 520 px. The strip
		paints from the same frame; the queue and the footer are absent, and the
		core keeps their state. -->
	<div class="flex h-full min-h-0 flex-col" data-panel-state={layout}>
		<CompactStrip playback={frame.playback} {next} art={carriedArt} />
	</div>
{:else}
	<!-- The expanded artboard, with the queue rows or (`q`) their one-line
		summary. -->
	<div class="flex h-full min-h-0 flex-col" data-panel-state={layout}>
		<QueuePlaybackPanel
			playback={frame.playback}
			overlay={frame.overlay}
			carried={carriedArt}
			queueFolded={frame.panel.queueFolded}
		/>
		<div class="min-h-0 flex-1">
			{#if layout === 'expandedWithoutQueue'}
				<FoldedQueue summary={frame.summary} />
			{:else}
				<QueuePanel
					queue={frame.queue}
					summary={frame.summary}
					overlay={frame.overlay}
					focused={frame.focus.surface === 'queue'}
					{panelFocused}
				/>
			{/if}
		</div>
		<QueueFooter footer={frame.footer} status={frame.status} />
	</div>
{/if}
