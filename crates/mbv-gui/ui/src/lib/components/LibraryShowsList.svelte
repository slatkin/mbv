<script lang="ts">
	// The Shows list (add-gui-library-panel design D12, row 7.1): the All series
	// mode's series rows under their letter headings, with the A–Z letter rail
	// beside them, and the Latest and Upcoming modes' flat rows with their date
	// gutter and no rail. The rows paint at the fixed heights of
	// `$lib/library/shows-list` over native scroll, and only the rows in view
	// paint, so a long series list stays fast.
	//
	// The cursor, the scroll offset, the viewport, and the filter text are the
	// window content's own in `$lib/library/shows` (design D1); this component
	// paints that state and reports the two facts only the DOM holds. A press
	// selects the row the Hero shows, a double-click on a flat row plays it,
	// and the keys' Enter selects a series and plays a flat row. The `/`
	// filter field narrows the rows to the titles that contain its text, the
	// rail's keyboard path in the All series mode (design D7).
	import { onMount } from 'svelte';
	import type { GuiShowsFrame } from '$lib/bridge/types';
	import {
		activeShowsMode,
		buildShowsRows,
		showsLetterRail,
		showsListWindow,
		showsRowLetter,
		showsRowsHeight,
		showsSeriesMode,
	} from '$lib/library/shows-list';
	import type { ShowsListRow } from '$lib/library/shows-list';
	import {
		acceptShowsFilter,
		jumpShowsToLetter,
		measureShowsList,
		playShowsRow,
		pressShowsRow,
		scrollShowsList,
		setShowsFilter,
		showsUi,
	} from '$lib/library/shows';
	import { resetShowsEpisodeCursor } from '$lib/library/shows-workspace';
	import LibraryFilterField from './LibraryFilterField.svelte';
	import LibraryLetterRail from './LibraryLetterRail.svelte';

	interface Props {
		/** The carried Shows content (design D12): the active mode's rows. */
		shows: GuiShowsFrame;
	}
	const { shows }: Props = $props();

	let listEl: HTMLElement | undefined = $state();

	/** The active view mode (design D12): the pill the core marked. */
	const mode = $derived(activeShowsMode(shows));

	/** The painted rows (design D12): the series rows with their headings, or
	 * the flat rows with no heading, narrowed by the filter's text. */
	const rows = $derived(buildShowsRows(shows.rows, mode, $showsUi.filter));

	/** The rows in view: only these paint. */
	const view = $derived(showsListWindow(rows, $showsUi.scrollTop, $showsUi.viewportHeight));
	const painted = $derived(rows.slice(view.start, view.end));

	/** The list's whole painted height, and the spacers that hold its scroll
	 * range while only the rows in view paint. */
	const total = $derived(showsRowsHeight(rows));
	const above = $derived(painted.length > 0 ? painted[0].top : 0);
	const below = $derived(total - (painted.length > 0 ? lastBottom() : above));

	/** The rail's A–Z letters over the painted rows (design D12): the All
	 * series mode paints it, and the flat modes paint none. */
	const rail = $derived(showsLetterRail(rows));

	/** The row the cursor rests on, or `null` while none carries it. */
	const cursorRow = $derived(rows.find((row) => row.id === $showsUi.cursor) ?? null);

	/** The letter the rail marks: the one the cursor row belongs to. */
	const marked = $derived(cursorRow === null ? null : showsRowLetter(cursorRow));

	/** The bottom edge of the last painted row: the below-spacer's pair. */
	function lastBottom(): number {
		const last = painted[painted.length - 1];
		return last === undefined ? 0 : last.top + last.height;
	}

	/** The `var(--canvas-<name>)` value a part paints in: the Library's canvas
	 * hexes, which name no theme slot. */
	function canvasVar(name: string): string {
		return `var(--canvas-${name})`;
	}

	/** The row's painted fill (design D12): the cursor row's focus fill, and
	 * the core's selected row the raised fill while the cursor sits elsewhere —
	 * the selection is what the Hero shows, so its row stays marked. */
	function rowBackground(row: ShowsListRow): string | undefined {
		if (row.kind === 'heading') {
			return undefined;
		}
		if (row.id === $showsUi.cursor) {
			return canvasVar('selected');
		}
		return row.id === shows.selected ? canvasVar('library-raise') : undefined;
	}

	/** Report the list's measured height (design D12): the rows in view follow
	 * it. */
	function measure(): void {
		measureShowsList(listEl?.clientHeight ?? 0);
	}

	/** Report the list's native scroll offset, so the windowed paint follows a
	 * user scroll. */
	function onScroll(event: Event): void {
		scrollShowsList((event.currentTarget as HTMLElement).scrollTop);
	}

	// The scroller mirrors the state (design D12): a key that moved the cursor
	// set the offset, and the element follows it. A user scroll already wrote
	// the offset back, so this assignment is a no-op for it.
	$effect(() => {
		const el = listEl;
		const top = $showsUi.scrollTop;
		if (el !== undefined && el.scrollTop !== top) {
			el.scrollTop = top;
		}
	});

	// The Hero's episode cursor belongs to the selected row (row 7.1): a
	// selection change starts it over, because the episodes it rested on belong
	// to the previous series.
	let lastSelected: string | null = null;
	$effect(() => {
		const selected = shows.selected;
		if (selected === lastSelected) {
			return;
		}
		lastSelected = selected;
		resetShowsEpisodeCursor();
	});

	onMount(() => {
		measure();
		const observer =
			typeof ResizeObserver === 'undefined' ? undefined : new ResizeObserver(measure);
		if (listEl !== undefined) {
			observer?.observe(listEl);
		}
		return () => observer?.disconnect();
	});
