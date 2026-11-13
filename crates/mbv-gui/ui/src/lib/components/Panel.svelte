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
	// The artwork is not held here: every playback section carries the art
	// reference (design D7), so each state reads it from the section, and a
	// state that mounts later paints the same reference.
	import { panelState } from '$lib/bridge/panel';
	import { panel, playback, summary } from '$lib/bridge/sections';
	import type { LayoutSize } from '$lib/bridge/types';
	import CollapsedRail from './CollapsedRail.svelte';
	import CompactStrip from './CompactStrip.svelte';
	import FoldedQueue from './FoldedQueue.svelte';
	import QueueFooter from './QueueFooter.svelte';
	import QueuePanel from './QueuePanel.svelte';
	import QueuePlaybackPanel from './QueuePlaybackPanel.svelte';

	interface Props {
		/** The panel container's measured painted size (design D10), as `App`
		 * measures and reports it. */
		container: LayoutSize;
		/** Whether the Library panel shows beside this one (design D10): the
		 * Library Status bar owns the armed prefix and live toast then, so the
		 * queue footer line paints neither. */
		libraryVisible: boolean;
	}
	const { container, libraryVisible }: Props = $props();

	/** The state the panel paints: the toggles and the container size (design
	 * D10). */
	const layout = $derived(panelState(container, $panel));

	/** The compact strip's next-item title: the core's summary names the
	 * slot after the playhead independently of the painted viewport (review
	 * of this row, finding P2) — the strip paints no Queue list, so its own
	 * rows cannot name it. */
	const next = $derived($summary.nextTitle);
</script>

{#if layout === 'rail'}
	<!-- The collapsed rail (`z`): the ring, the transport, the remaining
		time, and the control that expands the panel again. -->
	<CollapsedRail art={$playback.artwork} />
{:else if layout === 'compactStrip'}
	<!-- The compact strip: the container is shorter than 520 px. The strip
		paints from the playback and summary sections; the queue and the footer
		are absent, and the core keeps their state. -->
	<div class="flex h-full min-h-0 flex-col" data-panel-state={layout}>
		<CompactStrip {next} art={$playback.artwork} />
	</div>
{:else}
	<!-- The expanded artboard, with the queue rows or (`q`) their one-line
		summary. -->
	<div class="flex h-full min-h-0 flex-col" data-panel-state={layout}>
		<QueuePlaybackPanel />
		<div class="min-h-0 flex-1">
			{#if layout === 'expandedWithoutQueue'}
				<FoldedQueue />
			{:else}
				<QueuePanel />
			{/if}
		</div>
		<QueueFooter {libraryVisible} />
	</div>
{/if}
