<script lang="ts">
	// The panel's folded queue (add-tauri-gui-client row 6.6, design D10):
	// the expanded artboard after the `q` toggle. The canvas replaces the
	// queue's rows with one line — the remaining count, the queue finish
	// clock, and the control that expands the queue again.
	//
	// The expand control reports the `queueFold` target, the id the core's
	// one `q` toggle runs from (the canvas paints no header fold control;
	// `q` folds the queue). The line keeps the queue box's fill and its
	// two-column inset, so the fold does not move the neighbouring bands'
	// edges.
	import { onMount } from 'svelte';
	import { ChevronDown } from '@lucide/svelte';
	import { elementRegion, observeLayout, reportLayout } from '$lib/bridge/layout';
	import { roleVar } from '$lib/bridge/theme';
	import type { GuiQueueSummary, LayoutRegion } from '$lib/bridge/types';

	interface Props {
		summary: GuiQueueSummary;
	}
	const { summary }: Props = $props();

	const reportKey = Symbol('folded-queue');
	let lineEl: HTMLElement | undefined = $state();
	let expandEl: HTMLElement | undefined = $state();

	/** The painted regions (design D4): the expand control, the line's one
	 * interactive target. */
	function currentRegions(): LayoutRegion[] {
		const region = elementRegion(expandEl, {
			surface: 'queuePlayback',
			id: 'queueFold'
		});
		return region === null ? [] : [region];
	}

	function report(): void {
		reportLayout(reportKey, { lists: [], regions: currentRegions() });
	}

	// Each paint that moved a region reports (design D4): this effect runs
	// after the DOM update, and the ResizeObserver reports layout-driven
	// moves. The reporter dedupes an unchanged contribution.
	$effect(() => {
		void summary;
		report();
	});

	onMount(() => observeLayout(reportKey, lineEl, report));
</script>

<!-- The folded queue's one line, on the queue box's fill and its
	two-column inset: the Queue label, the remaining count, the finish clock
	at the trailing edge, and the expand control. -->
<div
	class="flex shrink-0 items-center overflow-hidden whitespace-nowrap"
	style:height="var(--mbv-cell-row)"
	style:margin-inline="calc(var(--mbv-cell-col) * 2)"
	style:padding-inline="calc(var(--mbv-cell-col) * 2)"
	style:gap="var(--mbv-cell-col)"
	style:background="var(--mbv-queuePanel)"
	bind:this={lineEl}
>
	<span class="text-xs font-semibold tracking-widest uppercase" style:color={roleVar('textMuted')}
		>Queue</span
	>
	<span
		class="font-mono text-xs tabular-nums"
		style:color={roleVar('textSecondary')}
		data-remaining>{summary.remaining}</span
	>
	<span class="flex-1"></span>
	{#if summary.finishClock}
		<span
			class="font-mono text-xs tabular-nums"
			style:color={roleVar('textMuted')}
			data-finish-clock>{summary.finishClock}</span
		>
	{/if}
	<button
		type="button"
		class="flex items-center justify-center rounded-sm p-1"
		bind:this={expandEl}
		data-target="queueFold"
		aria-label="Expand queue"
		title="Expand queue (q)"
	>
		<ChevronDown size={16} color={roleVar('textMuted')} />
	</button>
</div>
