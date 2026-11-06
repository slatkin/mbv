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
	// times the TUI flanks it with.
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
	 * TUI's report precedence: the transport controls, the seekbar, and the
	 * header's playback-target indicator last. */
	function currentRegions(): LayoutRegion[] {
		return [
			region(playPauseEl, 'playPause'),
			region(stopEl, 'stop'),
			region(prevEl, 'previous'),
			region(nextEl, 'next'),
			region(seekbarEl, 'seekbar'),
			region(targetEl, 'playbackTarget'),
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
		report();
	});

	onMount(() => {
		const observer =
			typeof ResizeObserver === 'undefined'
				? undefined
				: new ResizeObserver(report);
		if (panelEl) {
			observer?.observe(panelEl);
		}
		return () => {
			observer?.disconnect();
			retractLayout(reportKey);
		};
	});
</script>

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
			as the TUI's `render_queue_panel` gates them; the frame carries no
			visual-slot artwork, so no `visualSlot` region exists. -->
		<div class="px-2 pb-2" style:background="var(--mbv-pillSelectedFg)">
			{#if playback.nowPlayingTitle}
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
							<Play size={16} fill="currentColor" color="var(--mbv-accent)" />
						{:else}
							<Pause
								size={16}
								fill="currentColor"
								color="var(--mbv-textFocusAccent)"
							/>
						{/if}
					</span>
					<span
						class="flex items-center"
						bind:this={stopEl}
						data-target="stop"
					>
						<Square
							size={14}
							fill="currentColor"
							color={playback.availability.stop
								? 'var(--mbv-statusError)'
								: 'var(--mbv-textMuted)'}
						/>
					</span>
					<span
						class="flex items-center"
						bind:this={prevEl}
						data-target="previous"
					>
						<SkipBack
							size={14}
							fill="currentColor"
							color={playback.availability.previous
								? 'var(--mbv-textStrong)'
								: 'var(--mbv-textMuted)'}
						/>
					</span>
					<span
						class="flex items-center"
						bind:this={nextEl}
						data-target="next"
					>
						<SkipForward
							size={14}
							fill="currentColor"
							color={playback.availability.next
								? 'var(--mbv-textStrong)'
								: 'var(--mbv-textMuted)'}
						/>
					</span>
				</div>
				<div
					class="flex items-center gap-1 px-2"
					style:height="{ROW_HEIGHT}px"
				>
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
			{/if}
		</div>
	{/if}
</div>
