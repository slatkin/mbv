<script lang="ts">
	// The Shows list (add-gui-library-panel design D14): the Music tree over
	// the TUI's TV node kinds — series nodes, their seasons, and each season's
	// episodes — under the letter headings of the first series of each letter,
	// with the A–Z rail beside them, and the Latest and Upcoming modes' flat
	// rows with their date gutter and no rail. The list header is the Music
	// list's: the `Series` title, the active mode as the selection label, and
	// the `/` filter field. The rows paint at the fixed heights of
	// `$lib/library/shows-list` over native scroll, and only the rows in view
	// paint, so a long series list stays fast.
	//
	// The cursor, the open and closed nodes, the scroll offset, the viewport,
	// and the filter text are the window content's own in `$lib/library/shows`
	// (design D1); this component paints that state and reports the two facts
	// only the DOM holds. A press on a node opens or closes it and selects it
	// for the Hero, a press on a leaf or a flat row selects it, a double-click
	// on an episode plays it, and a right-click opens the item menu, as the
	// Music list's rows do.
	import type { GuiShowsDetail, GuiShowsList } from '$lib/bridge/types';
	import {
		activeShowsMode,
		showsLetterRail,
		showsRowsOf,
		showsRowLetter,
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
	import { showsScroll } from '$lib/library/scroll';
	import { openRowMenuAt } from '$lib/library/state';
	import { isPlayedEpisode, resetShowsEpisodeCursor } from '$lib/library/shows-workspace';
	import LibraryLetterRail from './LibraryLetterRail.svelte';
	import LibraryListHeader from './LibraryListHeader.svelte';
	import LibraryListNotice from './LibraryListNotice.svelte';
	import LibraryItemRow from './LibraryItemRow.svelte';
	import LibraryLetterHeading from './LibraryLetterHeading.svelte';
	import LibraryVirtualList from './LibraryVirtualList.svelte';
	import { canvasVar } from '$lib/bridge/theme';

	interface Props {
		/** The carried Shows content (design D14): the tree or the flat rows. */
		list: GuiShowsList;
		detail: GuiShowsDetail;
	}
	const { list, detail }: Props = $props();

	/** The active view mode: the pill the core marked. */
	const mode = $derived(activeShowsMode(list));

	/** The painted rows (design D14): the tree in the All mode, the flat rows
	 * in the Latest and Upcoming modes, narrowed by the filter's text. */
	const rows = $derived(
		showsRowsOf(list, mode, $showsUi.filter, $showsUi.open, $showsUi.closed),
	);

	/** The rail's A–Z letters over the painted rows: the All series mode paints
	 * it, and the flat modes paint none. */
	const rail = $derived(showsLetterRail(rows));

	/** The row the cursor rests on, or `null` while none carries it. */
	const cursorRow = $derived(rows.find((row) => row.id === $showsUi.cursor) ?? null);

	/** The letter the rail marks: the one the cursor row belongs to. */
	const marked = $derived(cursorRow === null ? null : showsRowLetter(cursorRow));

	/** The header's right line, in the Music list's shape: the active mode and
	 * its sort rule. The All mode is sorted by name, the flat modes by date. */
	const selectionLabel = $derived(
		mode === 'all'
			? `${list.collectionNames[0] ?? 'All'} · sort name`
			: (list.collectionNames[0] ?? 'All'),
	);

	/** The filter field's placeholder, naming the selection as the Music list
	 * does. */
	const filterPlaceholder = $derived(`Filter ${list.collectionNames[0] ?? 'shows'}`);


	/** The stable Service id a row names, or `null` on a heading: the item the
	 * menu and the double-click act on. */
	function rowSourceId(row: ShowsListRow): string | null {
		if (row.kind === 'heading') {
			return null;
		}
		return row.kind === 'flat' ? row.row.id : row.sourceId;
	}

	/** Whether the row is the core's selected one: the series the Hero shows,
	 * or the flat row it shows. */
	function isSelectedRow(row: ShowsListRow): boolean {
		return detail.selected !== null && rowSourceId(row) === detail.selected;
	}

	/** The row's painted fill (design D14): the cursor row's focus fill, and
	 * the core's selected row the raised fill while the cursor sits elsewhere —
	 * the selection is what the Hero shows, so its row stays marked. */
	function rowBackground(row: ShowsListRow): string | undefined {
		if (row.kind === 'heading') {
			return undefined;
		}
		if (row.id === $showsUi.cursor) {
			return canvasVar('selected');
		}
		return isSelectedRow(row) ? canvasVar('library-raise') : undefined;
	}

	/** The row's indentation: the Music tree's depths — the series at the list's
	 * own inset, the season under it, the episode under the season. */
	function rowInset(row: ShowsListRow): string {
		switch (row.kind) {
			case 'series':
			case 'flat':
				return '4px';
			case 'season':
				return '22px';
			case 'episode':
				return '40px';
			default:
				return '8px';
		}
	}

	/** The row's name colour: the canvas's text tone, dimmed once an episode is
	 * played, as the Workspace's rows are. */
	function rowNameColor(row: ShowsListRow): string {
		if (row.kind === 'episode') {
			return episodePlayed(row.id) ? canvasVar('muted') : canvasVar('text');
		}
		return canvasVar('text');
	}

	/** Whether an episode leaf names a played episode: its Workspace row carries
	 * the shared media state. */
	function episodePlayed(rowId: string): boolean {
		const episodeId = rowId.slice('shows-episode:'.length);
		for (const series of list.series) {
			for (const season of series.seasons) {
				const episode = season.episodes?.find((row) => row.episodeId === episodeId);
				if (episode !== undefined) {
					return isPlayedEpisode(episode.state);
				}
			}
		}
		return false;
	}

	/** The chevron's rotation: the open node's caret points down. */
	function chevronStyle(open: boolean): string {
		return `transform: rotate(${open ? 90 : 0}deg);`;
	}

	/** Open the item menu for the row (design D14): the Music list's
	 * right-click, at the item's stable Service id. */
	function rowMenu(event: MouseEvent, row: ShowsListRow): void {
		openRowMenuAt(event, rowSourceId(row));
	}

	// The Hero's episode cursor belongs to the selected row: a selection change
	// starts it over, because the episodes it rested on belong to the previous
	// series.
	let lastSelected: string | null = null;
	$effect(() => {
		const selected = detail.selected;
		if (selected === lastSelected) {
			return;
		}
		lastSelected = selected;
		resetShowsEpisodeCursor();
	});
</script>

<div class="flex min-h-0 min-w-0 flex-1 flex-col" data-library-shows-list-column="">
	<!-- The Music list's header (design D14): the title, the active mode as the
	selection label, and the `/` filter field. -->
	<div class="flex flex-none flex-col">
		<LibraryListHeader
			title="Series"
			titleAttr="data-library-shows-list-title"
			label={selectionLabel}
			labelAttr="data-library-shows-selection-label"
			filter={$showsUi.filter}
			placeholder={filterPlaceholder}
			ariaLabel="Filter series"
			oninput={setShowsFilter}
			onaccept={acceptShowsFilter}
		/>
	</div>
	{#if list.loading}
		<!-- The active mode's list has not landed yet: one placeholder line
		holds the list's place. -->
		<LibraryListNotice text="Loading…" attr="data-library-shows-loading" />
	{:else if list.upcomingFailed}
		<!-- The Upcoming mode's eager fetch failed (design D14): the TUI's line,
		and the pill is the retry. -->
		<LibraryListNotice
			text="Upcoming fetch failed — select the pill to retry"
			attr="data-library-shows-failed"
		/>
	{/if}
	<div class="flex min-h-0 flex-1" style:overflow="hidden">
		<LibraryVirtualList
		{rows}
		scrollTop={$showsScroll.scrollTop}
		viewportHeight={$showsScroll.viewportHeight}
		attr="shows"
		padding="8px 0 8px 8px"
		onmeasure={measureShowsList}
		onscroll={scrollShowsList}
	>
		{#snippet row(row)}
					{#if row.kind === 'heading'}
					<LibraryLetterHeading attr="shows" letter={row.letter} height={row.height} />
				{:else if row.kind === 'series' || row.kind === 'season'}
					<button
						type="button"
						class="flex w-full items-center overflow-hidden"
						style:height="{row.height}px"
						style:padding="0 8px"
						style:padding-left={rowInset(row)}
						style:gap="8px"
						style:border="0"
						style:border-radius="4px"
						style:text-align="left"
						style:font-size="14.5px"
						style:background={rowBackground(row)}
						style:cursor="pointer"
						aria-expanded={row.expandable ? row.open : undefined}
						data-library-shows-row={row.kind}
						data-library-shows-row-id={row.id}
						data-library-shows-cursor={row.id === $showsUi.cursor ? '' : undefined}
						data-library-shows-selected={isSelectedRow(row) ? '' : undefined}
						data-library-shows-open={row.open ? '' : undefined}
						onclick={() => pressShowsRow(row)}
						oncontextmenu={(event) => rowMenu(event, row)}
					>
						<span
							class="flex flex-none items-center justify-center"
							style:width="18px"
							style:color="var(--canvas-muted)"
							data-library-shows-chevron={row.expandable ? '' : undefined}
						>
							{#if row.expandable}
								<svg
									width="10"
									height="10"
									viewBox="0 0 24 24"
									fill="none"
									stroke="currentColor"
									stroke-width="3"
									stroke-linecap="round"
									stroke-linejoin="round"
									aria-hidden="true"
									style={chevronStyle(row.open)}><path d="m9 5 7 7-7 7" /></svg
								>
							{/if}
						</span>
						<span
							class="min-w-0 flex-1 truncate"
							style:color={rowNameColor(row)}
							style:font-weight={row.kind === 'series' || isSelectedRow(row) ? 600 : 400}
							>{row.title}</span
						>
					</button>
				{:else if row.kind === 'episode'}
					<button
						type="button"
						class="flex w-full items-center overflow-hidden"
						style:height="{row.height}px"
						style:padding="0 8px"
						style:padding-left={rowInset(row)}
						style:gap="8px"
						style:border="0"
						style:border-radius="4px"
						style:text-align="left"
						style:font-size="14px"
						style:background={rowBackground(row)}
						style:cursor="pointer"
						data-library-shows-row="episode"
						data-library-shows-row-id={row.id}
						data-library-shows-cursor={row.id === $showsUi.cursor ? '' : undefined}
						data-library-shows-selected={isSelectedRow(row) ? '' : undefined}
						onclick={() => pressShowsRow(row)}
						ondblclick={() => playShowsRow(row.sourceId)}
						oncontextmenu={(event) => rowMenu(event, row)}
					>
						<span
							class="flex-none font-mono tabular-nums"
							style:font-size="12px"
							style:color="var(--canvas-ordinal)"
							data-library-shows-episode-number="">{row.number}</span
						>
						<span
							class="min-w-0 flex-1 truncate"
							style:color={rowNameColor(row)}
							data-library-shows-title="">{row.title}</span
						>
						{#if row.duration !== ''}
							<span
								class="flex-none font-mono tabular-nums"
								style:font-size="12px"
								style:color="var(--canvas-muted)"
								data-library-shows-episode-duration="">{row.duration}</span
							>
						{/if}
					</button>
				{:else}
					<LibraryItemRow
						attr="shows"
						kind="flat"
						id={row.id}
						height={row.height}
						title={row.row.title}
						subtitle={row.row.subtitle}
						trail={row.row.date}
						trailPart="date"
						titleColor={canvasVar('text')}
						background={rowBackground(row)}
						cursor={row.id === $showsUi.cursor}
						selected={isSelectedRow(row)}
						progress={null}
						onpress={() => pressShowsRow(row)}
						onplay={() => playShowsRow(row.row.id)}
						oncontext={(event) => rowMenu(event, row)}
					/>
				{/if}
			{/snippet}
	</LibraryVirtualList>
		{#if showsSeriesMode(mode)}
			<LibraryLetterRail letters={rail} {marked} jump={jumpShowsToLetter} />
		{/if}
	</div>
</div>
