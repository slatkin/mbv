<script lang="ts">
	// The player panel's top block (add-tauri-gui-client task 6.4, design
	// D15): the expanded canvas artboard's header, artwork with its episode
	// badge, title and parent line, scrubber, and transport. The canvas, not
	// the TUI band, is the reference; the paint uses the theme's roles and
	// chrome fills through `--mbv-<name>` properties only (design D7).
	//
	// The scrubber paints the played fraction, the chapter boundaries, the
	// intro and credits mark segments, the thumb, and the elapsed, remaining,
	// and finish-clock texts. Its hover preview paints from the frame's
	// `hover` datum (the core resolves the pointer move against the painted
	// bar), so the component holds no hover state; the trickplay tile stays a
	// placeholder until task 6.7 fetches its bytes. A press on the bar
	// forwards like every other pointer event: the component interprets no
	// gesture, it reports its regions and the core resolves the press against
	// the hit map (design D4).
	//
	// The header's playback target reports `playbackTarget`; its press opens
	// the core's chooser, whose popover paints under the header from the
	// frame's `targetChooser` overlay. Each row reports its `targetChoice`
	// index — the core resolves a row press against the same live choice list
	// the frame carries — under a second report key, first inserted when the
	// chooser opens: this component mounts before the queue panel and footer,
	// so its own key lands before their regions in the merged report, and
	// only a key inserted after them puts the rows last — the hit map's last
	// push wins where a row paints over a queue row beneath it. The header's
	// collapse and queue-fold controls report the ids the core's `z`/`q`
	// toggles run from. The window holds no open, cursor, or hover state; Esc
	// and a press elsewhere close the popover through the core's key and
	// pointer paths. The track chips row and its popover live in `TrackChips`
	// below the transport.
	import { onMount } from 'svelte';
	import {
		Cast,
		Check,
		ChevronDown,
		ChevronUp,
		Film,
		ListCollapse,
		Pause,
		Play,
		RotateCcw,
		RotateCw,
		SkipBack,
		SkipForward,
	} from '@lucide/svelte';
	import { reportLayout, retractLayout } from '$lib/bridge/layout';
	import TrackChips from './TrackChips.svelte';
	import { CELL_COL_PX } from '$lib/bridge/cells';
	import { fmtDurationShort } from '$lib/bridge/format';
	import { roleProperty } from '$lib/bridge/theme';
	import type {
		GuiCardArtwork,
		GuiChapter,
		GuiNowPlayingStatus,
		GuiOverlayFrame,
		GuiQueuePlayback,
		GuiSegmentMark,
		GuiTrickplaySheet,
		LayoutRegion,
		QueuePlaybackTargetId,
	} from '$lib/bridge/types';

	interface Props {
		playback: GuiQueuePlayback;
		/** The open menu or popover (design D4): the playback-target chooser
		 * paints under the header while it is the open overlay. `null` while
		 * none is open. */
		overlay: GuiOverlayFrame | null;
		/** The artwork the window carried for the active item (design D10),
		 * held by the panel above this one so a state that mounts later paints
		 * it too. `null` in a bare render, where the frame's own `artwork` is
		 * the only source. */
		carried?: GuiCardArtwork | null;
		/** The trickplay sheet the window carried (task 6.7), held by the panel
		 * above this one: a frame re-carries the sheet's image only when the
		 * hovered sheet changes, so the preview repaints the latched image while
		 * the tile names its key. `null` in a bare render. */
		carriedSheet?: GuiTrickplaySheet | null;
		/** Whether the queue is folded (the `q` toggle, design D10): the
		 * header's fold control paints the expand direction then, so the
		 * control names the toggle's next effect in both directions. */
		queueFolded?: boolean;
	}
	const {
		playback,
		overlay,
		carried = null,
		carriedSheet = null,
		queueFolded = false,
	}: Props = $props();

	const reportKey = Symbol('queue-playback-panel');
	const chooserKey = Symbol('target-chooser');
	let panelEl: HTMLElement | undefined = $state();
	let targetEl: HTMLElement | undefined = $state();
	let playPauseEl: HTMLElement | undefined = $state();
	let prevEl: HTMLElement | undefined = $state();
	let nextEl: HTMLElement | undefined = $state();
	let seekBackEl: HTMLElement | undefined = $state();
	let seekForwardEl: HTMLElement | undefined = $state();
	let seekbarEl: HTMLElement | undefined = $state();
	let artEl: HTMLElement | undefined = $state();
	let collapseEl: HTMLElement | undefined = $state();
	let queueFoldEl: HTMLElement | undefined = $state();
	let chooserEl: HTMLElement | undefined = $state();

	/** The art the panel paints: this frame's carried value when it has one,
	 * else the value the panel above carried for the window (design D10). */
	const art = $derived(playback.artwork ?? carried);

	/** The open playback-target chooser, or `null` while another overlay (or
	 * none) is up. The rows paint the core's live choice list; a row's press
	 * resolves against its reported `targetChoice` index, so no cursor or
	 * open state lives here. */
	const chooser = $derived(overlay?.kind === 'targetChooser' ? overlay : null);

	/** The played fraction of the scrubber, clamped to `[0, 1]`. */
	const progress = $derived(
		clampRatio(
			playback.playback.positionTicks,
			playback.playback.runtimeTicks,
		),
	);

	/** The ticks left in the active item. */
	const remainingTicks = $derived(
		Math.max(
			0,
			playback.playback.runtimeTicks - playback.playback.positionTicks,
		),
	);

	/** The chapter boundaries the bar paints, skipping the first tick (the
	 * bar's own left edge) and any boundary outside the runtime. */
	const chapterTicks = $derived(
		visibleChapters(
			playback.chapters,
			playback.playback.runtimeTicks,
		),
	);

	/** The intro and credits marks the bar paints, in bar fractions. */
	const marks = $derived(
		[
			markSpan(playback.introMark, playback.playback.runtimeTicks),
			markSpan(playback.creditsMark, playback.playback.runtimeTicks),
		].filter((mark) => mark !== null),
	);

	/** The scrubber's hover preview, painted from the frame: the pointer's
	 * fraction, the time it names, the chapter covering it, and the trickplay
	 * tile (task 6.7). `null` while the pointer is away from the bar. */
	const hover = $derived(playback.hover);

	/** The preview thumbnail's painted width: the canvas's `w-32` box. */
	const TILE_WIDTH_PX = 128;

	/** The hover preview's tile crop (task 6.7): the thumbnail's place in its
	 * sheet, scaled to the preview box, as the CSS sprite the browser paints.
	 * `null` while no sheet is latched for the hovered tile, so the preview
	 * falls back to the placeholder. */
	const tileCrop = $derived.by(() => {
		const tile = hover?.tile;
		const image = tile
			? (tile.sheet ??
				(carriedSheet?.key === tile.sheetKey ? carriedSheet.image : null))
			: null;
		if (!tile || !image || tile.width <= 0) {
			return null;
		}
		const scale = TILE_WIDTH_PX / tile.width;
		return {
			width: `${TILE_WIDTH_PX}px`,
			height: `${tile.height * scale}px`,
			backgroundImage: `url('${image}')`,
			backgroundSize: `${tile.columns * TILE_WIDTH_PX}px auto`,
			backgroundPosition: `-${tile.column * TILE_WIDTH_PX}px -${tile.row * tile.height * scale}px`,
		};
	});

	/** A fraction `ticks / runtime`, clamped to `[0, 1]`; zero runtime is
	 * zero. */
	function clampRatio(ticks: number, runtime: number): number {
		if (runtime <= 0) {
			return 0;
		}
		return Math.min(1, Math.max(0, ticks / runtime));
	}

	/** The chapter boundaries inside the runtime, with their bar fractions. */
	function visibleChapters(
		chapters: GuiChapter[],
		runtime: number,
	): { name: string; ratio: number }[] {
		return chapters
			.filter(
				(chapter) =>
					chapter.startTicks > 0 &&
					chapter.startTicks < runtime,
			)
			.map((chapter) => ({
				name: chapter.name,
				ratio: clampRatio(chapter.startTicks, runtime),
			}));
	}

	/** One mark's span in bar fractions, `null` outside the runtime. */
	function markSpan(
		mark: GuiSegmentMark | null,
		runtime: number,
	): { left: number; width: number } | null {
		if (!mark || runtime <= 0) {
			return null;
		}
		const left = clampRatio(mark.startTicks, runtime);
		const right = clampRatio(mark.endTicks, runtime);
		return right > left ? { left, width: right - left } : null;
	}

	/** The header's status word (`header_status_word`). */
	function statusWord(status: GuiNowPlayingStatus): string {
		switch (status) {
			case 'playing':
				return 'PLAYING';
			case 'paused':
				return 'PAUSED';
			case 'idle':
				return 'IDLE';
		}
	}

	/** The status word's semantic colour (`status_color`). */
	function statusColor(status: GuiNowPlayingStatus): string {
		switch (status) {
			case 'playing':
				return 'var(--mbv-accent)';
			case 'paused':
				return 'var(--mbv-textFocusAccent)';
			case 'idle':
				return 'var(--mbv-textMuted)';
		}
	}

	/** The `var(--mbv-<role>)` value a part paints in. */
	function roleVar(role: string): string {
		return `var(${roleProperty(role)})`;
	}

	/** A transport glyph's colour: its role while actionable, muted when
	 * not — the availability gates the TUI's glyph colours carry. */
	function glyphColor(available: boolean, role: string): string {
		return available ? roleVar(role) : roleVar('textMuted');
	}

	/** One element's painted region, `null` while the element is absent. */
	function region(
		el: HTMLElement | undefined,
		id: QueuePlaybackTargetId,
	): LayoutRegion | null {
		if (!el) {
			return null;
		}
		const rect = el.getBoundingClientRect();
		return {
			rect: { x: rect.x, y: rect.y, width: rect.width, height: rect.height },
			target: { surface: 'queuePlayback', id },
		};
	}

	/** The open chooser's painted rows, each resolved to its choice index in
	 * the frame's `targetChoices` list. */
	function chooserRegions(): LayoutRegion[] {
		if (!chooser || !chooserEl) {
			return [];
		}
		return [...chooserEl.querySelectorAll<HTMLElement>('[data-choice]')]
			.map((el) => region(el, { targetChoice: Number(el.dataset.choice) }))
			.filter((entry) => entry !== null);
	}

	/** The painted regions this frame's DOM reports (design D4): the transport
	 * controls, the seekbar, the visual slot, the header's collapse and
	 * queue-fold controls, and the playback-target indicator. The open
	 * chooser's rows report under the chooser's own key (the header comment
	 * names why they report last). */
	function currentRegions(): LayoutRegion[] {
		return [
			region(playPauseEl, 'playPause'),
			region(prevEl, 'previous'),
			region(seekBackEl, 'seekBack'),
			region(seekForwardEl, 'seekForward'),
			region(nextEl, 'next'),
			region(seekbarEl, 'seekbar'),
			region(artEl, 'visualSlot'),
			region(collapseEl, 'collapse'),
			region(queueFoldEl, 'queueFold'),
			region(targetEl, 'playbackTarget'),
		].filter((entry) => entry !== null);
	}

	function report(): void {
		reportLayout(reportKey, { lists: [], regions: currentRegions() });
		if (chooser) {
			reportLayout(chooserKey, { lists: [], regions: chooserRegions() });
		}
	}

	/** The observer the mount and the art element's effect share; the
	 * art element's own resizes report (design D4) — the artwork's
	 * intrinsic size moves the rows below it. */
	let artObserver: ResizeObserver | undefined;

	// Each paint that moved a region reports (design D4): this effect runs
	// after the DOM update, and the ResizeObserver reports layout-driven
	// moves. Reading the overlay registers the chooser's rows, which appear
	// and disappear with that frame section; the chooser's key retracts while
	// none is open. The reporter dedupes an unchanged contribution.
	$effect(() => {
		void playback;
		void overlay;
		report();
		if (!chooser) {
			retractLayout(chooserKey);
		}
	});

	// The art element's resizes report: an artwork swap rebinds it, and its
	// bitmap's load sizes it — both move the regions beside it.
	$effect(() => {
		const el = artEl;
		const observer = artObserver;
		if (!el || !observer) {
			return;
		}
		observer.observe(el);
		return () => observer.unobserve(el);
	});

	onMount(() => {
		const observer =
			typeof ResizeObserver === 'undefined'
				? undefined
				: new ResizeObserver(report);
		artObserver = observer;
		if (panelEl) {
			observer?.observe(panelEl);
		}
		return () => {
			observer?.disconnect();
			artObserver = undefined;
			retractLayout(reportKey);
			retractLayout(chooserKey);
		};
	});
