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
	// through, the now-playing row shows a play glyph beside its title (the
	// highlight is for focus only, so the row keeps its own fill), and a partial-progress row (and the playing row) draws a
	// thin progress track on the title's line. Every row leads with its
	// padded queue ordinal and carries one title line and the
	// right-column value — no second line and no date gutter, as the
	// canvas's row draws. The cursor row and the marked rows paint the
	// canvas's picked fill (the TUI's `focused || multi_selected` rule),
	// and the active row's fill wins over it. The right column paints the
	// value the summary's mode names: "Length" or "Ends at".
	//
	// The row hover and the drag paint from the frame too — the window
	// holds neither (the `gui-client` spec): `queue.hover` names the row
	// that shows the six-dot grip in its ordinal's slot and the canvas's
	// 30 px actions button in its right value's slot — the two chrome
	// pieces stay siblings of the row's one button, over slots the row
	// reserves, so no control nests in it — and `queue.drag` carries the
	// drag ghost's content and the insertion line's stable-slot anchor.
	// The ghost floats at the line the drop would insert at, with no
	// ordinal and the dragged row's percent when it has one; the dragged
	// row no longer dims in place (the canvas keeps it lit). Every pointer
	// press, move, and release forwards through the shared input installer
	// — the core owns the gesture, the drop index, and the move request.
	//
	// The regions report in paint order: the header's Length / Ends-at
	// switch and filter resolve to their `rightColumn` and `filter`
	// targets, the rows resolve to their stable slots, then the hovered
	// row's `rowGrip` and `rowActions` — pushed after the row's `slot`
	// region so the chrome wins where it paints over the row (the hit
	// map's last push wins). The open row menu's
	// entries (`QueueRowMenu`, presentation-only) report their
	// `rowMenuEntry` indices under a second report key this component
	// inserts when the menu opens — the same technique `TrackChips` uses
	// for its popover rows — so they land after every region the panel
	// reports and win the hit map where the menu paints over a row
	// beneath it.
	//
	// The row paint is the canvas's (U4.2): its hexes come through the
	// queue-local `--canvas-*` tokens, so no theme slot or TUI palette value
	// moves (U13). Two fills still alias role values because the frame
	// carries roles and the chrome fills, not surface fills (design D7): the
	// recessed box paints the chrome's `queuePanel` pair — `Surface::QueuePanel`,
	// focus-reactive like the TUI's `queue_row_background(focused)` — and the
	// zebra stripe paints the `queueColumn` pair.
	import { onMount } from 'svelte';
	import { Ellipsis, GripVertical, Play } from '@lucide/svelte';
	import { reportLayout, retractLayout } from '$lib/bridge/layout';
	import { canvasVar, roleVar } from '$lib/bridge/theme';
	import type {
		GuiOverlayFrame,
		GuiQueue,
		GuiQueueRow,
		GuiQueueSummary,
		LayoutRegion,
		QueueTargetId,
	} from '$lib/bridge/types';
	import QueueHeader from './QueueHeader.svelte';
	import QueueRowMenu from './QueueRowMenu.svelte';
	import TypedText from './TypedText.svelte';

	type ItemRow = Extract<GuiQueueRow, { row: 'item' }>;

	/** Which cells a row paints: the list row, the hovered row (the chrome
	 * replaces the ordinal and the right value), or the drag ghost (the
	 * title and the percent only). */
	type RowChrome = 'row' | 'hover' | 'ghost';

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
		nextTitle: null,
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
	 * the first slice attaches only to a local Owner (design D9). The
	 * canvas draws no empty-queue state (design D15), so the placeholder
	 * paints in the canvas's type — a display-sized medium weight in the
	 * `emptyQueueFg` role — as a missing visual does. */
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
	 * edge of the row the menu opened on — the overlay's `anchorSlot`,
	 * which the core names (hover and cursor can move while the menu is
	 * up) — less the canvas's 6 px overlap into that row (`Menus.dc.html`
	 * sits the 230 px menu over its anchor row's bottom edge, not under it).
	 * A `0` fallback holds the menu at the header while the anchor is not a
	 * painted row. */
	const menuTop = $derived.by(() => {
		const index = queue.rows.findIndex(
			(row) => row.row === 'item' && row.slot === menu?.anchorSlot,
		);
		const below = index >= 0 ? (index + 1) * ROW_HEIGHT - 6 : 0;
		return `calc(var(--canvas-queue-header-height) + ${below}px)`;
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


	/** The row's painted background: the hovered or picked row's fill, then the
	 * zebra stripe. The playing row keeps its own fill, because a highlight marks
	 * focus only. */
	function rowBackground(row: ItemRow): string | undefined {
		if (queue.hover === row.slot || bars(row.slot)) {
			return canvasVar('selected');
		}
		if (row.striped) {
			return panelFocused ? 'var(--mbv-queueColumnFocused)' : 'var(--mbv-queueColumn)';
		}
		return undefined;
	}

	/** The colour set one row's cells paint in (the canvas's row states): the
	 * filled active row reads the dark ink, the hovered row the canvas's
	 * hover text, the watched row the muted text with a strike, and every
	 * other row the canvas's row text. */
	interface RowTone {
		title: string;
		weight: number;
		strike: string;
		ordinal: string;
		track: string;
		fill: string;
	}

	function tone(row: ItemRow): RowTone {
		if (row.state.state === 'nowPlaying') {
			return {
				title: canvasVar('heading'),
				weight: 600,
				strike: canvasVar('strike'),
				ordinal: canvasVar('accent'),
				track: canvasVar('track-current'),
				fill: canvasVar('accent'),
			};
		}
		const watched = row.state.state === 'played';
		const hovered = queue.hover === row.slot;
		return {
			title: hovered ? canvasVar('heading') : watched ? canvasVar('muted') : canvasVar('text'),
			weight: 400,
			strike: canvasVar('strike'),
			ordinal: hovered ? canvasVar('heading') : canvasVar('ordinal'),
			track: canvasVar('track'),
			fill: canvasVar('partial'),
		};
	}

	/** One element's painted region, resolved to its Queue target id. Its
	 * elements come from a live `querySelectorAll` over attached DOM, so the
	 * rect is always measurable — the nullable shape the other reporters use
	 * has no counterpart here. */
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

	/** The painted regions this frame's DOM reports (design D4): the
	 * header's switch and filter resolve to their own targets, the item
	 * rows resolve to their stable slots, then the hovered row's grip and
	 * actions button — pushed after the row regions so the chrome wins
	 * where it paints over its row. */
	function currentRegions(): LayoutRegion[] {
		const regions: LayoutRegion[] = [];
		if (!panelEl) {
			return regions;
		}
		for (const el of panelEl.querySelectorAll<HTMLElement>(
			'[data-right-column-segment]',
		)) {
			regions.push(region(el, 'rightColumn'));
		}
		for (const el of panelEl.querySelectorAll<HTMLElement>('[data-filter]')) {
			regions.push(region(el, 'filter'));
		}
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

{#snippet rowCells(row: ItemRow, cellTone: RowTone, chrome: RowChrome)}
	<!-- The item row's canvas cells: the padded queue ordinal, the one title
		line over its progress track, and the right-column value. The hovered
		row reserves the leading and trailing slots its sibling chrome covers,
		and the drag ghost paints the title and the percent only. -->
	{@const progress = rowProgress(row)}
	{@const right = chrome === 'row' ? rightValue(row) : null}
	{#if chrome === 'row'}
		<span
			class="shrink-0 font-mono tabular-nums"
			style:width="18px"
			style:font-size="11px"
			style:color={cellTone.ordinal}
			data-ordinal>{String(row.ordinal).padStart(2, '0')}</span
		>
	{:else if chrome === 'hover'}
		<!-- The grip's slot: the six-dot grip replaces the ordinal. -->
		<span class="shrink-0" style:width="14px" aria-hidden="true"></span>
	{/if}
	<span class="flex min-w-0 flex-1 flex-col justify-center" style:gap="3px">
		<span class="flex min-w-0 items-center" style:gap="6px">
			{#if row.state.state === 'nowPlaying' && chrome === 'row'}
				<Play size={12} fill={canvasVar('accent')} color={canvasVar('accent')} />
			{/if}
			<span
				class="min-w-0 truncate"
				style:font-size="13.5px"
				style:font-weight={cellTone.weight}
				style:text-decoration-color={cellTone.strike}
				class:line-through={row.state.state === 'played'}
				style:color={chrome === 'ghost' ? canvasVar('heading') : cellTone.title}
				>{row.primary}</span
			>
		</span>
		{#if progress !== null && chrome !== 'ghost'}
			<span
				class="block shrink-0"
				style:height="2px"
				style:border-radius="1px"
				style:background={cellTone.track}
				data-progress
			>
				<span
					class="block"
					style:height="2px"
					style:border-radius="1px"
					style:width="{Math.max(progress, 3)}%"
					style:background={cellTone.fill}
				></span>
			</span>
		{/if}
	</span>
	{#if right}
		<!-- The right column repeats a time down the list, so it paints the time
			token at the repeat opacity; the current row keeps its own ink at full
			strength (design D14). -->
		{#if row.state.state === 'nowPlaying'}
			<span
				class="shrink-0 font-mono text-xs tabular-nums"
				style:color={canvasVar('ink')} data-right-value>{right}</span
			>
		{:else}
			<span class="shrink-0 font-mono text-xs tabular-nums" data-right-value>
				<TypedText segments={[{ text: right, kind: 'time' }]} repeat />
			</span>
		{/if}
	{:else if chrome === 'ghost' && progress !== null}
		<span
			class="shrink-0 font-mono tabular-nums"
			style:font-size="12px"
			style:color={canvasVar('partial')}
			data-ghost-percent>{progress}%</span
		>
	{:else if chrome === 'hover'}
		<!-- The actions button's slot: it replaces the right-column value. -->
		<span class="shrink-0" style:width="30px" aria-hidden="true"></span>
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
		<div
			class="relative min-h-0 flex-1 overflow-hidden"
			style:padding-inline="8px"
			data-queue-list
			bind:this={listEl}
		>
			{#if queue.rows.length === 0}
				<!-- The empty-queue placeholder (design D15's missing-visual
					rule): the TUI's shrug set as the canvas's display type —
					display-sized, medium weight, wide tracking — in the
					`emptyQueueFg` role, centred in the list box. -->
				<div class="flex h-full items-center justify-center">
					<span
						class="text-2xl font-medium tracking-widest"
						style:color={roleVar('emptyQueueFg')}>{EMPTY_QUEUE_TEXT}</span
					>
				</div>
			{:else}
				{#each queue.rows as row, i (row.row === 'item' ? row.slot : `structural-${i}`)}
					{#if row.row === 'item'}
						<!-- The row is one button; its hover chrome and the drag's
							insertion line are siblings inside the positioning box, so
							no control nests in the row's own button. -->
						<div class="relative" data-row={row.slot}>
							<button
								type="button"
								class="flex w-full items-center overflow-hidden text-left whitespace-nowrap"
								style:height="{ROW_HEIGHT}px"
								style:padding-inline={queue.hover === row.slot ? '4px 2px' : '8px'}
								style:gap="10px"
								style:border-radius="4px"
								style:background={rowBackground(row)}
								data-slot={row.slot}
							>
								{@render rowCells(
									row,
									tone(row),
									queue.hover === row.slot ? 'hover' : 'row',
								)}
							</button>
							{#if insertion?.slot === row.slot}
								<!-- The drag's insertion line, anchored on this row's
									top edge (`before`) or bottom edge (`after`), inset
									from the row's edges as the canvas draws it. -->
								<span
									class="pointer-events-none absolute right-0 left-0 z-10"
									style:height="2px"
									style:top={insertion.at === 'before' ? '0' : undefined}
									style:bottom={insertion.at === 'after' ? '0' : undefined}
									style:margin="0 8px"
									style:border-radius="1px"
									style:background={canvasVar('insertion')}
									data-insertion={insertion.at}
								></span>
							{/if}
							{#if queue.hover === row.slot}
								<!-- The hover chrome (the Menus artboard): the six-dot grip
									over the leading slot the row reserves in place of its
									ordinal, and the 30 px actions button over the trailing
									slot in place of its right value. Each is its own
									region, pushed after the row's `slot` region so the
									press resolves to the chrome. They stay siblings of the
									row's button, so no control nests in it. -->
								<span
									class="absolute top-1/2 flex -translate-y-1/2 items-center"
									style:left="4px"
									style:width="14px"
									data-grip={row.slot}
								>
									<GripVertical size={14} color={canvasVar('muted')} />
								</span>
								<button
									type="button"
									class="absolute flex items-center justify-center"
									style:top="0"
									style:right="2px"
									style:width="30px"
									style:height="30px"
									style:border-radius="4px"
									style:background={canvasVar('action')}
									data-actions={row.slot}
									aria-label="Row actions"
								>
									<Ellipsis
										size={14}
										fill={canvasVar('heading')}
										color={canvasVar('heading')}
									/>
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
			{/if}
		</div>
		{#if insertion && ghost}
			<!-- The drag ghost (the Menus artboard): the dragged row's title
				and its percent in a floating card at the insertion line,
				`pointer-events-none` so the pointer keeps resolving the regions
				beneath it. It sits outside the clipped list so its canvas
				overhang past the row's right edge paints, keeps the row's own
				insets so its cells line up with the row beneath, and paints no
				ordinal. The core owns the drag; the release's move request is
				its. -->
			<div
				class="pointer-events-none absolute z-20"
				style:top="calc(var(--canvas-queue-header-height) + {ghostTop}px)"
				style:left="28px"
				style:right="-6px"
				data-ghost
			>
				<div
					class="flex items-center overflow-hidden whitespace-nowrap"
					style:height="{ROW_HEIGHT}px"
					style:padding-inline="8px"
					style:gap="10px"
					style:background={canvasVar('ghost')}
					style:border-radius="4px"
					style:box-shadow="var(--canvas-ghost-shadow)"
					style:transform="rotate(-1deg)"
				>
					{@render rowCells(ghost, tone(ghost), 'ghost')}
				</div>
			</div>
		{/if}
		<!-- The recessed box's one-row bottom spacer: its own fill, so the
			rows never touch the box's edges. -->
		<div style:height="var(--mbv-cell-row)"></div>
		{#if menu}
			<!-- The open row menu (design D15): the frame's entries and key
				hints, dropped over the bottom edge of the row the menu opened
				on, as the canvas places it. Its entries report their own regions
				under the component's key, last over the rows beneath (the
				component's header comment names why). -->
			<QueueRowMenu cursor={menu.cursor} entries={menu.entries} top={menuTop} />
		{/if}
	</div>
</div>
