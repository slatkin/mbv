<script lang="ts">
	// The canvas footer line (add-tauri-gui-client row 6.4, design D4): one
	// row at the queue column's bottom that merges what the TUI paints as
	// the queue footer's status pills and the Status bar's prefix and toast
	// rows — the queue source, the session marker, the prefix-armed state,
	// and the toasts.
	//
	// The left group is the queue source: the autosave indicator (while
	// save-on-consume is on for a saved playlist), then the playlist pill —
	// the pill's icon-plus-name span is its hit region (`playlistPill`,
	// reported under the queue surface, as the Queue panel's pill was). The
	// right group is the dirty indicator (while the queue differs from its
	// saved playlist), then the session marker: the footer owner pill's
	// label — the Player-owner label (`woims` on the canvas) — with NO
	// liveness dot: the frame carries no liveness field (the D4a answer
	// names the dot but the wire does not send it), so none is painted.
	//
	// The prefix-armed state paints the same `PREFIX` badge the TUI's
	// status row paints while prefix mode is armed, and it drops while a
	// toast is live — the TUI's pill-drop rule carried into the line. The
	// toast paints inline in the line, centred in its severity colour:
	// the spec permits the footer to show toasts in the line or in a
	// conventional GUI form. A live toast moves no layout: the line is
	// always painted and the toast only fills its centre span.
	//
	// The row keeps the TUI's `render_queue_status` placement: the footer
	// band sits two columns inside the placement on the `queueFooterBand`
	// fill, between the gap row above it and the placement's bottom
	// padding row — both keep the queue column's backdrop. Each pill's
	// slot maps to its Lucide icon where the TUI paints a Nerd Font glyph
	// (design D8): the playlist source's `playlist-play` → `list-music`,
	// the owner indicator's `connection` → `cable`, the autosave
	// `content-save-check` → `save-check`, and the dirty
	// `content-save-alert` → `save-off`.
	//
	// The slice's `GuiStatusBar` carries no volume, mute, or visual-mode
	// content, so the line paints no interactive status pill and reports no
	// statusBar-surface region — a point on the line resolves to nothing
	// but the playlist pill. It still reports through the shared layout
	// reporter so the merged report accounts for the mounted surface.
	import { onMount } from 'svelte';
	import { Cable, ListMusic, SaveCheck, SaveOff } from '@lucide/svelte';
	import { reportLayout, retractLayout } from '$lib/bridge/layout';
	import { roleProperty } from '$lib/bridge/theme';
	import type {
		GuiQueueFooter,
		GuiRole,
		GuiStatusBar,
		GuiToastSeverity,
		LayoutRegion,
	} from '$lib/bridge/types';

	interface Props {
		footer: GuiQueueFooter;
		status: GuiStatusBar;
	}
	const { footer, status }: Props = $props();

	const reportKey = Symbol('queue-footer');
	let lineEl: HTMLElement | undefined = $state();
	let pillEl: HTMLElement | undefined = $state();

	/** The `var(--mbv-<role>)` value a part paints in. */
	function roleVar(role: GuiRole): string {
		return `var(${roleProperty(role)})`;
	}

	/** The toast text's severity foreground (`toast_spans` in the TUI's
	 * status bar panel). */
	function toastColor(severity: GuiToastSeverity): string {
		switch (severity) {
			case 'neutral':
				return 'var(--mbv-textPrimary)';
			case 'success':
				return 'var(--mbv-statusAvailable)';
			case 'warning':
				return 'var(--mbv-textFocusAccent)';
			case 'error':
				return 'var(--mbv-statusError)';
		}
	}

	/** The painted regions this frame's DOM reports (design D4): the
	 * playlist pill, the line's only interactive target. */
	function currentRegions(): LayoutRegion[] {
		if (!pillEl) {
			return [];
		}
		const rect = pillEl.getBoundingClientRect();
		return [
			{
				rect: { x: rect.x, y: rect.y, width: rect.width, height: rect.height },
				target: { surface: 'queue', id: 'playlistPill' },
			},
		];
	}

	function report(): void {
		reportLayout(reportKey, { lists: [], regions: currentRegions() });
	}

	// Each paint that moved a region reports (design D4): this effect runs
	// after the DOM update, and the ResizeObserver reports layout-driven
	// moves. The reporter dedupes an unchanged contribution.
	$effect(() => {
		void footer;
		void status;
		report();
	});

	onMount(() => {
		const observer =
			typeof ResizeObserver === 'undefined'
				? undefined
				: new ResizeObserver(report);
		if (lineEl) {
			observer?.observe(lineEl);
		}
		return () => {
			observer?.disconnect();
			retractLayout(reportKey);
		};
	});
