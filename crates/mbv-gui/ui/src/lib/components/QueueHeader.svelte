<script lang="ts">
	// The queue's canvas header (add-tauri-gui-client row 6.5, design D15):
	// the remaining count and the queue finish clock at the left, the
	// "Length / Ends at" right-column switch and the filter control at the
	// right. All of it paints the frame's `summary` section — the switch's
	// mode, the filter's text, and the caret fact are Client-core state
	// (design D4a), so the header holds none.
	//
	// The switch and the filter each carry a pointer target: the marked
	// elements report the `rightColumn` and `filter` regions through
	// `QueuePanel`'s own report key, so a press resolves in the core to
	// the `t` and `/` chords' requests — the control and the chord run
	// one path.
	import { ListFilter } from '@lucide/svelte';
	import { roleProperty } from '$lib/bridge/theme';
	import type { GuiQueueSummary, GuiRightColumn } from '$lib/bridge/types';

	interface Props {
		summary: GuiQueueSummary;
	}
	const { summary }: Props = $props();

	/** The `var(--mbv-<role>)` value a part paints in. */
	function roleVar(role: string): string {
		return `var(${roleProperty(role)})`;
	}

	/** The right-column switch's two modes in paint order. */
	const MODES: { mode: GuiRightColumn; label: string }[] = [
		{ mode: 'length', label: 'Length' },
		{ mode: 'endsAt', label: 'Ends at' },
	];
</script>

<div
	class="flex items-center whitespace-nowrap"
	style:height="var(--mbv-cell-row)"
	style:padding-inline="calc(var(--mbv-cell-col) * 2)"
	style:gap="var(--mbv-cell-col)"
	data-queue-header
>
	<span
		class="text-xs font-semibold tracking-widest uppercase"
		style:color={roleVar('textMuted')}>Queue</span
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
	<!-- The "Length / Ends at" switch: the frame's mode pills the active
		segment, and a press on it runs the `t` chord's toggle in the core
		through the `rightColumn` region the mark reports. -->
	<div
		class="flex items-center rounded-full border text-xs"
		style:border-color="var(--mbv-border)"
		title="Right column (t)"
		data-right-column-switch
	>
		{#each MODES as { mode, label } (mode)}
			{@const active = summary.rightColumn === mode}
			<span
				class="rounded-full px-2 py-0.5"
				style:background={active ? 'var(--mbv-secondary)' : undefined}
				style:color={active ? roleVar('textPrimary') : roleVar('textMuted')}
				data-mode={mode}
				data-active={active || undefined}>{label}</span
			>
		{/each}
	</div>
	<!-- The filter control: the icon, the filter text (or the muted
		placeholder), and the block caret while the entry holds the keyboard
		(`filterActive`). A press on it runs the `/` chord's text entry in
		the core through the `filter` region the mark reports. -->
	<span
		class="flex items-center rounded-full border px-2 py-0.5 text-xs"
		style:gap="calc(var(--mbv-cell-col) / 2)"
		style:border-color="var(--mbv-border)"
		title="Filter (/)"
		data-filter
	>
		<ListFilter size={12} color={roleVar('textMuted')} />
		<span style:color={summary.filter ? roleVar('textPrimary') : roleVar('textMuted')}
			>{summary.filter || 'Filter'}</span
		>
		{#if summary.filterActive}
			<span
				class="inline-block w-px"
				style:height="12px"
				style:background={roleVar('textPrimary')}
				data-caret
			></span>
		{/if}
	</span>
</div>
