<script lang="ts">
	// The Queue of the design canvas (add-tauri-gui-client row 6.5, design
	// D15): the canvas header (`QueueHeader`), fixed 30 px rows with no
	// native DOM scroll — the core owns scroll and sends the viewport rows
	// to paint — and the canvas footer line is `QueueFooter`'s. The
	// component measures the list and reports the whole 30 px rows it fits
	// plus the painted regions through the shared layout reporter.
	//
	// The rows paint the canvas's states from the frame's per-row
	// `GuiSemanticState`: a watched (`played`) row strikes its text
	// through, the now-playing row fills with the accent and leads with a
	// Lucide play marker, and a partial-progress row (and the playing row)
	// draws a thin progress bar under its title. The cursor row and the
	// marked rows keep the selection bar — the TUI's `focused ||
	// multi_selected` rule. The right column paints the value the
	// summary's mode names: "Length" or "Ends at".
	//
	// The row hover and the drag paint from the frame too — the window
	// holds neither (the `gui-client` spec): `queue.hover` names the row
	// that shows the grip and the actions button, and `queue.drag` carries
	// the drag ghost's content and the insertion line's stable-slot
	// anchor. The ghost floats at the line the drop would insert at; the
	// dragged row dims in place. Every pointer press, move, and release
	// forwards through the shared input installer — the core owns the
	// gesture, the drop index, and the move request.
	//
	// The regions report in paint order: the rows resolve to their stable
	// slots, then the hovered row's `rowGrip` and `rowActions` — pushed
	// after the row's `slot` region so the chrome wins where it paints
	// over the row (the hit map's last push wins). The open row menu's
	// entries (`QueueRowMenu`, presentation-only) report their
	// `rowMenuEntry` indices under a second report key this component
	// inserts when the menu opens — the same technique `TrackChips` uses
	// for its popover rows — so they land after every region the panel
	// reports and win the hit map where the menu paints over a row
	// beneath it.
	//
	// Two fills alias role values because the frame carries roles and the
	// chrome fills, not surface fills (design D7): the recessed box paints
	// the chrome's `queuePanel` pair — `Surface::QueuePanel`, focus-reactive
	// like the TUI's `queue_row_background(focused)` — and the selection bar
	// resolves `Surface::SelectedRow`, which the surface table fixes at the
	// Green slot — the value `--mbv-duration` carries.
	import { onMount } from 'svelte';
	import { Ellipsis, GripVertical, Play } from '@lucide/svelte';
	import { reportLayout, retractLayout } from '$lib/bridge/layout';
	import { roleProperty } from '$lib/bridge/theme';
	import type {
		GuiOverlayFrame,
		GuiQueue,
		GuiQueueRow,
		GuiQueueSummary,
		GuiRole,
		LayoutRegion,
		QueueTargetId,
	} from '$lib/bridge/types';
	import QueueHeader from './QueueHeader.svelte';
	import QueueRowMenu from './QueueRowMenu.svelte';

	type ItemRow = Extract<GuiQueueRow, { row: 'item' }>;

	interface Props {
		queue: GuiQueue;
		/** The queue header's facts (design D4): the remaining count, the
		 * finish clock, the right-column mode, and the filter. Optional so
		 * a bare render still compiles — `App.svelte` always carries it. */
		summary?: GuiQueueSummary;
		/** The open menu or popover (design D4): the row menu paints while
		 * `rowMenu` is the open overlay. `null` while none is open. */
		overlay?: GuiOverlayFrame | null;
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
	/** The summary a bare render falls back to: an empty queue's header
	 * facts. `App.svelte` always carries the frame's real section. */
	const EMPTY_SUMMARY: GuiQueueSummary = {
		remaining: 0,
		finishClock: null,
		rightColumn: 'length',
		filter: '',
		filterActive: false,
	};

	const {
		queue,
		summary = EMPTY_SUMMARY,
		overlay = null,
		focused,
		panelFocused,
	}: Props = $props();

	/** The canvas's fixed queue row height in logical pixels (the spec's
	 * 30 px rows). Capacity is whole rows. */
	const ROW_HEIGHT = 30;

	/** The empty-queue text, as the TUI's local queue shows it. The frame
	 * carries no queue link, so the remote-queue text cannot be selected;
	 * the first slice attaches only to a local Owner (design D9). */
	const EMPTY_QUEUE_TEXT = '¯\\_(ツ)_/¯';

	const reportKey = Symbol('queue-panel');
	const menuKey = Symbol('queue-row-menu');
	let panelEl: HTMLElement | undefined = $state();
	let listEl: HTMLElement | undefined = $state();

	/** The open row menu, or `null` while another overlay (or none) is up. */
	const menu = $derived(overlay?.kind === 'rowMenu' ? overlay : null);

	/** The drag's insertion line resolved against the painted rows: the
	 * anchor slot's row index, or `null` while no drag runs or its anchor
	 * is not a painted row. */
	const insertion = $derived.by(() => {
		const drag = queue.drag;
		if (!drag) {
			return null;
		}
		const index = queue.rows.findIndex(
			(row) => row.row === 'item' && row.slot === drag.insertion.slot,
		);
		return index < 0 ? null : { at: drag.insertion.at, slot: drag.insertion.slot, index };
	});

	/** The drag ghost's content, `null` while no drag runs or the ghost
	 * row is structural (the core builds it from an item row). */
	const ghost = $derived(
		queue.drag && queue.drag.ghost.row === 'item' ? queue.drag.ghost : null,
	);

	/** The ghost's top edge in list pixels: the row top the drop would
	 * insert at — the anchor row's top edge for `before`, its bottom edge
	 * for `after`. */
	const ghostTop = $derived(
		insertion
			? insertion.at === 'before'
				? insertion.index * ROW_HEIGHT
				: (insertion.index + 1) * ROW_HEIGHT
			: 0,
	);

	/** The row-menu's top edge: the header row's height plus the bottom
	 * edge of the row the menu opens on — the hovered row (the actions
	 * button's press) or, from the keyboard, the cursor row. The overlay
	 * carries no anchor slot, so the two frame facts are the anchor. */
	const menuTop = $derived.by(() => {
		const anchor = queue.hover ?? queue.cursor;
		const index = queue.rows.findIndex(
			(row) => row.row === 'item' && row.slot === anchor,
		);
		const below = (index >= 0 ? index + 1 : 0) * ROW_HEIGHT;
		return `calc(var(--mbv-cell-row) + ${below}px)`;
	});

	/** Whether one item row paints the selection bar: a marked row bars
	 * always; the cursor row bars only while the panel holds focus, as the
	 * TUI's `focused || multi_selected` rule paints. */
	function bars(slot: number): boolean {
		return queue.marks.includes(slot) || (focused && queue.cursor === slot);
	}

	/** The row's progress percentage, where its state carries one. */
	function rowProgress(row: ItemRow): number | null {
		return row.state.state === 'active' || row.state.state === 'nowPlaying'
			? row.state.progress
			: null;
	}

	/** The row's right-column text under the summary's mode: "Length"
	 * paints the media row's duration; "Ends at" paints the finish clock
	 * the frame carries for the slot. */
	function rightValue(row: ItemRow): string | null {
		if (summary.rightColumn === 'endsAt') {
			return row.endsAt;
		}
		return row.kind === 'media' ? row.duration : null;
	}

	/** The row's painted background: the selection bar wins (it is the
	 * interaction state), then the canvas's filled active row, then the
	 * zebra stripe. */
	function rowBackground(row: ItemRow): string | undefined {
		if (bars(row.slot)) {
			return 'var(--mbv-duration)';
		}
		if (row.state.state === 'nowPlaying') {
			return 'var(--mbv-accent)';
		}
		if (row.striped) {
			return panelFocused ? 'var(--mbv-queueColumnFocused)' : 'var(--mbv-queueColumn)';
		}
		return undefined;
	}

	/** The colour set one row's cells paint in. The barred row reads from
	 * the selection roles, the filled active row from the on-accent role,
	 * the watched row muted, and the plain row from the media-list roles. */
	interface RowTone {
		primary: string;
		secondary: string;
		meta: string;
		right: string;
		progress: string;
	}

	function tone(row: ItemRow, barred: boolean): RowTone {
		if (barred) {
			return {
				primary: 'var(--mbv-selectedRowFg)',
				secondary: 'var(--mbv-selectedRowFg)',
				meta: 'var(--mbv-selectedRowFg)',
				right: 'var(--mbv-selectedRowFg)',
				progress: 'var(--mbv-selectedRowProgressFg)',
			};
		}
		if (row.state.state === 'nowPlaying') {
			return {
				primary: 'var(--mbv-textOnAccent)',
				secondary: 'var(--mbv-textOnAccent)',
				meta: 'var(--mbv-textOnAccent)',
				right: 'var(--mbv-textOnAccent)',
				progress: 'var(--mbv-textOnAccent)',
			};
		}
		if (row.state.state === 'played') {
			return {
				primary: 'var(--mbv-textMuted)',
				secondary: 'var(--mbv-textMuted)',
				meta: 'var(--mbv-textMuted)',
				right: 'var(--mbv-textMuted)',
				progress: 'var(--mbv-accent)',
			};
		}
		return {
			primary: row.secondary ? 'var(--mbv-splitRowContextFg)' : 'var(--mbv-textEmphasis)',
			secondary: 'var(--mbv-splitRowTitleFg)',
			meta: 'var(--mbv-statusAvailable)',
			right: 'var(--mbv-duration)',
			progress: 'var(--mbv-accent)',
		};
	}

	/** The `var(--mbv-<role>)` value a part paints in. */
	function roleVar(role: GuiRole): string {
		return `var(${roleProperty(role)})`;
	}

	/** One element's painted region, resolved to its Queue target id. */
	function region(el: HTMLElement, id: QueueTargetId): LayoutRegion {
		const rect = el.getBoundingClientRect();
		return {
			rect: { x: rect.x, y: rect.y, width: rect.width, height: rect.height },
			target: { surface: 'queue', id },
		};
	}

	/** The open menu's painted entries, each resolved to its index in the
	 * overlay's `entries` list. They report under `menuKey` — inserted in
	 * `report` after this component's own key, so the entries win over
	 * the rows beneath the menu even on a first frame that already
	 * carries the open menu. */
	function menuRegions(): LayoutRegion[] {
		if (!panelEl) {
			return [];
		}
		return [...panelEl.querySelectorAll<HTMLElement>('[data-menu-entry]')].map((el) =>
			region(el, { rowMenuEntry: Number(el.dataset.menuEntry) }),
		);
	}

	/** The painted regions this frame's DOM reports (design D4): the item
	 * rows resolve to their stable slots, then the hovered row's grip and
	 * actions button — pushed after the row regions so the chrome wins
	 * where it paints over its row. */
	function currentRegions(): LayoutRegion[] {
		const regions: LayoutRegion[] = [];
		if (!listEl) {
			return regions;
		}
		for (const el of listEl.querySelectorAll<HTMLElement>('[data-slot]')) {
			regions.push(region(el, { slot: Number(el.dataset.slot) }));
		}
		for (const el of listEl.querySelectorAll<HTMLElement>('[data-grip]')) {
			regions.push(region(el, { rowGrip: Number(el.dataset.grip) }));
		}
		for (const el of listEl.querySelectorAll<HTMLElement>('[data-actions]')) {
			regions.push(region(el, { rowActions: Number(el.dataset.actions) }));
		}
		return regions;
	}

	function report(): void {
		const wholeRows = Math.floor((listEl?.clientHeight ?? 0) / ROW_HEIGHT);
		reportLayout(reportKey, {
			lists: [{ list: 'queue', wholeRows }],
			regions: currentRegions(),
		});
		if (menu) {
			reportLayout(menuKey, { lists: [], regions: menuRegions() });
		}
	}

	// Each paint that moved a region reports (design D4): this effect runs
	// after the DOM update, and the ResizeObserver reports layout-driven
	// moves. Reading the overlay registers the menu's entries, which
	// appear and disappear with that frame section; the menu's key
	// retracts while none is open. The reporter dedupes an unchanged
	// contribution, so a frame that moves only the cursor sends nothing.
	$effect(() => {
		void queue;
		void summary;
		void overlay;
		report();
		if (!menu) {
			retractLayout(menuKey);
		}
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
			retractLayout(menuKey);
		};
	});
