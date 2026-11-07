<script lang="ts">
	// The player panel's top block: the expanded canvas artboard's header,
	// artwork with its episode badge, title and parent line, scrubber, and
	// transport (`align-gui-with-design-canvas` U3). The canvas, not the TUI
	// band, is the reference; the paint uses the theme's roles and chrome
	// fills through `--mbv-<name>` properties only (design D7).
	//
	// The scrubber paints the canvas geometry: a 6 px track, the intro and
	// credits blocks, the played fill, 2×14 chapter ticks, a 16 px thumb, the
	// hover position line, and the elapsed and `−remaining · ends HH:MM`
	// labels. Its hover preview paints from the frame's `hover` datum (the
	// core resolves the pointer move against the painted bar), so the
	// component holds no hover state; the trickplay tile stays a `[trickplay]`
	// box until a sheet is cached. A press on the bar forwards like every
	// other pointer event: the component interprets no gesture, it reports its
	// regions and the core resolves the press against the hit map (design
	// D4).
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
	// collapse control reports the id the core's `z` toggle runs from. The
	// canvas paints no queue-fold control in the header (`main` audit 4); `q`
	// stays a chord and the folded line owns the expand control. The window
	// holds no open, cursor, or hover state; Esc and a press elsewhere close
	// the popover through the core's key and pointer paths. The track chips
	// row and its popover live in `TrackChips` below the transport.
	import { onMount } from 'svelte';
	import {
		Cast,
		Check,
		ChevronDown,
		Maximize2,
		Pause,
		Play,
		SkipBack,
		SkipForward
	} from '@lucide/svelte';
	import { elementRegion, reportLayout, retractLayout } from '$lib/bridge/layout';
	import TrackChips from './TrackChips.svelte';
	import { glyphColor, roleVar } from '$lib/bridge/theme';
	import { clampRatio, minus } from '$lib/bridge/time';
	import type {
		GuiCardArtwork,
		GuiChapter,
		GuiNowPlayingStatus,
		GuiOverlayFrame,
		GuiQueuePlayback,
		GuiSegmentMark,
		GuiTrickplaySheet,
		LayoutRegion,
		QueuePlaybackTargetId
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
	}
	const {
		playback,
		overlay,
		carried = null,
		carriedSheet = null,
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

	/** The chapter boundaries the bar paints as 2×14 ticks, skipping the first
	 * boundary (the bar's own left edge) and any outside the runtime, in bar
	 * fractions. */
	const chapterTicks = $derived(
		visibleChapters(
			playback.chapters,
			playback.playback.runtimeTicks,
		),
	);

	/** The intro and credits marks' starts in bar fractions (`main` audit 10);
	 * `null` while a mark is unknown. */
	const introLeft = $derived(
		markLeft(playback.introMark, playback.playback.runtimeTicks),
	);
	const creditsLeft = $derived(
		markLeft(playback.creditsMark, playback.playback.runtimeTicks),
	);

	/** The scrubber's hover preview, painted from the frame: the pointer's
	 * fraction, the time it names, the chapter covering it, and the trickplay
	 * tile (task 6.7). The tile is present while the item offers one — the
	 * preview paints it cropped or as the in-flight box — and `null` for a
	 * position without trickplay, where the preview paints the label chip
	 * alone. `null` while the pointer is away from the bar. */
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

	/** One element's painted region for a QueuePlayback target, `null` while
	 * the element is absent. */
	const region = (
		el: HTMLElement | undefined,
		id: QueuePlaybackTargetId
	): LayoutRegion | null => elementRegion(el, { surface: 'queuePlayback', id });

	/** The chapter boundaries inside the runtime, with their bar fractions. */
	function visibleChapters(
		chapters: GuiChapter[],
		runtime: number,
	): number[] {
		return chapters
			.filter(
				(chapter) =>
					chapter.startTicks > 0 &&
					chapter.startTicks < runtime,
			)
			.map((chapter) => clampRatio(chapter.startTicks, runtime));
	}

	/** One mark's start in bar fractions, `null` while unknown or outside the
	 * runtime. */
	function markLeft(
		mark: GuiSegmentMark | null,
		runtime: number,
	): number | null {
		if (!mark || runtime <= 0) {
			return null;
		}
		return clampRatio(mark.startTicks, runtime);
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
	 * controls, the seekbar, the visual slot, the header's collapse control,
	 * and the playback-target indicator. The open chooser's rows report under
	 * the chooser's own key (the header comment names why they report last).
	 */
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

<!-- The player panel block on the canvas's backdrop: the header row, then
	the fixed still slot, the title and parent line, the scrubber, and the
	transport (`align-gui-with-design-canvas` U3; `Main.dc.html` is the
	reference). The canvas paints no outer pad and no gutter between these
	blocks: the header and the still slot are full-bleed, and the title,
	scrubber, and transport carry their own 16 px insets. -->
<div class="flex flex-col" bind:this={panelEl}>
	<!-- The header row: the playback target at the left, then the state
		label and the collapse control at the right. The target button is the
		`playbackTarget` region; its press opens the core's chooser, whose
		popover paints below it from the frame's `targetChooser` overlay. Each
		chooser row is its own `targetChoice` region; Esc and an outside press
		close the popover in the core. The canvas paints no queue-fold control
		here (`main` audit 4): `q` stays a chord. -->
	<div
		class="relative flex items-center justify-between whitespace-nowrap"
		style:height="44px"
		style:padding="0 8px 0 4px"
		style:border-bottom="1px solid var(--mbv-border)"
	>
		<button
			type="button"
			class="flex min-w-0 items-center"
			style:height="44px"
			style:padding="0 10px"
			style:gap="8px"
			style:font-size="13px"
			style:color={playback.hostIsRemote
				? roleVar('playbackTargetRemoteFg')
				: roleVar('playbackTargetLocalFg')}
			bind:this={targetEl}
			data-target="playbackTarget"
			aria-label="Cast to device"
		>
			<Cast size={18} strokeWidth={1.8} color="currentColor" />
			{#if playback.host}
				<span class="truncate">{playback.host}</span>
			{/if}
			<ChevronDown size={12} strokeWidth={2} color="currentColor" />
		</button>
		<div class="flex items-center" style:gap="4px">
			<span
				class="font-mono"
				style:font-size="11px"
				style:letter-spacing="0.08em"
				style:padding="0 8px"
				style:color={statusColor(playback.status)}
				>{`\u25CF ${statusWord(playback.status)}`}</span
			>
			<button
				type="button"
				class="flex items-center justify-center"
				style:width="44px"
				style:height="44px"
				bind:this={collapseEl}
				data-target="collapse"
				aria-label="Collapse panel"
				title="Collapse panel (z)"
			>
				<Maximize2 size={18} strokeWidth={1.8} color={roleVar('textMuted')} />
			</button>
		</div>
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
		<!-- The still slot (`main` audit 6): a fixed 225 px box on the raised
			control fill, the artwork inside it, and the canvas caption while no
			image lands. The wrapper is the visual slot's painted region. The
			season-and-episode badge is the canvas chip at the box's bottom
			left (`main` audit 7); a non-episode item paints none. -->
		{#if art && art.kind !== 'hidden'}
			<div
				class="relative flex items-center justify-center overflow-hidden"
				style:height="225px"
				style:background="var(--mbv-secondary)"
				bind:this={artEl}
				data-target="visualSlot"
			>
				{#if art.kind === 'image' || art.kind === 'placeholder'}
					<img
						class="h-full w-full object-contain"
						alt=""
						src={art.detail}
					/>
				{:else}
					<!-- The canvas's source caption while no image lands
						(design D4a's still slot): the frame's still-caption
						fact, one `[...]` line per item kind; an item with no
						image source paints no caption. -->
					{#if playback.stillCaption}
						<span
							class="font-mono"
							style:font-size="12px"
							style:color={roleVar('textMuted')}
							>{`[${playback.stillCaption}]`}</span
						>
					{/if}
				{/if}
				{#if playback.episodeBadge}
					<span
						class="absolute font-mono"
						style:left="12px"
						style:bottom="10px"
						style:font-size="11px"
						style:padding="3px 6px"
						style:border-radius="4px"
						style:background="var(--mbv-background)"
						style:color={roleVar('textPrimary')}
						>{playback.episodeBadge}</span
					>
				{/if}
			</div>
		{/if}
		<!-- The title at the canvas's 18 px/600 and its parent line in the
			canvas link style at 13 px (`main` audit 8). The canvas draws the
			parent line as an `<a href="#">`, but the GUI presents no surface to
			go to: the static line carries no region and the press is a no-op. -->
		<div class="flex min-w-0 flex-col" style:padding="14px 16px 6px" style:gap="2px">
			<span
				class="truncate"
				style:font-size="18px"
				style:font-weight="600"
				style:color={roleVar('playbackTitleFg')}>{playback.nowPlayingTitle}</span
			>
			{#if playback.parentLine}
				<span
					class="truncate"
					style:font-size="13px"
					style:color={roleVar('playbackContextFg')}>{playback.parentLine}</span
				>
			{/if}
		</div>
		{#if playback.showControls}
			<!-- The scrubber (`main` audits 9–12): the 60 px block with the
				track at 22 px, the intro and credits blocks, the played fill,
				the chapter ticks, the pointer's position line, the thumb, and
				the elapsed and `−remaining · ends HH:MM` labels at 42 px. The
				block's own rect is the `seekbar` region; a press resolves there
				in the core. -->
			<div style:padding="10px 16px 4px">
				<div
					class="relative"
					role="slider"
					tabindex="0"
					aria-label="Seek"
					aria-valuemin={0}
					aria-valuemax={100}
					aria-valuenow={Math.round(progress * 100)}
					style:height="60px"
					bind:this={seekbarEl}
					data-target="seekbar"
				>
					{#if hover}
						<!-- The hover preview (`main` audit 12): the 128×72
							trickplay tile stacked over the label chip, which reads
							the time and the covering chapter's number, not its
							name. A hovered tile paints the cached sheet when one
							is latched and the canvas `[trickplay]` box while the
							sheet's fetch is still in flight; a position the item
							offers no tile for paints the label chip alone (the
							spec's no-trickplay scenario). -->
						<div
							class="pointer-events-none absolute z-10 flex -translate-x-1/2 flex-col items-center"
							style:left="{hover.fraction * 100}%"
							style:top="-84px"
							style:width="128px"
							style:gap="4px"
						>
							{#if hover.tile}
								{#if tileCrop}
									<div
										class="overflow-hidden bg-no-repeat"
										style:width={tileCrop.width}
										style:height={tileCrop.height}
										style:border="1px solid var(--mbv-border)"
										style:border-radius="4px"
										style:background-image={tileCrop.backgroundImage}
										style:background-size={tileCrop.backgroundSize}
										style:background-position={tileCrop.backgroundPosition}
									></div>
								{:else}
									<div
										class="flex items-center justify-center font-mono"
										style:width="128px"
										style:height="72px"
										style:font-size="10px"
										style:background="var(--mbv-secondary)"
										style:border="1px solid var(--mbv-border)"
										style:border-radius="4px"
										style:color={roleVar('textSecondary')}
										>[trickplay]</div
									>
								{/if}
							{/if}
							<div
								class="font-mono"
								style:font-size="11px"
								style:padding="2px 6px"
								style:border-radius="3px"
								style:background="var(--mbv-background)"
								style:color={roleVar('playbackTitleFg')}
								>{hover.timeLabel}{hover.chapterNumber !== null
									? ` · Ch. ${hover.chapterNumber}`
									: ''}</div
							>
						</div>
					{/if}
					<div
						class="absolute inset-x-0"
						data-track
						style:top="22px"
						style:height="6px"
						style:background="var(--mbv-progressTrack)"
						style:border-radius="3px"
					></div>
					{#if introLeft !== null}
						<div
							class="absolute"
							style:left="{introLeft * 100}%"
							style:top="22px"
							style:width="6px"
							style:height="6px"
							style:background="var(--mbv-progressPercent)"
							style:border-radius="3px 0 0 3px"
							title="Intro"
						></div>
					{/if}
					<div
						class="absolute left-0"
						data-played
						style:top="22px"
						style:width="{progress * 100}%"
						style:height="6px"
						style:background="var(--mbv-accent)"
						style:border-radius="3px"
					></div>
					{#each chapterTicks as ratio, i (i)}
						<div
							class="absolute"
							style:left="{ratio * 100}%"
							style:top="18px"
							style:width="2px"
							style:height="14px"
							style:background={roleVar('textMuted')}
						></div>
					{/each}
					{#if creditsLeft !== null}
						<div
							class="absolute"
							style:left="{creditsLeft * 100}%"
							style:top="22px"
							style:width="4px"
							style:height="6px"
							style:background="var(--mbv-progressPercent)"
							title="Credits"
						></div>
					{/if}
					{#if hover}
						<div
							class="pointer-events-none absolute"
							data-hover-line
							style:left="{hover.fraction * 100}%"
							style:top="14px"
							style:width="1px"
							style:height="22px"
							style:background={roleVar('playbackTitleFg')}
						></div>
					{/if}
					<div
						class="absolute -translate-x-1/2 rounded-full"
						data-thumb
						style:left="{progress * 100}%"
						style:top="17px"
						style:width="16px"
						style:height="16px"
						style:background={roleVar('playbackTitleFg')}
						style:box-shadow="0 0 0 3px var(--mbv-queuePlaybackBand)"
					></div>
					<div
						class="absolute inset-x-0 flex justify-between font-mono tabular-nums"
						style:top="42px"
						style:font-size="12px"
					>
						<span style:color="var(--mbv-accent)">{playback.elapsedClock}</span>
						<span style:color={roleVar('textMuted')}
							>{minus(playback.remainingClock)}{playback.finishClock
								? ` · ends ${playback.finishClock}`
								: ''}</span
						>
					</div>
				</div>
			</div>
			<!-- The transport (`main` audits 13–14): previous, `−10`, the 56 px
				round play/pause control, `+30`, and next, on the canvas gap and
				insets. The spec's list carries no stop control, so the canvas
				panel paints none. -->
			<div
				class="flex items-center justify-center"
				style:gap="4px"
				style:padding="0 16px 6px"
			>
				<button
					type="button"
					class="flex items-center justify-center"
					style:width="44px"
					style:height="44px"
					bind:this={prevEl}
					data-target="previous"
					aria-label="Previous"
				>
					<SkipBack
						size={20}
						fill={glyphColor(playback.availability.previous, 'textPrimary')}
						color={glyphColor(playback.availability.previous, 'textPrimary')}
					/>
				</button>
				<button
					type="button"
					class="flex items-center justify-center font-mono"
					style:width="44px"
					style:height="44px"
					style:font-size="11px"
					style:color={roleVar('textPrimary')}
					bind:this={seekBackEl}
					data-target="seekBack"
					aria-label="Back 10 seconds"
					title="Back 10 s"
				>
					−10
				</button>
				<button
					type="button"
					class="flex items-center justify-center rounded-full"
					style:width="56px"
					style:height="56px"
					style:background="var(--mbv-accent)"
					bind:this={playPauseEl}
					data-target="playPause"
					aria-label={playback.playback.paused ? 'Play' : 'Pause'}
				>
					{#if playback.playback.paused}
						<Play
							size={22}
							fill={roleVar('textOnAccent')}
							color={roleVar('textOnAccent')}
						/>
					{:else}
						<Pause
							size={22}
							fill={roleVar('textOnAccent')}
							color={roleVar('textOnAccent')}
						/>
					{/if}
				</button>
				<button
					type="button"
					class="flex items-center justify-center font-mono"
					style:width="44px"
					style:height="44px"
					style:font-size="11px"
					style:color={roleVar('textPrimary')}
					bind:this={seekForwardEl}
					data-target="seekForward"
					aria-label="Forward 30 seconds"
					title="Forward 30 s"
				>
					+30
				</button>
				<button
					type="button"
					class="flex items-center justify-center"
					style:width="44px"
					style:height="44px"
					bind:this={nextEl}
					data-target="next"
					aria-label="Next"
				>
					<SkipForward
						size={20}
						fill={glyphColor(playback.availability.next, 'textPrimary')}
						color={glyphColor(playback.availability.next, 'textPrimary')}
					/>
				</button>
			</div>
			<!-- The track chips row under the transport: the video label and
				the audio and subtitle chips with the open popover anchored below
				its chip, right-aligned. `trackChips` is `null` while the local
				player is inactive, so the row is absent then, as the
				status-indicator cluster is. -->
			<TrackChips chips={playback.trackChips} {overlay} />
		{/if}
	{/if}
</div>