</script>

<!-- The player panel block on the queue column's backdrop: the header row,
	then the artwork, the title and parent line, the scrubber, and the
	transport. Every gap and pad is a cell-unit multiple, the geometry basis
	the queue column already paints with (`bridge/cells`). -->
<div
	class="flex flex-col"
	style:padding="var(--mbv-cell-row) calc(var(--mbv-cell-col) * 2)"
	style:gap="var(--mbv-cell-row)"
	bind:this={panelEl}
>
	<!-- The header row: the playback target at the left, then the state word,
		the collapse control, and the queue-fold control at the right. The
		target span is the `playbackTarget` region; its press opens the core's
		chooser, whose popover paints below it from the frame's
		`targetChooser` overlay. Each chooser row is its own `targetChoice`
		region; Esc and an outside press close the popover in the core. -->
	<div
		class="relative flex items-center whitespace-nowrap"
		style:height="var(--mbv-cell-row)"
		style:gap="var(--mbv-cell-col)"
	>
		<span
			class="flex min-w-0 shrink items-center"
			style:gap="var(--mbv-cell-col)"
			bind:this={targetEl}
			data-target="playbackTarget"
		>
			<Cast
				size={CELL_COL_PX}
				color={playback.hostIsRemote
					? 'var(--mbv-playbackTargetRemoteFg)'
					: 'var(--mbv-playbackTargetLocalFg)'}
			/>
			{#if playback.host}
				<span class="truncate" style:color={roleVar('textPrimary')}>{playback.host}</span>
			{/if}
		</span>
		<span class="flex-1"></span>
		<span
			class="font-bold tracking-wide"
			style:color={statusColor(playback.status)}>{statusWord(playback.status)}</span
		>
		<button
			type="button"
			class="flex items-center rounded-sm p-1"
			bind:this={collapseEl}
			data-target="collapse"
			aria-label="Collapse panel"
			title="Collapse panel (z)"
		>
			<ChevronUp size={16} color={roleVar('textMuted')} />
		</button>
		<button
			type="button"
			class="flex items-center rounded-sm p-1"
			bind:this={queueFoldEl}
			data-target="queueFold"
			aria-label={queueFolded ? 'Expand queue' : 'Fold queue'}
			title={queueFolded ? 'Expand queue (q)' : 'Fold queue (q)'}
		>
			{#if queueFolded}
				<ChevronDown size={16} color={roleVar('textMuted')} />
			{:else}
				<ListCollapse size={16} color={roleVar('textMuted')} />
			{/if}
		</button>
		{#if chooser}
			<!-- The playback-target chooser: the core's live choice list, the
				attached output marked with a check. The rows report their
				`targetChoice` indices under the chooser's own key, last over
				every region beneath them (the header comment names why). -->
			<div
				class="absolute top-full left-0 z-20 mt-1 flex min-w-56 flex-col rounded-md border py-1 shadow-md"
				bind:this={chooserEl}
				style:background="var(--mbv-popover)"
				style:border-color="var(--mbv-border)"
			>
				{#each chooser.choices as choice, index (index)}
					<button
						type="button"
						class="flex items-center text-left"
						style:padding="calc(var(--mbv-cell-row) / 4) calc(var(--mbv-cell-col) * 1)"
						style:gap="var(--mbv-cell-col)"
						style:color={index === chooser.cursor
							? roleVar('textFocusAccent')
							: roleVar('textPrimary')}
						data-choice={index}
					>
						<span class="flex w-4 shrink-0 justify-center">
							{#if choice.isActive}
								<Check size={14} color={roleVar('accent')} />
							{/if}
						</span>
						<span class="truncate">{choice.label}</span>
					</button>
				{/each}
			</div>
		{/if}
	</div>
	{#if playback.nowPlayingTitle}
		<!-- The artwork with the season-and-episode badge; the badge is an
			Emby-episode datum, so a non-episode item paints none. The wrapper
			is the visual slot's painted region. -->
		{#if art && art.kind !== 'hidden'}
			<div
				class="relative flex justify-center"
				bind:this={artEl}
				data-target="visualSlot"
			>
				{#if art.kind === 'loading'}
					<div
						class="aspect-square"
						style:width="min(100%, calc(var(--mbv-cell-row) * 24))"
						style:background="var(--mbv-textMuted)"
					></div>
				{:else}
					<img
						class="max-w-full rounded-md object-contain"
						alt=""
						src={art.detail}
						style:max-height="calc(var(--mbv-cell-row) * 24)"
					/>
				{/if}
				{#if playback.episodeBadge}
					<span
						class="absolute top-0 right-0 rounded-md px-2 py-0.5 font-mono text-xs font-semibold"
						style:background="var(--mbv-popover)"
						style:color={roleVar('playbackMetaFg')}
						>{playback.episodeBadge}</span
					>
				{/if}
			</div>
		{/if}
		<!-- The title and its parent line: the title in the playback-title
			role, the parent line muted below it. -->
		<div class="flex min-w-0 flex-col">
			<span
				class="truncate font-semibold"
				style:color={roleVar('playbackTitleFg')}>{playback.nowPlayingTitle}</span
			>
			{#if playback.parentLine}
				<span class="truncate" style:color={roleVar('playbackContextFg')}
					>{playback.parentLine}</span
				>
			{/if}
		</div>
		{#if playback.showControls}
			<!-- The scrubber: elapsed at the left, the bar, then the remaining
				time and the finish clock at the right. The bar carries the
				played fill, the intro and credits mark segments, the chapter
				ticks, and the thumb. Its own rect is the `seekbar` region; a
				press resolves there in the core. -->
			<div class="flex items-center" style:gap="var(--mbv-cell-col)">
				<span
					class="shrink-0 font-mono text-xs tabular-nums"
					style:color={roleVar('playbackMetaFg')}
					>{fmtDurationShort(playback.playback.positionTicks)}</span
				>
				<div
					class="relative min-w-4 flex-1"
					role="slider"
					tabindex="0"
					aria-label="Seek"
					aria-valuemin={0}
					aria-valuemax={100}
					aria-valuenow={Math.round(progress * 100)}
					style:height="var(--mbv-cell-row)"
					bind:this={seekbarEl}
					data-target="seekbar"
				>
					<div
						class="absolute inset-x-0 top-1/2 h-1 -translate-y-1/2 overflow-hidden rounded-full"
						style:background="var(--mbv-progressTrack)"
					>
						{#each marks as mark, i (i)}
							<div
								class="absolute inset-y-0 opacity-40"
								style:left="{mark.left * 100}%"
								style:width="{mark.width * 100}%"
								style:background="var(--mbv-accent)"
							></div>
						{/each}
						<div
							class="absolute inset-y-0 left-0"
							style:width="{progress * 100}%"
							style:background="var(--mbv-accent)"
						></div>
						{#each chapterTicks as chapter, i (i)}
							<div
								class="absolute inset-y-0 w-px"
								style:left="{chapter.ratio * 100}%"
								style:background="var(--mbv-textMuted)"
							></div>
						{/each}
					</div>
					<div
						class="absolute top-1/2 h-3 w-3 -translate-x-1/2 -translate-y-1/2 rounded-full border-2"
						style:left="{progress * 100}%"
						style:background="var(--mbv-accent)"
						style:border-color="var(--mbv-background)"
					></div>
					{#if hover}
						<!-- The hover preview: the trickplay frame (task 6.7 — the
							thumbnail cropped from the sheet the core's cache
							encoded, else the placeholder while no sheet is cached),
							the time at the pointer, and the chapter that covers it,
							all from the frame's `hover` datum. -->
						<div
							class="pointer-events-none absolute bottom-full z-10 mb-1 -translate-x-1/2 rounded-md border p-1 whitespace-nowrap"
							style:left="{hover.fraction * 100}%"
							style:background="var(--mbv-popover)"
							style:border-color="var(--mbv-border)"
						>
							{#if tileCrop}
								<div
									class="mb-1 overflow-hidden rounded-sm bg-no-repeat"
									style:width={tileCrop.width}
									style:height={tileCrop.height}
									style:background-image={tileCrop.backgroundImage}
									style:background-size={tileCrop.backgroundSize}
									style:background-position={tileCrop.backgroundPosition}
								></div>
							{:else if playback.trickplay}
								<div
									class="mb-1 flex aspect-video w-32 items-center justify-center rounded-sm"
									style:background="var(--mbv-secondary)"
								>
									<Film size={16} color={roleVar('textMuted')} />
								</div>
							{/if}
							<div class="flex items-center" style:gap="var(--mbv-cell-col)">
								<span
									class="font-mono text-xs tabular-nums"
									style:color={roleVar('textPrimary')}
									>{fmtDurationShort(hover.ticks)}</span
								>
								{#if hover.chapter}
									<span class="text-xs" style:color={roleVar('playbackContextFg')}
										>{hover.chapter}</span
									>
								{/if}
							</div>
						</div>
					{/if}
				</div>
				<span
					class="shrink-0 font-mono text-xs tabular-nums"
					style:color={roleVar('playbackMetaFg')}
					>-{fmtDurationShort(remainingTicks)}</span
				>
				{#if playback.finishClock}
					<span
						class="shrink-0 font-mono text-xs tabular-nums"
						style:color={roleVar('playbackMetaFg')}>{playback.finishClock}</span
					>
				{/if}
			</div>
		{/if}
		<!-- The transport controls (the canvas's row): previous, back 10 s,
			play/pause, forward 30 s, and next. The spec's list carries no stop
			control, so the canvas panel paints none. -->
		<div
			class="flex items-center justify-center"
			style:height="var(--mbv-cell-row)"
			style:gap="calc(var(--mbv-cell-col) * 2)"
		>
			<button
				type="button"
				class="flex items-center"
				bind:this={prevEl}
				data-target="previous"
				aria-label="Previous"
			>
				<SkipBack
					size={18}
					fill={glyphColor(playback.availability.previous, 'textStrong')}
					color={glyphColor(playback.availability.previous, 'textStrong')}
				/>
			</button>
			<button
				type="button"
				class="relative flex items-center justify-center"
				bind:this={seekBackEl}
				data-target="seekBack"
				aria-label="Back 10 seconds"
				title="Back 10 s"
			>
				<RotateCcw size={18} color={roleVar('textStrong')} />
				<span
					class="absolute font-mono font-semibold"
					style:font-size="8px"
					style:color={roleVar('textStrong')}>10</span
				>
			</button>
			<button
				type="button"
				class="flex items-center rounded-full p-1"
				bind:this={playPauseEl}
				data-target="playPause"
				aria-label={playback.playback.paused ? 'Play' : 'Pause'}
				style:background="var(--mbv-secondary)"
			>
				{#if playback.playback.paused}
					<Play size={18} fill={roleVar('accent')} color={roleVar('accent')} />
				{:else}
					<Pause size={18} fill={roleVar('textFocusAccent')} color={roleVar('textFocusAccent')} />
				{/if}
			</button>
			<button
				type="button"
				class="relative flex items-center justify-center"
				bind:this={seekForwardEl}
				data-target="seekForward"
				aria-label="Forward 30 seconds"
				title="Forward 30 s"
			>
				<RotateCw size={18} color={roleVar('textStrong')} />
				<span
					class="absolute font-mono font-semibold"
					style:font-size="8px"
					style:color={roleVar('textStrong')}>30</span
				>
			</button>
			<button
				type="button"
				class="flex items-center"
				bind:this={nextEl}
				data-target="next"
				aria-label="Next"
			>
				<SkipForward
					size={18}
					fill={glyphColor(playback.availability.next, 'textStrong')}
					color={glyphColor(playback.availability.next, 'textStrong')}
				/>
			</button>
		</div>
		<!-- The track chips row under the transport: the video label and the
			audio and subtitle chips with the open popover anchored above its
			chip. `trackChips` is `null` while the local player is inactive, so
			the row is absent then, as the status-indicator cluster is. -->
		<TrackChips chips={playback.trackChips} {overlay} />
	{/if}
</div>
