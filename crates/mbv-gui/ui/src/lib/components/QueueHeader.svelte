<script lang="ts">
	// The queue's canvas header (align-gui-with-design-canvas U4.1, design
	// D15): the `Up next · N` heading over the `queue ends HH:MM` line at the
	// left, the "Length / Ends at" right-column switch and the filter control
	// at the right. All of it paints the frame's `summary` section — the
	// switch's mode, the filter's text, and the caret fact are Client-core
	// state (design D4a), so the header holds none.
	//
	// The header paints the canvas's own hexes through the queue-local
	// `--canvas-*` tokens (U13): the canvas names no `mbv-theme` slot for
	// them, and the TUI's palette stays untouched.
	//
	// The switch and the filter each carry a pointer target: the switch's
	// inactive segment and the filter report the `rightColumn` and `filter`
	// regions through `QueuePanel`'s own report key, so a press resolves in
	// the core to the `t` and `/` chords' requests — the control and the
	// chord run one path. The active segment reports no target: pressing it
	// would toggle away from the mode it already shows.
	import { ListFilter } from '@lucide/svelte';
	import type { GuiQueueSummary, GuiRightColumn } from '$lib/bridge/types';

	interface Props {
		summary: GuiQueueSummary;
	}
	const { summary }: Props = $props();

	/** The `var(--canvas-<name>)` value a part paints in: the queue's canvas
	 * hexes that name no theme slot (U13). */
	function canvasVar(name: string): string {
		return `var(--canvas-${name})`;
	}

	/** The right-column switch's two modes in paint order. */
	const MODES: { mode: GuiRightColumn; label: string }[] = [
		{ mode: 'length', label: 'Length' },
		{ mode: 'endsAt', label: 'Ends at' },
	];
</script>

<div
	class="flex items-center justify-between overflow-hidden whitespace-nowrap"
	style:height="var(--canvas-queue-header-height)"
	style:padding="12px 8px 8px 16px"
	data-queue-header
>
	<!-- The canvas's two heading lines, 2 px apart: the count over the
		queue's finish clock. -->
	<div class="flex flex-col" style:gap="2px">
		<span
			style:font-size="13px"
			style:line-height="18px"
			style:font-weight="600"
			style:color={canvasVar('heading')}
			data-queue-heading>Up next · {summary.remaining}</span
		>
		{#if summary.finishClock}
			<span
				class="font-mono"
				style:font-size="11px"
				style:line-height="16px"
				style:color={canvasVar('muted')}
				data-finish-clock>queue ends {summary.finishClock}</span
			>
		{/if}
	</div>
	<div class="flex items-center" style:gap="4px">
		<!-- The "Length / Ends at" switch: a recessed track the active segment
			fills, and a press on the inactive one runs the `t` chord's toggle
			in the core through the `rightColumn` region the segment reports. -->
		<div
			class="flex items-center"
			style:background={canvasVar('well')}
			style:border-radius="6px"
			style:padding="2px"
			title="Right column (t)"
			data-right-column-switch
		>
			{#each MODES as { mode, label } (mode)}
				{@const active = summary.rightColumn === mode}
				<span
					class="flex items-center"
					style:height="28px"
					style:padding-inline="10px"
					style:border-radius="4px"
					style:font-size="12px"
					style:background={active ? canvasVar('segment-active') : 'transparent'}
					style:color={active ? canvasVar('heading') : canvasVar('muted')}
					data-mode={mode}
					data-active={active || undefined}
					data-right-column-segment={active ? undefined : mode}>{label}</span
				>
			{/each}
		</div>
		<!-- The filter control: a magnifier button at rest, and the text entry
			with the block caret while the entry holds the keyboard
			(`filterActive`) or a filter narrows the rows. A press on it runs
			the `/` chord's text entry in the core through the `filter` region
			the mark reports. -->
		{#if summary.filterActive || summary.filter}
			<span
				class="flex items-center"
				style:height="28px"
				style:padding-inline="10px"
				style:border-radius="4px"
				style:gap="6px"
				style:font-size="12px"
				title="Filter (/)"
				data-filter
			>
				<ListFilter size={14} color={canvasVar('muted')} />
				<span style:color={summary.filter ? canvasVar('heading') : canvasVar('muted')}
					>{summary.filter || 'Filter'}</span
				>
				{#if summary.filterActive}
					<span
						class="inline-block w-px"
						style:height="12px"
						style:background={canvasVar('heading')}
						data-caret
					></span>
				{/if}
			</span>
		{:else}
			<button
				type="button"
				class="flex items-center justify-center"
				style:width="44px"
				style:height="44px"
				style:color={canvasVar('muted')}
				title="Filter (/)"
				aria-label="Filter queue"
				data-filter
			>
				<ListFilter size={17} color={canvasVar('muted')} />
			</button>
		{/if}
	</div>
</div>
