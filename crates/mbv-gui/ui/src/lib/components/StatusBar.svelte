<script lang="ts">
	// The Status bar (add-tauri-gui-client task 3.4, design D4): the floating
	// status row. The frame carries the Status bar's Surface state — the
	// prefix-armed display and the live toast — so the bar shows the armed
	// pill while prefix mode is armed, and a live toast takes over the whole
	// row in its severity colour, centred, as the TUI's `render_status_bar`
	// paints it (spec toast-notification-semantics: the pills drop while a
	// toast is live).
	//
	// The row floats over the queue panel placement's bottom padding row —
	// the TUI's queue-only mode reserves no status-bar band, so the slice
	// overlays the one-row band on that pad row: the queue footer keeps its
	// one-row clearance above the window bottom, the idle bar paints
	// nothing (the slice's bar carries no pill content, so an idle bar
	// would otherwise paint a contentless band in the palette's darkest
	// tier — the user-reported black bar under the queue footer), and a
	// toast's arrival moves no layout. The row's fill is the `StatusBar`
	// surface — the BgDim slot `--mbv-pillSelectedFg` carries — inset two
	// columns each side like the TUI's `status_bar_row`.
	//
	// The slice's `GuiStatusBar` carries no volume, mute, or visual-mode
	// content, so the bar paints no interactive pill and reports an empty
	// region set — the prefix pill is non-interactive in the TUI too, and a
	// point on the bar resolves to nothing. It still reports through the
	// shared layout reporter so the merged report accounts for the mounted
	// surface.
	import { onMount } from 'svelte';
	import { reportLayout, retractLayout } from '$lib/bridge/layout';
	import type { GuiStatusBar, GuiToastSeverity } from '$lib/bridge/types';

	interface Props {
		status: GuiStatusBar;
	}
	const { status }: Props = $props();

	const reportKey = Symbol('status-bar');
	let barEl: HTMLElement | undefined = $state();

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

	function report(): void {
		reportLayout(reportKey, { lists: [], regions: [] });
	}

	// The bar paints no interactive region, so a paint never moves one; the
	// mount-time report still registers the surface's (empty) contribution.
	$effect(() => {
		void status;
		report();
	});

	onMount(() => {
		const observer =
			typeof ResizeObserver === 'undefined'
				? undefined
				: new ResizeObserver(report);
		if (barEl) {
			observer?.observe(barEl);
		}
		return () => {
			observer?.disconnect();
			retractLayout(reportKey);
		};
	});
</script>

<div
	class="absolute inset-x-0 bottom-0"
	style:height="var(--mbv-cell-row)"
	bind:this={barEl}
>
	<div
		class="flex h-full items-center overflow-hidden whitespace-nowrap"
		style:margin-inline="calc(var(--mbv-cell-col) * 2)"
		style:background={status.toast || status.prefixArmed
			? 'var(--mbv-pillSelectedFg)'
			: undefined}
	>
		{#if status.toast}
			<span
				class="w-full truncate text-center"
				style:color={toastColor(status.toast.severity)}>{status.toast.text}</span
			>
		{:else if status.prefixArmed}
			<span
				class="font-bold"
				style:padding-inline="var(--mbv-cell-col)"
				style:color="var(--mbv-accent)">PREFIX</span
			>
		{/if}
	</div>
</div>
