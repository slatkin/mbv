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
	// empty slot. The hover preview's trickplay sheet is carried the same way
	// (task 6.7): a frame re-carries the sheet's image only when the hovered
	// sheet changes.
	import { panelState } from '$lib/bridge/panel';
	import type {
		GuiCardArtwork,
		GuiFrame,
		GuiTrickplaySheet,
		LayoutSize,
	} from '$lib/bridge/types';
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

	/** The trickplay sheet the window carries for the expanded panel's hover
	 * preview (task 6.7): the image last carried, keyed by the sheet the tile
	 * names. The hover preview reuses it while the frame names the same key. */
	let carriedSheet = $state<GuiTrickplaySheet | null>(null);

	/** The state the panel paints: the toggles and the container size (design
	 * D10). */
	const layout = $derived(panelState(container, frame.panel));

	/** Whether the queue Panel holds the frame's appearance focus
	 * (`GuiFocus::panel`): the TUI's `panel_appearance_focus`, which picks the
	 * focused fills of the focus-reactive queue surfaces. */
	const panelFocused = $derived(frame.focus.panel === 'queue');

	/** The compact strip's next-item title: the core's summary names the
	 * slot after the playhead independently of the painted viewport (review
	 * of this row, finding P2) — the strip paints no Queue list, so its own
	 * rows cannot name it. */
	const next = $derived(frame.summary.nextTitle);

	// The artwork adoption (design D10): a frame's carried value replaces the
	// last one; a frame without one keeps painting it.
	$effect(() => {
		if (frame.playback.artwork) {
			carriedArt = frame.playback.artwork;
		}
	});

	// The trickplay sheet adoption (task 6.7): a frame carries the sheet's
	// image only when the hovered sheet changed, so the last one stays until
	// another key arrives.
	$effect(() => {
		const tile = frame.playback.hover?.tile;
		if (tile?.sheet) {
			carriedSheet = { key: tile.sheetKey, image: tile.sheet };
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
			carriedSheet={carriedSheet}
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
