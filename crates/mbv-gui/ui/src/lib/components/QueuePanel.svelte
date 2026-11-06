<script lang="ts">
	// The Queue panel (add-tauri-gui-client task 3.3, design D5): a fixed-row
	// column of the frame's viewport rows with no native DOM scroll — the
	// core owns scroll and sends the rows to paint. The component measures
	// the list and reports the whole rows it fits plus the painted row and
	// playlist-pill regions through the shared layout reporter.
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
	// Three fills alias role values because the frame carries roles and the
	// chrome fills, not surface fills (design D7): the recessed box paints
	// the chrome's `queuePanel` pair — `Surface::QueuePanel`, focus-reactive
	// like the TUI's `queue_row_background(focused)` — the selection bar
	// resolves `Surface::SelectedRow`, which the surface table fixes at the
	// Green slot — the value `--mbv-duration` carries; and the footer pills
	// share `Surface::StatusBarPill`, the BgDim slot that
	// `--mbv-pillSelectedFg` carries.
	import { onMount } from 'svelte';
	import {
		Cable,
		ListMusic,
		Play,
		SaveCheck,
		SaveOff,
	} from '@lucide/svelte';
	import { CELL_ROW_PX } from '$lib/bridge/cells';
	import { reportLayout, retractLayout } from '$lib/bridge/layout';
	import { roleProperty } from '$lib/bridge/theme';
	import type {
		GuiQueue,
		GuiQueueFooter,
		GuiRole,
	} from '$lib/bridge/types';
	import type { LayoutRegion } from '$lib/bridge/types';

	interface Props {
		queue: GuiQueue;
		footer: GuiQueueFooter;
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
	const { queue, footer, focused, panelFocused }: Props = $props();

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
	let pillEl: HTMLElement | undefined = $state();

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
	 * item row, resolved to its stable slot, plus the playlist pill. */
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
		if (pillEl) {
			const rect = pillEl.getBoundingClientRect();
			regions.push({
				rect: { x: rect.x, y: rect.y, width: rect.width, height: rect.height },
				target: { surface: 'queue', id: 'playlistPill' },
			});
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
		void footer;
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
	and both the recessed queue-list box (`QueuePanel` fill, with its
	one-row top pad and bottom spacer) and the footer row below it sit two
	columns inside it — `queue_list_box` and `queue_footer_row` both inset
	from `placement.x + 2`, the same +2 columns the playback band's
	`queue_panel_inset` sits at, so every band edge aligns. The footer
	band's gap row and the placement's bottom padding row keep the queue
	column's backdrop. -->
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
							style:background={barred ? 'var(--mbv-duration)' : undefined}
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
	<!-- The gap row between the recessed box and the footer band, and the
		placement's bottom padding row below the footer: both keep the queue
		column's backdrop the placement sits on. -->
	<div style:height="var(--mbv-cell-row)"></div>
	<div
		class="flex items-center justify-between"
		style:height="var(--mbv-cell-row)"
		style:margin-inline="calc(var(--mbv-cell-col) * 2)"
		style:background="var(--mbv-queueFooterBand)"
	>
		<!-- The footer's status pills (design D8): the GUI maps each pill's
			slot to its Lucide icon where the TUI paints its Nerd Font glyph —
			the playlist source's `playlist-play`, the owner indicator's
			`connection`, the autosave `content-save-check`, and the dirty
			`content-save-alert`. The icon and the glyph-free label paint in
			the pill's role colour, one column apart. -->
		<!-- The left group as `render_queue_status` paints it: a one-column
			pad, then the autosave pill (when present), then the playlist pill,
			then a one-column pad — every pad cell carries the status-pill
			fill, so the group paints on `--mbv-pillSelectedFg` with its pads
			inside it. The playlist pill's icon-plus-name span is the pill's
			hit region (icon and name, without the outer pads). -->
		<div
			class="flex items-center"
			style:background="var(--mbv-pillSelectedFg)"
			style:padding-inline="var(--mbv-cell-col)"
			style:gap="var(--mbv-cell-col)"
		>
			{#if footer.autosave}
				<SaveCheck size={14} color={roleVar(footer.autosave.role)} class="shrink-0" />
			{/if}
			<span bind:this={pillEl} class="flex items-center" style:gap="var(--mbv-cell-col)">
				<ListMusic
					size={14}
					color={roleVar(footer.playlist.role)}
					class="shrink-0"
				/>
				{#if footer.playlist.text}
					<span style:color={roleVar(footer.playlist.role)}>{footer.playlist.text}</span>
				{/if}
			</span>
		</div>
		<!-- The right group: the dirty pill (when present) wrapped in
			one-column pads on the pill fill, then the owner indicator flush at
			the row's right edge — `render_queue_status` right-anchors the
			group and the owner span carries no outer pad. -->
		<div class="flex items-center">
			{#if footer.dirty}
				<span
					class="flex items-center"
					style:background="var(--mbv-pillSelectedFg)"
					style:padding-inline="var(--mbv-cell-col)"
					style:gap="var(--mbv-cell-col)"
				>
					<SaveOff
						size={14}
						color={roleVar(footer.dirty.role)}
						class="shrink-0"
					/>
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
</div>
