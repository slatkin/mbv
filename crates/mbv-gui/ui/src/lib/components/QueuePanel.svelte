<script lang="ts">
	// The Queue panel (add-tauri-gui-client task 3.3, design D5): a fixed-row
	// column of the frame's viewport rows with no native DOM scroll — the
	// core owns scroll and sends the rows to paint. The component measures
	// the list and reports the whole rows it fits plus the painted row
	// regions through the shared layout reporter. The canvas footer line
	// below the list is `QueueFooter`'s.
	//
	// The paint mirrors the TUI's queue panel arrangement and media-list row
	// (crates/mbv-render/src/arrangements/queue.rs,
	// crates/mbv-render/src/components/media_list/row.rs, and
	// crates/mbv-render/src/components/queue.rs), resolved through the
	// frame's roles, with every offset a `calc()` over the cell unit
	// (`bridge/cells`): one row is `--mbv-cell-row`, one column
	// `--mbv-cell-col`. The cursor row and the marked rows paint the
	// selection bar, the active slot leads its row with a Lucide play
	// marker in the accent role, and a played row mutes its title.
	//
	// Two fills alias role values because the frame carries roles and the
	// chrome fills, not surface fills (design D7): the recessed box paints
	// the chrome's `queuePanel` pair — `Surface::QueuePanel`, focus-reactive
	// like the TUI's `queue_row_background(focused)` — and the selection bar
	// resolves `Surface::SelectedRow`, which the surface table fixes at the
	// Green slot — the value `--mbv-duration` carries.
	import { onMount } from 'svelte';
	import { Play } from '@lucide/svelte';
	import { CELL_ROW_PX } from '$lib/bridge/cells';
	import { reportLayout, retractLayout } from '$lib/bridge/layout';
	import { roleProperty } from '$lib/bridge/theme';
	import type { GuiQueue, GuiRole } from '$lib/bridge/types';
	import type { LayoutRegion } from '$lib/bridge/types';

	interface Props {
		queue: GuiQueue;
		/** Whether the queue surface holds interaction focus
		 * (`GuiFocus::surface`): the selection bar's rule, the TUI's
		 * `focused || multi_selected`. */
		focused: boolean;
		/** Whether the queue Panel holds the frame's appearance focus
		 * (`GuiFocus::panel`): the TUI's `panel_appearance_focus`, which
		 * picks the focused fills of the focus-reactive `QueuePanel` and
		 * `QueueColumn` surfaces. */
		panelFocused: boolean;
	}
	const { queue, focused, panelFocused }: Props = $props();

	/** The fixed row height in logical pixels: the cell unit's row
	 * (`bridge/cells`). Capacity is whole rows. */
	const ROW_HEIGHT = CELL_ROW_PX;

	/** The empty-queue text, as the TUI's local queue shows it. The frame
	 * carries no queue link, so the remote-queue text cannot be selected;
	 * the first slice attaches only to a local Owner (design D9). */
	const EMPTY_QUEUE_TEXT = '¯\\_(ツ)_/¯';

	const reportKey = Symbol('queue-panel');
	let panelEl: HTMLElement | undefined = $state();
	let listEl: HTMLElement | undefined = $state();

	/** Whether one item row paints the selection bar: a marked row bars
	 * always; the cursor row bars only while the panel holds focus, as the
	 * TUI's `focused || multi_selected` rule paints. */
	function bars(slot: number): boolean {
		return queue.marks.includes(slot) || (focused && queue.cursor === slot);
	}

	/** The `var(--mbv-<role>)` value a pill or row part paints in. */
	function roleVar(role: GuiRole): string {
		return `var(${roleProperty(role)})`;
	}

	/** The painted regions this frame's DOM reports (design D4): one per
	 * item row, resolved to its stable slot. */
	function currentRegions(): LayoutRegion[] {
		const regions: LayoutRegion[] = [];
		if (listEl) {
			for (const el of listEl.querySelectorAll<HTMLElement>('[data-slot]')) {
				const rect = el.getBoundingClientRect();
				regions.push({
					rect: {
						x: rect.x,
						y: rect.y,
						width: rect.width,
						height: rect.height,
					},
					target: { surface: 'queue', id: { slot: Number(el.dataset.slot) } },
				});
			}
		}
		return regions;
	}

	function report(): void {
		const wholeRows = Math.floor((listEl?.clientHeight ?? 0) / ROW_HEIGHT);
		reportLayout(reportKey, {
			lists: [{ list: 'queue', wholeRows }],
			regions: currentRegions(),
		});
	}

	// Each paint that moved a region reports (design D4): this effect runs
	// after the DOM update, and the ResizeObserver reports layout-driven
	// moves. The reporter dedupes an unchanged contribution, so a frame
	// that moves only the cursor sends nothing.
	$effect(() => {
		void queue;
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

<!-- The TUI's queue panel arrangement (the `queue_panel_geometry`
	structure), in cell units: the placement spans the full queue column,
	and the recessed queue-list box (`QueuePanel` fill, with its one-row
	top pad and bottom spacer) sits two columns inside it — `queue_list_box`
	insets from `placement.x + 2`, the same +2 columns the playback band's
	`queue_panel_inset` sits at, so every band edge aligns. The canvas
	footer line below the box is `QueueFooter`'s. -->
<div class="flex h-full flex-col" bind:this={panelEl}>
	<div
		class="flex min-h-0 flex-1 flex-col"
		style:margin-inline="calc(var(--mbv-cell-col) * 2)"
		style:background={panelFocused
			? 'var(--mbv-queuePanelFocused)'
			: 'var(--mbv-queuePanel)'}
	>
		<!-- The recessed box's one-row top pad: the list begins one pad row
			below the box's first row. -->
		<div style:height="var(--mbv-cell-row)"></div>
		<div class="min-h-0 flex-1 overflow-hidden" bind:this={listEl}>
		{#if queue.rows.length === 0}
			<div
				class="flex h-full items-center justify-center font-bold"
				style:color={roleVar('emptyQueueFg')}
			>
				{EMPTY_QUEUE_TEXT}
			</div>
		{:else}
				{#each queue.rows as row, i (row.row === 'item' ? row.slot : `structural-${i}`)}
					{#if row.row === 'item'}
						{@const barred = bars(row.slot)}
						<div
							class="flex items-center overflow-hidden whitespace-nowrap"
							style:height="var(--mbv-cell-row)"
							style:padding-inline="calc(var(--mbv-cell-col) * 2)"
							style:gap="var(--mbv-cell-col)"
							style:background={barred
								? 'var(--mbv-duration)'
								: row.striped
									? panelFocused
										? 'var(--mbv-queueColumnFocused)'
										: 'var(--mbv-queueColumn)'
									: undefined}
							data-slot={row.slot}
						>
							{#if queue.active === row.slot}
								<Play
									size={14}
									fill="currentColor"
									color={barred ? 'var(--mbv-selectedRowFg)' : 'var(--mbv-accent)'}
									class="shrink-0"
								/>
							{/if}
							<span
								class="truncate"
								style:color={barred
									? 'var(--mbv-selectedRowFg)'
									: row.secondary
										? 'var(--mbv-splitRowContextFg)'
										: row.state.state === 'played'
											? 'var(--mbv-textMuted)'
											: 'var(--mbv-textEmphasis)'}>{row.primary}</span
							>
							{#if row.secondary}
								<span
									class="truncate"
									style:color={barred
										? 'var(--mbv-selectedRowFg)'
										: row.state.state === 'played'
											? 'var(--mbv-textMuted)'
											: 'var(--mbv-splitRowTitleFg)'}>{row.secondary}</span
								>
							{/if}
							{#if row.state.state === 'active' || row.state.state === 'nowPlaying'}
								{#if row.state.progress !== null}
									<span
										style:color={barred
											? 'var(--mbv-selectedRowProgressFg)'
											: 'var(--mbv-progressPercent)'}
										>{row.state.progress}%</span
									>
								{/if}
							{/if}
							<span class="flex-1"></span>
							{#if row.trailing}
								<span
									class="overflow-hidden text-right whitespace-nowrap"
									style:min-width="calc(var(--mbv-cell-col) * 6)"
									style:max-width="calc(var(--mbv-cell-col) * 6)"
									style:color={barred
										? 'var(--mbv-selectedRowFg)'
										: 'var(--mbv-statusAvailable)'}>{row.trailing.gutter}</span
								>
							{/if}
							{#if row.kind === 'media' && row.duration}
								<span
									style:color={barred
										? 'var(--mbv-selectedRowFg)'
										: 'var(--mbv-duration)'}>{row.duration}</span
								>
							{/if}
						</div>
					{:else if row.row === 'heading'}
						<div
							class="flex items-center overflow-hidden uppercase"
							style:height="var(--mbv-cell-row)"
							style:padding-inline="calc(var(--mbv-cell-col) * 2)"
							style:color={roleVar('groupHeadingFg')}>{row.text}</div
						>
					{:else}
						<div style:height="var(--mbv-cell-row)"></div>
					{/if}
				{/each}
			{/if}
		</div>
		<!-- The recessed box's one-row bottom spacer: its own fill, so the
			rows never touch the box's edges. -->
		<div style:height="var(--mbv-cell-row)"></div>
	</div>
</div>