</script>

<div class="flex min-h-0 min-w-0 flex-1 flex-col" data-library-shows-list-column="">
	<div class="flex flex-none flex-col" style:padding="8px 16px 0">
		<LibraryFilterField
			value={$showsUi.filter}
			placeholder="Filter shows"
			ariaLabel="Filter shows"
			oninput={setShowsFilter}
			onaccept={acceptShowsFilter}
		/>
	</div>
	{#if shows.loading}
		<!-- The active mode's list has not landed yet (design D12): one
			placeholder line holds the list's place. -->
		<div
			class="flex flex-none items-center"
			style:height="36px"
			style:padding="0 16px"
			style:font-size="13px"
			style:color="var(--canvas-muted)"
			data-library-shows-loading=""
		>
			Loading…
		</div>
	{/if}
	<div class="flex min-h-0 flex-1" style:overflow="hidden">
		<ol
			bind:this={listEl}
			class="min-h-0 flex-1"
			style:overflow-y="auto"
			style:list-style="none"
			style:margin="0"
			style:padding="8px 0 8px 8px"
			data-library-shows-list=""
			onscroll={onScroll}
		>
			<li style:height="{above}px" aria-hidden="true"></li>
			{#each painted as row (row.id)}
				<li>
					{#if row.kind === 'heading'}
						<div
							class="flex items-end font-mono"
							style:height="{row.height}px"
							style:padding="0 8px 2px"
							style:font-size="11px"
							style:color="var(--canvas-source)"
							data-library-shows-letter={row.letter}
						>
							{row.letter}
						</div>
					{:else if row.kind === 'series'}
						<button
							type="button"
							class="flex w-full items-center overflow-hidden"
							style:height="{row.height}px"
							style:padding="0 8px"
							style:gap="8px"
							style:border="0"
							style:border-radius={row.id === shows.selected ? '4px' : '0'}
							style:text-align="left"
							style:font-size="13.5px"
							style:background={rowBackground(row)}
							style:cursor="pointer"
							data-library-shows-row="series"
							data-library-shows-row-id={row.id}
							data-library-shows-cursor={row.id === $showsUi.cursor ? '' : undefined}
							data-library-shows-selected={row.id === shows.selected ? '' : undefined}
							onclick={() => pressShowsRow(row)}
						>
							<span
								class="min-w-0 flex-1 truncate"
								style:color={canvasVar('text')}
								style:font-weight={row.id === shows.selected ? 600 : 400}>{row.row.title}</span
							>
						</button>
					{:else}
						<button
							type="button"
							class="relative flex w-full items-center overflow-hidden"
							style:height="{row.height}px"
							style:padding="0 12px"
							style:gap="10px"
							style:border="0"
							style:border-radius="4px"
							style:text-align="left"
							style:font-size="13px"
							style:background={rowBackground(row)}
							style:cursor="pointer"
							data-library-shows-row="flat"
							data-library-shows-row-id={row.id}
							data-library-shows-cursor={row.id === $showsUi.cursor ? '' : undefined}
							data-library-shows-selected={row.id === shows.selected ? '' : undefined}
							onclick={() => pressShowsRow(row)}
							ondblclick={() => playShowsRow(row.row.id)}
						>
							<span class="flex min-w-0 flex-1 flex-col" style:gap="2px">
								<span
									class="truncate"
									style:color={canvasVar('text')}
									data-library-shows-title="">{row.row.title}</span
								>
								{#if row.row.subtitle !== null}
									<span
										class="truncate"
										style:font-size="11px"
										style:color="var(--canvas-muted)"
										data-library-shows-subtitle="">{row.row.subtitle}</span
									>
								{/if}
							</span>
							{#if row.row.date !== null}
								<span
									class="flex-none tabular-nums"
									style:font-size="11px"
									style:color="var(--canvas-muted)"
									data-library-shows-date="">{row.row.date}</span
								>
							{/if}
						</button>
					{/if}
				</li>
			{/each}
			<li style:height="{below}px" aria-hidden="true"></li>
		</ol>
		{#if showsSeriesMode(mode)}
			<LibraryLetterRail letters={rail} {marked} jump={jumpShowsToLetter} />
		{/if}
	</div>
</div>
