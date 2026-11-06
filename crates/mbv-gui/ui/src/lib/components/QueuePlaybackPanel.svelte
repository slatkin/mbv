<script lang="ts">
	// The Queue playback panel (add-tauri-gui-client task 3.4, design D4): the
	// queue column's transport band. The header row paints in every state —
	// the playback-target indicator (a Lucide icon, local vs remote by its
	// role colour), the remote host name, the now-playing title, and the
	// right-aligned status word. The transport band paints only while the
	// frame's `shown` says a transport exists (idle collapses it), and its
	// controls and seek row paint only while a now-playing title rides, the
	// same gates the TUI's `render_queue_panel` applies.
	//
	// The paint mirrors the TUI's queue band
	// (crates/mbv-render/src/components/chrome_player/title/): the header and
	// band sit on the `QueueOnlyPlaybackPanel` surface — the BgDim slot
	// `--mbv-pillSelectedFg` carries — and the controls row sits on
	// `TransportRow`, the BgDim1 slot `--mbv-secondary` carries (design D7).
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
		Monitor,
		Pause,
		Play,
		SkipBack,
		SkipForward,
		Square,
	} from '@lucide/svelte';
	import { reportLayout, retractLayout } from '$lib/bridge/layout';
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

	/** The fixed row height in logical pixels, the queue list's row. */
	const ROW_HEIGHT = 28;

	/** The visual slot's box height in logical points (design D10): the
	 * TUI's height-cap tiers are terminal-row geometry; the slot's box
	 * fixes at this height and the artwork contains itself inside it. */
	const ART_HEIGHT = 224;

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

{#snippet transportRows()}
	<div
		class="flex items-center gap-2 px-2"
		style:height="{ROW_HEIGHT}px"
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
	</div>
	<div class="flex items-center gap-1 px-2" style:height="{ROW_HEIGHT}px">
		{#if playback.showControls}
			<span
				class="shrink-0 font-mono"
				style:color={roleVar('playbackMetaFg')}
				>{fmtDurationShort(playback.playback.positionTicks)}</span
			>
			<div
				class="h-1.5 min-w-4 flex-1 overflow-hidden"
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
			<!-- The controls and seekbar carry no playback: the empty
				track paints and resolves no seek. -->
			<div
				class="h-1.5 min-w-4 flex-1"
				style:background="var(--mbv-progressTrack)"
			></div>
		{/if}
	</div>
{/snippet}

<div class="bg-card" bind:this={panelEl}>
	<!-- The header row: it paints in every state, idle included. -->
	<div
		class="flex items-center gap-1 overflow-hidden px-2 whitespace-nowrap"
		style:height="{ROW_HEIGHT}px"
		style:background="var(--mbv-pillSelectedFg)"
	>
		<span
			class="flex shrink-0 items-center gap-1"
			bind:this={targetEl}
			data-target="playbackTarget"
		>
			{#if playback.hostIsRemote}
				<Cast size={14} color="var(--mbv-playbackTargetRemoteFg)" />
				{#if playback.host}
					<span class="truncate" style:color={roleVar('textPrimary')}
						>{playback.host}</span
					>
				{/if}
			{:else}
				<Monitor size={14} color="var(--mbv-playbackTargetLocalFg)" />
			{/if}
		</span>
		{#if playback.nowPlayingTitle}
			<span class="truncate" style:color={roleVar('playbackContextFg')}
				>{playback.nowPlayingTitle}</span
			>
		{/if}
		<span class="flex-1"></span>
		<span class="font-bold" style:color={statusColor(playback.status)}
			>{statusWord(playback.status)}</span
		>
	</div>
	{#if playback.shown}
		<!-- The transport band on the `QueueOnlyPlaybackPanel` fill. The
			controls and seek rows paint only while a now-playing title rides,
			as the TUI's `render_queue_panel` gates them. While the visual
			slot's artwork paints (design D10), the band takes the TUI's wide
			arrangement: the artwork left, the transport beside it; without
			one, the transport spans the band. -->
		<div class="px-2 pb-2" style:background="var(--mbv-pillSelectedFg)">
			{#if playback.nowPlayingTitle}
				{#if art && art.kind !== 'hidden'}
					<div class="flex" style:height="{ART_HEIGHT}px">
						<div class="flex min-w-0 items-center justify-center">
							{#if art.kind === 'loading'}
								<div
									class="aspect-square h-full"
									bind:this={artEl}
									data-target="visualSlot"
									style:background="var(--mbv-textMuted)"
								></div>
							{:else}
								<img
									class="h-full max-w-full object-contain"
									alt=""
									bind:this={artEl}
									data-target="visualSlot"
									src={art.detail}
								/>
							{/if}
						</div>
						<div class="flex min-w-0 flex-1 flex-col">
							{@render transportRows()}
						</div>
					</div>
				{:else}
					{@render transportRows()}
				{/if}
			{/if}
		</div>
	{/if}
</div>
