<script lang="ts">
	// The panel's compact strip (add-tauri-gui-client row 6.6, design D10):
	// the state a container shorter than 520 px paints. The canvas's strip is
	// a 96×54 still, the title, its meta line (the parent line and the next
	// item), a thin progress line, and the transport's play/pause and next.
	//
	// The strip is size-driven, so it paints no collapse or fold control: the
	// `z` key still collapses it to the rail through the core, and the rail's
	// own control brings the panel back. Both controls report the
	// `QueuePlayback` target ids, so a press resolves through the same core
	// arms as the expanded header's controls.
	import { onMount } from 'svelte';
	import { Pause, Play, SkipForward } from '@lucide/svelte';
	import {
		elementRegion,
		observeLayout,
		reportLayout
	} from '$lib/bridge/layout';
	import { clampRatio } from '$lib/bridge/time';
	import { glyphColor, roleVar } from '$lib/bridge/theme';
	import type {
		GuiCardArtwork,
		GuiQueuePlayback,
		LayoutRegion,
		QueuePlaybackTargetId
	} from '$lib/bridge/types';

	interface Props {
		playback: GuiQueuePlayback;
		/** The next item's title (`GuiQueueSummary.nextTitle`, review of this
		 * row's finding P2), `null` while no slot follows the playhead. */
		next: string | null;
		/** The artwork the window carries for the active item (design D10). */
		art: GuiCardArtwork | null;
	}
	const { playback, next, art }: Props = $props();

	/** The still's canvas size (the spec's 96×54). */
	const STILL_WIDTH_PX = 96;
	const STILL_HEIGHT_PX = 54;

	const reportKey = Symbol('compact-strip');
	let stripEl: HTMLElement | undefined = $state();
	let stillEl: HTMLElement | undefined = $state();
	let playPauseEl: HTMLElement | undefined = $state();
	let nextEl: HTMLElement | undefined = $state();

	/** The played fraction of the thin progress line, clamped to `[0, 1]`. */
	const progress = $derived(
		clampRatio(playback.playback.positionTicks, playback.playback.runtimeTicks),
	);

	/** The artwork's bitmap, `null` while the item has none. */
	const image = $derived(art?.kind === 'image' ? art.detail : null);

	/** The meta line: the parent line and the next item, whichever the frame
	 * carries, joined as one truncating line. */
	const meta = $derived(
		[playback.parentLine, next === null ? null : `Next: ${next}`]
			.filter((part): part is string => part !== null && part.length > 0)
			.join(' · '),
	);

	/** One element's painted region for a QueuePlayback target, `null` while
	 * the element is absent. */
	const region = (
		el: HTMLElement | undefined,
		id: QueuePlaybackTargetId
	): LayoutRegion | null => elementRegion(el, { surface: 'queuePlayback', id });

	/** The painted regions (design D4): the still's slot, the transport's two
	 * controls. */
	function currentRegions(): LayoutRegion[] {
		return [
			region(stillEl, 'visualSlot'),
			region(playPauseEl, 'playPause'),
			region(nextEl, 'next'),
		].filter((entry) => entry !== null);
	}

	function report(): void {
		reportLayout(reportKey, { lists: [], regions: currentRegions() });
	}

	// Each paint that moved a region reports (design D4): this effect runs
	// after the DOM update, and the ResizeObserver reports layout-driven
	// moves. The reporter dedupes an unchanged contribution.
	$effect(() => {
		void playback;
		void next;
		void art;
		report();
	});

	onMount(() => observeLayout(reportKey, stripEl, report));
</script>

<!-- The strip: one row across the column's top, the still at the leading
	edge, the title and meta lines in the middle, and the transport at the
	trailing edge. The thin progress line sits under the meta line. -->
<div
	class="flex items-center"
	style:padding="var(--mbv-cell-row) calc(var(--mbv-cell-col) * 2)"
	style:gap="calc(var(--mbv-cell-col) * 2)"
	bind:this={stripEl}
	style:background="var(--mbv-queuePlaybackBand)"
>
	<div
		class="shrink-0 overflow-hidden rounded-sm"
		style:width="{STILL_WIDTH_PX}px"
		style:height="{STILL_HEIGHT_PX}px"
		bind:this={stillEl}
		data-target="visualSlot"
	>
		{#if image}
			<img class="h-full w-full object-cover" alt="" src={image} />
		{:else}
			<div class="h-full w-full" style:background="var(--mbv-textMuted)"></div>
		{/if}
	</div>
	<div class="flex min-w-0 flex-1 flex-col" style:gap="calc(var(--mbv-cell-row) / 4)">
		<span
			class="truncate font-semibold"
			style:color={roleVar('playbackTitleFg')}>{playback.nowPlayingTitle ?? ''}</span
		>
		{#if meta}
			<span class="truncate text-xs" style:color={roleVar('playbackContextFg')}>{meta}</span>
		{/if}
		<!-- The thin progress line: the played fraction over the track. -->
		<span
			class="block overflow-hidden rounded-full"
			style:height="2px"
			style:background="var(--mbv-progressTrack)"
			data-progress
		>
			<span
				class="block h-full rounded-full"
				style:width="{progress * 100}%"
				style:background="var(--mbv-accent)"
			></span>
		</span>
	</div>
	<button
		type="button"
		class="flex shrink-0 items-center justify-center rounded-full p-1"
		bind:this={playPauseEl}
		data-target="playPause"
		aria-label={playback.playback.paused ? 'Play' : 'Pause'}
		title={playback.playback.paused ? 'Play' : 'Pause'}
		style:background="var(--mbv-secondary)"
	>
		{#if playback.playback.paused}
			<Play size={16} fill={roleVar('accent')} color={roleVar('accent')} />
		{:else}
			<Pause size={16} fill={roleVar('textFocusAccent')} color={roleVar('textFocusAccent')} />
		{/if}
	</button>
	<button
		type="button"
		class="flex shrink-0 items-center justify-center"
		bind:this={nextEl}
		data-target="next"
		aria-label="Next"
		title="Next"
	>
		<SkipForward
			size={18}
			fill={glyphColor(playback.availability.next, 'textStrong')}
			color={glyphColor(playback.availability.next, 'textStrong')}
		/>
	</button>
</div>
