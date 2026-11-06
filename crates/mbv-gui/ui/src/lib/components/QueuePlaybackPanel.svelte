<script lang="ts">
	// The Queue playback panel (add-tauri-gui-client task 3.4, design D4): the
	// queue column's transport band. The header row paints in every state —
	// the playback-target indicator (a Lucide icon, local vs remote by its
	// role colour), the remote host name, and the right-aligned status word;
	// the now-playing title paints on its own row below the header. The
	// transport band paints only while the frame's `shown` says a transport
	// exists (idle collapses it), and its controls and seek row paint only
	// while a now-playing title rides, the same gates the TUI's
	// `render_queue_panel` applies.
	//
	// The paint mirrors the TUI's queue band
	// (crates/mbv-render/src/components/chrome_player/title/): the header and
	// band sit on the `QueueOnlyPlaybackPanel` surface, the fill
	// `--mbv-queuePlaybackBand` carries, and the controls row sits on
	// `TransportRow`, the BgDim1 slot `--mbv-secondary` carries (design D7).
	// Every offset is a `calc()` over the cell unit (`bridge/cells`): one row
	// is `--mbv-cell-row`, one column `--mbv-cell-col`.
	// Every control is a Lucide icon (design D8: no emoji or Nerd Font
	// glyph), and the seekbar is a filled track with the elapsed and total
	// times the TUI flanks it with. The transport glyphs paint as solid
	// shapes in their role colour — `fill` and `stroke` on the icon's own
	// colour, the TUI's solid Nerd Font glyphs — while the indicators and
	// every other icon keep Lucide's outline shapes.
	//
	// The component interprets no pointer event: it paints and reports its
	// regions through the shared layout reporter, and the core resolves a
	// press against them.
	import { onMount } from 'svelte';
	import {
		Cast,
		Pause,
		Play,
		SkipBack,
		SkipForward,
		Square,
	} from '@lucide/svelte';
	import { reportLayout, retractLayout } from '$lib/bridge/layout';
	import { CELL_COL_PX, CELL_ROW_PX } from '$lib/bridge/cells';
	import { roleProperty } from '$lib/bridge/theme';
	import type {
		GuiCardArtwork,
		GuiNowPlayingStatus,
		GuiQueuePlayback,
		LayoutRegion,
		QueuePlaybackTargetId,
	} from '$lib/bridge/types';

	interface Props {
		playback: GuiQueuePlayback;
	}
	const { playback }: Props = $props();

	/** The fixed row height in logical pixels, the queue list's row: the
	 * GUI's cell unit's row (design D8's geometry basis, `bridge/cells`). */
	const ROW_HEIGHT = CELL_ROW_PX;

	/** The visual slot's height cap in rows, mirroring the TUI's
	 * `queue_card_height_cap`: 12 rows under 40 terminal rows, 16 under 50,
	 * 24 otherwise — kept small enough that the queue list below keeps its
	 * separator and a few rows. The terminal-row count reads off the
	 * window's own height in row units. */
	function cardHeightCapRows(windowHeight: number): number {
		const rows = Math.floor(windowHeight / ROW_HEIGHT);
		if (rows < 40) {
			return 12;
		}
		return rows < 50 ? 16 : 24;
	}

	/** The window's height in logical pixels, refreshed by the layout
	 * observer, so the cap tracks a window resize like the TUI's per-paint
	 * tier reads the terminal height. */
	let windowHeight = $state(window.innerHeight);

	/** The visual slot's height cap in logical pixels. */
	const artCapPx = $derived(cardHeightCapRows(windowHeight) * ROW_HEIGHT);

	/** The column width at which the queue column's visual slot and
	 * transport render side by side rather than stacked: the TUI's
	 * `queue_playback_column_wide` breakpoint, 100 columns
	 * (`QUEUE_PLAYBACK_WIDE_COLUMNS` in `arrangements/chrome.rs`), carried
	 * in cell units — no pixel threshold of the GUI's own. */
	const WIDE_COLUMNS = 100;

	/** The queue column's width in column cells, refreshed by the layout
	 * observer with the window height. */
	let columnCells = $state(0);

	/** Whether the band takes the TUI's wide arrangement: art left-aligned
	 * with the transport beside it after the two-column gap
	 * (`queue_playback_transport_area`'s wide branch), stacked below it
	 * otherwise. */
	const wide = $derived(columnCells >= WIDE_COLUMNS);

	/** Emby ticks in one second (`mbv_emby_model::TICKS_PER_SECOND`). */
	const TICKS_PER_SECOND = 10_000_000;

	const reportKey = Symbol('queue-playback-panel');
	let panelEl: HTMLElement | undefined = $state();
	let targetEl: HTMLElement | undefined = $state();
	let playPauseEl: HTMLElement | undefined = $state();
	let stopEl: HTMLElement | undefined = $state();
	let prevEl: HTMLElement | undefined = $state();
	let nextEl: HTMLElement | undefined = $state();
	let seekbarEl: HTMLElement | undefined = $state();
	let artEl: HTMLElement | undefined = $state();

	/** The visual slot's artwork the last frame carried (design D10): a
	 * frame arms it again only when the core says the art changed, so the
	 * slot keeps painting between frames. */
	let art = $state<GuiCardArtwork | null>(null);

	/** The queue band's status-indicator cluster; empty while the frame
	 * carries none. */
	const statusIndicators = $derived(playback.statusIndicators ?? []);

	/** `fmt_duration_short`: `M:SS` under an hour, `H:MM:SS` at or over it. */
	function fmtDurationShort(ticks: number): string {
		const s = Math.trunc(ticks / TICKS_PER_SECOND);
		if (s >= 3600) {
			const mm = String(Math.trunc((s % 3600) / 60)).padStart(2, '0');
			const ss = String(s % 60).padStart(2, '0');
			return `${Math.trunc(s / 3600)}:${mm}:${ss}`;
		}
		return `${Math.trunc(s / 60)}:${String(s % 60).padStart(2, '0')}`;
	}

	/** The played fraction of the seekbar, clamped to `[0, 1]`. */
	function seekRatio(): number {
		const { positionTicks, runtimeTicks } = playback.playback;
		if (runtimeTicks <= 0) {
			return 0;
		}
		return Math.min(1, Math.max(0, positionTicks / runtimeTicks));
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

	/** The painted regions this frame's DOM reports (design D4), in the
	 * TUI's report precedence: the transport controls, the seekbar, the
	 * visual slot, and the header's playback-target indicator last. */
	function currentRegions(): LayoutRegion[] {
		return [
			region(playPauseEl, 'playPause'),
			region(stopEl, 'stop'),
			region(prevEl, 'previous'),
			region(nextEl, 'next'),
			region(seekbarEl, 'seekbar'),
			region(artEl, 'visualSlot'),
			region(targetEl, 'playbackTarget'),
		].filter((entry) => entry !== null);
	}

	function report(): void {
		windowHeight = window.innerHeight;
		columnCells = Math.floor((panelEl?.clientWidth ?? 0) / CELL_COL_PX);
		reportLayout(reportKey, { lists: [], regions: currentRegions() });
	}

	/** The observer the mount and the art element's effect share; the
	 * art element's own resizes report (design D4) — the artwork's
	 * intrinsic size moves the transport column beside it. */
	let artObserver: ResizeObserver | undefined;

	// Each paint that moved a region reports (design D4): this effect runs
	// after the DOM update, and the ResizeObserver reports layout-driven
	// moves. The reporter dedupes an unchanged contribution.
	$effect(() => {
		void playback;
		report();
	});

	// The slot's artwork adoption (design D10): a frame's carried value
	// replaces the last one; a frame without one keeps painting it.
	$effect(() => {
		if (playback.artwork) {
			art = playback.artwork;
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
		};
	});
</script>

<div style:padding="var(--mbv-cell-row) 0" bind:this={panelEl}>
	<!-- The header row: it paints in every state, idle included. The root
		carries the queue placement's padding in row units: the TUI's
		playback band sits one row below the column's top edge and two
		columns inside it (`queue_panel_inset`), and its last row is the gap
		row that keeps the band clear of the recessed queue panel below.
		The row paints on the `QueueOnlyPlaybackPanel` fill
		(`--mbv-queuePlaybackBand`) with the header painter's one-cell
		interior padding each side. -->
	<div
		class="flex items-center overflow-hidden whitespace-nowrap"
		style:height="var(--mbv-cell-row)"
		style:margin-inline="calc(var(--mbv-cell-col) * 2)"
		style:padding-inline="var(--mbv-cell-col)"
		style:background="var(--mbv-queuePlaybackBand)"
	>
		<span
			class="flex shrink-0 items-center"
			style:gap="var(--mbv-cell-col)"
			bind:this={targetEl}
			data-target="playbackTarget"
		>
			{#if playback.hostIsRemote}
				<!-- The TUI's target glyph: `target_icon` paints the connected
					cast glyph while a remote host owns playback and the cast glyph
					otherwise, in the target colour (yellow local, green remote);
					Lucide's one `Cast` carries both states, so the colour and the
					host label carry the distinction. -->
				<Cast size={CELL_COL_PX} color="var(--mbv-playbackTargetRemoteFg)" />
				{#if playback.host}
					<span class="truncate" style:color={roleVar('textPrimary')}
						>{playback.host}</span
					>
				{/if}
			{:else}
				<Cast size={CELL_COL_PX} color="var(--mbv-playbackTargetLocalFg)" />
			{/if}
		</span>
		<span class="flex-1"></span>
		<span class="font-bold" style:color={statusColor(playback.status)}
			>{statusWord(playback.status)}</span
		>
	</div>
	{#if playback.shown}
		<!-- The transport band on the `QueueOnlyPlaybackPanel` fill
			(`--mbv-queuePlaybackBand`). The band sits two columns inside the
			column (`queue_panel_inset`), and every row height is one cell
			row. The arrangement is the TUI's width-driven one
			(`queue_playback_transport_area`): at 100 column cells and wider
			the artwork sits left-aligned with the transport beside it after
			the two-column gap (`SLOT_TRANSPORT_GAP`) and the taller of the
			two governs the band's height; below that the artwork centers and
			the transport's four rows stack below it at the band's full
			width. The four rows are the controls with the status text, a
			blank row, the seekbar with its flanking times, one blank gap row
			(`queue_band_rows`). The controls and seek rows paint their
			content only while a now-playing title rides; without one the TUI
			paints the four rows as blanks on the band's own fill, so the GUI
			does too. -->
		<div
			style:margin-inline="calc(var(--mbv-cell-col) * 2)"
			style:background="var(--mbv-queuePlaybackBand)"
		>
			{#snippet artwork(artwork: Exclude<GuiCardArtwork, { kind: 'hidden' }>)}
				{#if artwork.kind === 'loading'}
					<div
						class="aspect-square"
						bind:this={artEl}
						data-target="visualSlot"
						style:width="min(100%, {artCapPx}px)"
						style:background="var(--mbv-textMuted)"
					></div>
				{:else}
					<img
						class="max-w-full object-contain"
						alt=""
						bind:this={artEl}
						data-target="visualSlot"
						src={artwork.detail}
						style:max-height="{artCapPx}px"
					/>
				{/if}
			{/snippet}
			{#snippet bandRows()}
				{#if playback.nowPlayingTitle}
					<div
						class="flex items-center"
						style:height="var(--mbv-cell-row)"
						style:padding-inline="var(--mbv-cell-col)"
						style:gap="var(--mbv-cell-col)"
						style:background="var(--mbv-secondary)"
					>
						<span
							class="flex items-center"
							bind:this={playPauseEl}
							data-target="playPause"
						>
							{#if playback.playback.paused}
								<Play
									size={16}
									fill={roleVar('accent')}
									color={roleVar('accent')}
								/>
							{:else}
								<Pause
									size={16}
									fill={roleVar('textFocusAccent')}
									color={roleVar('textFocusAccent')}
								/>
							{/if}
						</span>
						<span class="flex items-center" bind:this={stopEl} data-target="stop">
							<Square
								size={14}
								fill={glyphColor(playback.availability.stop, 'statusError')}
								color={glyphColor(playback.availability.stop, 'statusError')}
							/>
						</span>
						<span class="flex items-center" bind:this={prevEl} data-target="previous">
							<SkipBack
								size={14}
								fill={glyphColor(playback.availability.previous, 'textStrong')}
								color={glyphColor(playback.availability.previous, 'textStrong')}
							/>
						</span>
						<span class="flex items-center" bind:this={nextEl} data-target="next">
							<SkipForward
								size={14}
								fill={glyphColor(playback.availability.next, 'textStrong')}
								color={glyphColor(playback.availability.next, 'textStrong')}
							/>
						</span>
						{#if statusIndicators.length > 0}
							<!-- The queue band's status-indicator cluster, right-aligned on
								the controls row in both arrangements as the TUI paints it:
								the uppercased labels, one space apart, each in its role
								colour. The TUI reports no hit region for the cluster, so it
								carries no target. -->
							<span
								class="ml-auto flex items-center font-bold"
								style:gap="var(--mbv-cell-col)"
							>
								{#each statusIndicators as indicator, i (i)}
									<span style:color={roleVar(indicator.role)}>{indicator.label}</span>
								{/each}
							</span>
						{/if}
					</div>
					<!-- The band's blank row between the controls and the seekbar: the
						TUI's former title row, kept blank (the title lives elsewhere). -->
					<div style:height="var(--mbv-cell-row)"></div>
					<div
						class="flex items-center"
						style:height="var(--mbv-cell-row)"
						style:padding-inline={
							playback.showControls ? 'var(--mbv-cell-col)' : '0'
						}
						style:gap="var(--mbv-cell-col)"
					>
						{#if playback.showControls}
							<span
								class="shrink-0 font-mono"
								style:color={roleVar('playbackMetaFg')}
								>{fmtDurationShort(playback.playback.positionTicks)}</span
							>
							<div
								class="h-full min-w-4 flex-1"
								bind:this={seekbarEl}
								data-target="seekbar"
								style:background="var(--mbv-secondary)"
							>
								<div
									class="h-full"
									style:width="{seekRatio() * 100}%"
									style:background="var(--mbv-accent)"
								></div>
							</div>
							<span
								class="shrink-0 font-mono"
								style:color={roleVar('playbackMetaFg')}
								>{fmtDurationShort(playback.playback.runtimeTicks)}</span
							>
						{:else}
							<!-- The controls and seekbar carry no playback: the TUI paints
								the track indicator across the row's full width with no inset
								and no times, and it resolves no seek. -->
							<div
								class="h-full flex-1"
								style:background="var(--mbv-progressTrack)"
							></div>
						{/if}
					</div>
					<!-- The band's trailing blank row, on the band's own fill. -->
					<div style:height="var(--mbv-cell-row)"></div>
				{:else}
					<!-- Without a now-playing title the TUI paints the four band rows
						as blanks on the band's own fill (`render_queue_panel`): the
						controls, former-title, seek, and gap rows. -->
					<div style:height="var(--mbv-cell-row)"></div>
					<div style:height="var(--mbv-cell-row)"></div>
					<div style:height="var(--mbv-cell-row)"></div>
					<div style:height="var(--mbv-cell-row)"></div>
				{/if}
			{/snippet}
			{#if playback.nowPlayingTitle}
				<!-- The now-playing title's own row, below the column top and
					inset one column each side: the TUI composites the artwork-site
					title into the image's top text row, so the header row paints
					only the target and the status word. The frame carries the
					plain title in the projection's `PlaybackValueFg` role; the
					TUI's scrolling marquee needs a clock the frame does not carry,
					so the row truncates with the slice's ellipsis treatment. -->
				<div
					class="flex items-center overflow-hidden whitespace-nowrap"
					style:height="var(--mbv-cell-row)"
					style:padding-inline="var(--mbv-cell-col)"
				>
					<span
						class="truncate"
						style:color={roleVar('playbackValueFg')}
						>{playback.nowPlayingTitle}</span
					>
				</div>
			{/if}
			{#if art && art.kind !== 'hidden'}
				<!-- The visual slot: `render_card_painting` fits the image
					within the slot region's width and the queue column's height
					tier (`queue_card_height_cap`) — left-aligned in the wide
					arrangement, centered when stacked. No fixed box: the image's
					own fit sizes the rows, and the loading reservation is the
					TUI's capped square estimate. -->
				{#if wide}
					<div
						class="flex items-center"
						style:gap="calc(var(--mbv-cell-col) * 2)"
					>
						{@render artwork(art)}
						<div class="flex min-w-0 flex-1 flex-col">
							{@render bandRows()}
						</div>
					</div>
				{:else}
					<div class="flex items-center justify-center">
						{@render artwork(art)}
					</div>
					{@render bandRows()}
				{/if}
			{:else}
				{@render bandRows()}
			{/if}
		</div>
	{/if}
</div>
