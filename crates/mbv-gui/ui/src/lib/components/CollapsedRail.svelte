<script lang="ts">
	// The panel's rail state (add-tauri-gui-client row 6.6, design D10): the
	// panel collapsed to 72 px by the `z` toggle. The canvas's rail is the
	// artwork with the played fraction as a ring around it, the transport's
	// play/pause and next, the remaining time, and the control that expands
	// the panel again.
	//
	// The expand control reports the `collapse` target, so its press runs the
	// same `z` toggle the expanded header's control runs — one core path for
	// both directions, as the toggle's own rule ("pressing `z` again restores
	// the previous state") requires. The rail paints nothing else: the queue
	// and the footer belong to the expanded artboard, and the core keeps
	// their state while the rail is up (the `gui-client` spec: the window
	// holds no interaction state).
	import { onMount } from 'svelte';
	import { ChevronsRight, Pause, Play, SkipForward } from '@lucide/svelte';
	import { QUEUE_RAIL_WIDTH_PX } from '$lib/bridge/arrangement';
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
		/** The artwork the window carries for the active item (design D10):
		 * the frame arms it on a change and `Panel` keeps it, so the rail
		 * paints the same art the expanded artboard showed. `null` while the
		 * window has carried none. */
		art: GuiCardArtwork | null;
	}
	const { playback, art }: Props = $props();

	/** The rail's width is the Queue panel's collapsed width
	 * (`arrangement.ts`), the same number the window measures the Library column
	 * against. The ring hugs the artwork, and its stroke spans the played
	 * fraction. */
	const RING_SIZE_PX = 52;
	const RING_STROKE_PX = 3;
	const RING_RADIUS_PX = (RING_SIZE_PX - RING_STROKE_PX) / 2;
	const RING_CIRCUMFERENCE_PX = 2 * Math.PI * RING_RADIUS_PX;
	const ART_SIZE_PX = RING_SIZE_PX - 2 * RING_STROKE_PX - 4;

	const reportKey = Symbol('collapsed-rail');
	let railEl: HTMLElement | undefined = $state();
	let artEl: HTMLElement | undefined = $state();
	let playPauseEl: HTMLElement | undefined = $state();
	let nextEl: HTMLElement | undefined = $state();
	let expandEl: HTMLElement | undefined = $state();

	/** The played fraction the ring paints, clamped to `[0, 1]`. */
	const progress = $derived(clampRatio(playback.playback.positionTicks, playback.playback.runtimeTicks));

	/** The artwork's bitmap, `null` while the item has none (a placeholder,
	 * loading, or hidden slot paints the empty ring). */
	const image = $derived(art?.kind === 'image' ? art.detail : null);

	/** The played arc's length in ring pixels. */
	const playedArc = $derived(progress * RING_CIRCUMFERENCE_PX);

	/** One element's painted region for a QueuePlayback target, `null` while
	 * the element is absent. */
	const region = (
		el: HTMLElement | undefined,
		id: QueuePlaybackTargetId
	): LayoutRegion | null => elementRegion(el, { surface: 'queuePlayback', id });

	/** The painted regions (design D4): the transport's two controls, the
	 * artwork slot, and the expand control. */
	function currentRegions(): LayoutRegion[] {
		return [
			region(playPauseEl, 'playPause'),
			region(nextEl, 'next'),
			region(artEl, 'visualSlot'),
			region(expandEl, 'collapse'),
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
		void art;
		report();
	});

	onMount(() => observeLayout(reportKey, railEl, report));
</script>

<!-- The rail: one narrow column at the column's leading edge, the ring and
	the transport centred, the remaining time under them, and the expand
	control at the bottom. -->
<div
	class="flex h-full shrink-0 flex-col items-center"
	style:width="{QUEUE_RAIL_WIDTH_PX}px"
	style:padding="var(--mbv-cell-row) 0"
	style:gap="var(--mbv-cell-row)"
	bind:this={railEl}
	data-panel-state="rail"
	style:background="var(--canvas-dim)"
>
	<!-- The artwork with the played fraction as a ring around it: the track
		circle in the progress-track role, the arc in the accent. -->
	<div
		class="relative flex shrink-0 items-center justify-center"
		style:width="{RING_SIZE_PX}px"
		style:height="{RING_SIZE_PX}px"
		bind:this={artEl}
		data-target="visualSlot"
	>
		<svg
			class="absolute inset-0 -rotate-90"
			viewBox="0 0 {RING_SIZE_PX} {RING_SIZE_PX}"
			width={RING_SIZE_PX}
			height={RING_SIZE_PX}
			aria-hidden="true"
		>
			<circle
				cx={RING_SIZE_PX / 2}
				cy={RING_SIZE_PX / 2}
				r={RING_RADIUS_PX}
				fill="none"
				stroke="var(--mbv-progressTrack)"
				stroke-width={RING_STROKE_PX}
			/>
			<circle
				cx={RING_SIZE_PX / 2}
				cy={RING_SIZE_PX / 2}
				r={RING_RADIUS_PX}
				fill="none"
				stroke="var(--mbv-accent)"
				stroke-width={RING_STROKE_PX}
				stroke-linecap="round"
				stroke-dasharray="{playedArc} {RING_CIRCUMFERENCE_PX}"
			/>
		</svg>
		{#if image}
			<img
				class="rounded-full object-cover"
				alt=""
				src={image}
				style:width="{ART_SIZE_PX}px"
				style:height="{ART_SIZE_PX}px"
			/>
		{:else}
			<div
				class="rounded-full"
				style:width="{ART_SIZE_PX}px"
				style:height="{ART_SIZE_PX}px"
				style:background="var(--mbv-textMuted)"
			></div>
		{/if}
	</div>
	<!-- The transport's play/pause and next, the rail's two controls (the
		spec's rail list). -->
	<button
		type="button"
		class="flex items-center justify-center rounded-full p-1"
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
		class="flex items-center justify-center"
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
	{#if playback.playback.runtimeTicks > 0}
		<!-- The remaining time: the same `-M:SS` the expanded scrubber's right
			end paints. -->
		<span
			class="font-mono text-xs tabular-nums"
			style:color={roleVar('playbackMetaFg')}
			data-remaining>-{playback.remainingClock}</span
		>
	{/if}
	<span class="flex-1"></span>
	<!-- The expand control: the `z` toggle's second direction, the same
		`collapse` target the header's control reports. -->
	<button
		type="button"
		class="flex items-center justify-center rounded-sm p-1"
		bind:this={expandEl}
		data-target="collapse"
		aria-label="Expand panel"
		title="Expand panel (z)"
	>
		<ChevronsRight size={16} color={roleVar('textMuted')} />
	</button>
</div>