</script>

<!-- The canvas footer line and its clearance: the gap row above the band
	and the placement's bottom padding row below it keep the queue column's
	backdrop. The band itself sits two columns inside the placement on the
	`queueFooterBand` fill. -->
<div style:height="var(--mbv-cell-row)"></div>
<div
	class="flex items-center overflow-hidden whitespace-nowrap"
	style:height="var(--mbv-cell-row)"
	style:margin-inline="calc(var(--mbv-cell-col) * 2)"
	style:background="var(--mbv-queueFooterBand)"
	bind:this={lineEl}
>
	<!-- The left group: the prefix badge while armed (dropped while a toast
		is live, the TUI's pill-drop rule), then the queue source — the
		autosave indicator and the playlist pill — in one-column pads on the
		status-pill fill, as `render_queue_status` paints the group. -->
	<div
		class="flex items-center"
		style:background="var(--mbv-pillSelectedFg)"
		style:padding-inline="var(--mbv-cell-col)"
		style:gap="var(--mbv-cell-col)"
	>
		{#if status.prefixArmed && !status.toast}
			<span class="font-bold" style:color="var(--mbv-accent)">PREFIX</span>
		{/if}
		{#if footer.autosave}
			<SaveCheck size={14} color={roleVar(footer.autosave.role)} class="shrink-0" />
		{/if}
		<span bind:this={pillEl} class="flex items-center" style:gap="var(--mbv-cell-col)">
			<ListMusic size={14} color={roleVar(footer.playlist.role)} class="shrink-0" />
			{#if footer.playlist.text}
				<span style:color={roleVar(footer.playlist.role)}>{footer.playlist.text}</span>
			{/if}
		</span>
	</div>
	<!-- The toast, inline in the line's centre in its severity colour (the
		footer-line toast form the spec permits); `null` while none is live. -->
	<span
		class="min-w-0 flex-1 truncate text-center"
		style:color={status.toast ? toastColor(status.toast.severity) : undefined}
	>
		{#if status.toast}{status.toast.text}{/if}
	</span>
	<!-- The right group: the dirty pill (when present) wrapped in
		one-column pads on the pill fill, then the session marker — the
		footer owner pill's label — flush at the row's right edge, with no
		liveness dot (the frame carries no liveness field). -->
	<div class="flex items-center">
		{#if footer.dirty}
			<span
				class="flex items-center"
				style:background="var(--mbv-pillSelectedFg)"
				style:padding-inline="var(--mbv-cell-col)"
				style:gap="var(--mbv-cell-col)"
			>
				<SaveOff size={14} color={roleVar(footer.dirty.role)} class="shrink-0" />
				<span class:font-bold={footer.dirty.bold} style:color={roleVar(footer.dirty.role)}
					>{footer.dirty.text}</span
				>
			</span>
		{/if}
		<span
			class="flex items-center"
			style:background="var(--mbv-pillSelectedFg)"
			style:gap="var(--mbv-cell-col)"
		>
			<Cable size={14} color={roleVar(footer.owner.role)} class="shrink-0" />
			<span style:color={roleVar(footer.owner.role)}>{footer.owner.text}</span>
		</span>
	</div>
</div>
<div style:height="var(--mbv-cell-row)"></div>
