<script lang="ts">
	// The Feeds list (add-gui-library-panel design D12, row 11.1): the active
	// pill's flat entry rows — the entry's title over its subscription, and
	// the entry date in the gutter — with no letter headings, because the list
	// is sorted by date, and so no letter rail. The rows paint at the fixed
	// heights of `$lib/library/feeds-list` over native scroll, and only the
	// rows in view paint, so a long Latest merge stays fast.
	//
	// The cursor, the scroll offset, and the viewport are the window content's
	// own in `$lib/library/feeds` (design D1); this component paints that
	// state and reports the two facts only the DOM holds. A press selects the
	// entry the Hero shows, and a double-click plays it (design D12): the
	// keys' Enter plays the cursor row, and the press is the selection's half.
	// The `/` filter field narrows the entries to the titles that contain
	// its text (design D7's filter rule).
	import type { GuiFeedsDetail, GuiFeedsList } from '$lib/bridge/types';
	import {
		feedsRowsOf,
		FEEDS_ROW_HEIGHT,
	} from '$lib/library/feeds-list';
	import type { FeedsItemRow } from '$lib/library/feeds-list';
	import {
		acceptFeedsFilter,
		measureFeedsList,
		playFeedsRow,
		pressFeedsRow,
		feedsUi,
		scrollFeedsList,
		setFeedsFilter,
	} from '$lib/library/feeds';
	import { feedsScroll } from '$lib/library/scroll';
	import LibraryListHeader from './LibraryListHeader.svelte';
	import LibraryListNotice from './LibraryListNotice.svelte';
	import LibraryItemRow from './LibraryItemRow.svelte';
	import LibraryVirtualList from './LibraryVirtualList.svelte';
	import { canvasVar } from '$lib/bridge/theme';

	interface Props {
		/** The carried Feeds content (design D12, row 11.1): the active pill's
		 * entry rows. */
		list: GuiFeedsList;
		detail: GuiFeedsDetail;
	}
	const { list, detail }: Props = $props();

	/** The painted rows of the flat list (row 11.1): the same model
	 * `$lib/library/feeds` builds from the carried content, narrowed by the
	 * filter's text. */
	/** The header's selection label (design D15): the active pill's name. */
	const selectionLabel = $derived(list.pills.find((pill) => pill.selected)?.name ?? 'Latest');

	const rows = $derived(feedsRowsOf(list.rows, $feedsUi.filter));


	/** An entry row's title colour: the canvas's text tone, dimmed on a played
	 * row (design D12), as a Home item row. */
	function nameColor(row: FeedsItemRow): string {
		return row.row.state === 'played' ? canvasVar('muted') : canvasVar('text');
	}

	/** An entry row's painted fill: the cursor row's focus fill, and the
	 * core's selected entry the raised fill while the cursor sits elsewhere —
	 * the selection is what the Hero shows, so its row stays marked. */
	function rowBackground(row: FeedsItemRow): string | undefined {
		if (row.id === $feedsUi.cursor) {
			return canvasVar('selected');
		}
		return row.id === detail.selected ? canvasVar('library-raise') : undefined;
	}

	/** The resume percent an active entry row paints, or `null`. */
	function rowProgress(row: FeedsItemRow): number | null {
		return typeof row.row.state === 'object' ? row.row.state.active.progress : null;
	}
</script>

<div class="flex min-h-0 min-w-0 flex-1 flex-col" data-library-feeds-list-column="">
	<!-- The Music list's header (design D15): the list names itself, the active
	pill is the carried scope, and the `/` filter field narrows it. -->
	<div class="flex flex-none flex-col">
		<LibraryListHeader
			title="Feeds"
			label={selectionLabel}
			labelAttr="data-library-feeds-selection-label"
			filter={$feedsUi.filter}
			placeholder="Filter {selectionLabel}"
			ariaLabel="Filter entries"
			oninput={setFeedsFilter}
			onaccept={acceptFeedsFilter}
		/>
	</div>
	{#if list.loading}
		<!-- The tab's fetch results have not landed yet (row 11.1): one
			placeholder line holds the list's place, as every Library list's does. -->
		<LibraryListNotice text="Loading…" attr="data-library-feeds-loading" />
	{/if}
	{#if list.failed}
		<!-- A fetch failed (row 11.1): the failure line, and no row until the
			destination opens again. -->
		<LibraryListNotice text="Couldn’t load this destination." attr="data-library-feeds-failed" />
	{/if}
	{#if !list.loading && !list.failed && list.rows.length === 0}
		<!-- The active view holds no entry (row 11.1): the TUI's empty slot. -->
		<LibraryListNotice text="No entries" attr="data-library-feeds-empty" />
	{/if}
	<LibraryVirtualList
		{rows}
		scrollTop={$feedsScroll.scrollTop}
		viewportHeight={$feedsScroll.viewportHeight}
		attr="feeds"
		onmeasure={measureFeedsList}
		onscroll={scrollFeedsList}
	>
		{#snippet row(row)}
			<LibraryItemRow
				attr="feeds"
				kind="entry"
				id={row.id}
				height={FEEDS_ROW_HEIGHT}
				title={row.row.title}
				subtitle={row.row.subtitle}
				trail={row.row.date}
				trailPart="date"
				titleColor={nameColor(row)}
				background={rowBackground(row)}
				cursor={row.id === $feedsUi.cursor}
				selected={row.id === detail.selected}
				progress={rowProgress(row)}
				onpress={() => pressFeedsRow(row)}
				onplay={() => playFeedsRow(row.row.id)}
			/>
		{/snippet}
	</LibraryVirtualList>
</div>