</script>

{#snippet rowCells(row: ItemRow, cellTone: RowTone)}
	<!-- The item row's shared cells: the playing marker, the title pair,
		the thin progress bar under the title, the date gutter, and the
		right column. The list rows and the drag ghost paint the same cells
		— the ghost's tone just reads the unbarred roles. -->
	{#if queue.active === row.slot}
		<Play size={14} fill="currentColor" color={cellTone.primary} class="shrink-0" />
	{/if}
	<span
		class="truncate"
		class:line-through={row.state.state === 'played'}
		style:color={cellTone.primary}>{row.primary}</span
	>
	{#if row.secondary}
		<span
			class="truncate"
			class:line-through={row.state.state === 'played'}
			style:color={cellTone.secondary}>{row.secondary}</span
		>
	{/if}
	{@const progress = rowProgress(row)}
	{#if progress !== null}
		<span
			class="pointer-events-none absolute right-0 bottom-0 left-0"
			style:height="2px"
			style:padding-inline="calc(var(--mbv-cell-col) * 2)"
			data-progress
		>
			<span
				class="block h-full rounded-full"
				style:width="{progress}%"
				style:background={cellTone.progress}
			></span>
		</span>
	{/if}
	<span class="flex-1"></span>
	{#if row.trailing}
		<span
			class="overflow-hidden text-right whitespace-nowrap"
			style:min-width="calc(var(--mbv-cell-col) * 6)"
			style:max-width="calc(var(--mbv-cell-col) * 6)"
			style:color={cellTone.meta}>{row.trailing.gutter}</span
		>
	{/if}
	{@const right = rightValue(row)}
	{#if right}
		<span class="font-mono text-xs tabular-nums" style:color={cellTone.right}>{right}</span>
	{/if}
{/snippet}

<!-- The TUI's queue panel arrangement (the `queue_panel_geometry`
	structure), in cell units, with the canvas's queue header inside the
	recessed box: the placement spans the full queue column, and the box
	(`QueuePanel` fill, with the header row at its top and its one-row
	bottom spacer) sits two columns inside it — `queue_list_box` insets
	from `placement.x + 2`, the same +2 columns the playback band's
	`queue_panel_inset` sits at, so every band edge aligns. The canvas
	footer line below the box is `QueueFooter`'s. -->
<div class="flex h-full flex-col" bind:this={panelEl}>
	<div
		class="relative flex min-h-0 flex-1 flex-col"
		style:margin-inline="calc(var(--mbv-cell-col) * 2)"
		style:background={panelFocused
			? 'var(--mbv-queuePanelFocused)'
			: 'var(--mbv-queuePanel)'}
	>
		<QueueHeader {summary} />
		<div class="relative min-h-0 flex-1 overflow-hidden" bind:this={listEl}>
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
							class="relative flex items-center overflow-hidden whitespace-nowrap"
							class:opacity-40={queue.drag?.slot === row.slot}
							style:height="{ROW_HEIGHT}px"
							style:padding-inline="calc(var(--mbv-cell-col) * 2)"
							style:gap="var(--mbv-cell-col)"
							style:background={rowBackground(row)}
							data-slot={row.slot}
						>
							{#if insertion?.slot === row.slot}
								<!-- The drag's insertion line, anchored on this row's
									top edge (`before`) or bottom edge (`after`). -->
								<span
									class="pointer-events-none absolute right-0 left-0 z-10"
									style:height="2px"
									style:top={insertion.at === 'before' ? '0' : undefined}
									style:bottom={insertion.at === 'after' ? '0' : undefined}
									style:background="var(--mbv-accent)"
									data-insertion={insertion.at}
								></span>
							{/if}
							{@render rowCells(row, tone(row, barred))}
							{#if queue.hover === row.slot}
								<!-- The hover chrome: the drag grip at the leading edge
									and the actions button at the trailing edge. Each is its
									own region, pushed after the row's `slot` region so the
									press resolves to the chrome. -->
								<span
									class="absolute top-1/2 -translate-y-1/2"
									style:left="2px"
									data-grip={row.slot}
								>
									<GripVertical size={14} color={roleVar('textMuted')} />
								</span>
								<button
									type="button"
									class="absolute top-1/2 -translate-y-1/2 rounded-sm p-0.5"
									style:right="2px"
									style:background="var(--mbv-secondary)"
									data-actions={row.slot}
									aria-label="Row actions"
								>
									<Ellipsis size={14} color={roleVar('textMuted')} />
								</button>
							{/if}
						</div>
					{:else if row.row === 'heading'}
						<div
							class="flex items-center overflow-hidden uppercase"
							style:height="{ROW_HEIGHT}px"
							style:padding-inline="calc(var(--mbv-cell-col) * 2)"
							style:color={roleVar('groupHeadingFg')}>{row.text}</div
						>
					{:else}
						<div style:height="{ROW_HEIGHT}px"></div>
					{/if}
				{/each}
				{#if insertion && ghost}
					<!-- The drag ghost: the dragged row's own cells in a
						floating card at the insertion line, `pointer-events-none`
						so the pointer keeps resolving the regions beneath it. The
						core owns the drag; the release's move request is its. -->
					<div
						class="pointer-events-none absolute right-0 left-0 z-20"
						style:top="{ghostTop}px"
						data-ghost
					>
						<div
							class="relative flex items-center overflow-hidden rounded-md border whitespace-nowrap shadow-lg"
							style:height="{ROW_HEIGHT}px"
							style:padding-inline="calc(var(--mbv-cell-col) * 2)"
							style:gap="var(--mbv-cell-col)"
							style:background="var(--mbv-popover)"
							style:border-color="var(--mbv-accent)"
						>
							{@render rowCells(ghost, tone(ghost, false))}
						</div>
					</div>
				{/if}
			{/if}
		</div>
		<!-- The recessed box's one-row bottom spacer: its own fill, so the
			rows never touch the box's edges. -->
		<div style:height="var(--mbv-cell-row)"></div>
		{#if menu}
			<!-- The open row menu (design D15): the frame's entries and key
				hints, dropped under the row the menu opened on. Its entries
				report their own regions under the component's key, last over
				the rows beneath (the component's header comment names why). -->
			<QueueRowMenu cursor={menu.cursor} entries={menu.entries} top={menuTop} />
		{/if}
	</div>
</div>
