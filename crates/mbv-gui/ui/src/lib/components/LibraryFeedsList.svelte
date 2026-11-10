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
	import type { GuiFeedsFrame } from '$lib/bridge/types';
	import {
		buildFeedsRows,
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
	import LibraryFilterField from './LibraryFilterField.svelte';
	import LibraryListNotice from './LibraryListNotice.svelte';
	import LibraryVirtualList from './LibraryVirtualList.svelte';
	import { canvasVar } from '$lib/bridge/theme';

	interface Props {
		/** The carried Feeds content (design D12, row 11.1): the active pill's
		 * entry rows. */
		feeds: GuiFeedsFrame;
	}
	const { feeds }: Props = $props();

	/** The painted rows of the flat list (row 11.1): the same model
	 * `$lib/library/feeds` builds from the carried content, narrowed by the
	 * filter's text. */
	const rows = $derived(buildFeedsRows(feeds.rows, $feedsUi.filter));


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
		return row.id === feeds.selected ? canvasVar('library-raise') : undefined;
	}

	/** The resume percent an active entry row paints, or `null`. */
	function rowProgress(row: FeedsItemRow): number | null {
		return typeof row.row.state === 'object' ? row.row.state.active.progress : null;
	}
</script>

<div class="flex min-h-0 min-w-0 flex-1 flex-col" data-library-feeds-list-column="">
	<div class="flex flex-none flex-col" style:padding="8px 16px 0">
		<LibraryFilterField
			value={$feedsUi.filter}
			placeholder="Filter entries"
			ariaLabel="Filter entries"
			oninput={setFeedsFilter}
			onaccept={acceptFeedsFilter}
		/>
	</div>
	{#if feeds.loading}
		<!-- The tab's fetch results have not landed yet (row 11.1): one
			placeholder line holds the list's place, as every Library list's does. -->
		<LibraryListNotice text="Loading…" attr="data-library-feeds-loading" />
	{/if}
	{#if feeds.failed}
		<!-- A fetch failed (row 11.1): the failure line, and no row until the
			destination opens again. -->
		<LibraryListNotice text="Couldn’t load this destination." attr="data-library-feeds-failed" />
	{/if}
	<LibraryVirtualList
		{rows}
		scrollTop={$feedsUi.scrollTop}
		viewportHeight={$feedsUi.viewportHeight}
		attr="feeds"
		onmeasure={measureFeedsList}
		onscroll={scrollFeedsList}
	>
		{#snippet row(row)}
			<button
				type="button"
				class="relative flex w-full items-center overflow-hidden"
				style:height="{FEEDS_ROW_HEIGHT}px"
				style:padding="0 10px"
				style:gap="10px"
				style:border="0"
				style:border-radius="4px"
				style:text-align="left"
				style:font-size="13px"
				style:background={rowBackground(row)}
				style:cursor="pointer"
				data-library-feeds-row="entry"
				data-library-feeds-row-id={row.id}
				data-library-feeds-cursor={row.id === $feedsUi.cursor ? '' : undefined}
				data-library-feeds-selected={row.id === feeds.selected ? '' : undefined}
				onclick={() => pressFeedsRow(row)}
				ondblclick={() => playFeedsRow(row.id)}
			>
				<span class="flex min-w-0 flex-1 flex-col" style:gap="2px">
					<span
						class="truncate"
						style:color={nameColor(row)}
						style:font-weight={row.id === feeds.selected ? 600 : 400}
						data-library-feeds-title="">{row.row.title}</span
					>
					{#if row.row.subtitle !== null}
						<span
							class="truncate"
							style:font-size="11px"
							style:color="var(--canvas-muted)"
							data-library-feeds-subtitle="">{row.row.subtitle}</span
						>
					{/if}
				</span>
				{#if row.row.date !== null}
					<span
						class="flex-none font-mono tabular-nums"
						style:font-size="11px"
						style:color="var(--canvas-muted)"
						data-library-feeds-date="">{row.row.date}</span
					>
				{/if}
				{#if rowProgress(row) !== null}
					<!-- The resume position (row 11.1): a thin accent run under the
						row's bottom edge, at the frame's bounded percent. -->
					<span
						class="absolute"
						style:left="0"
						style:bottom="0"
						style:height="2px"
						style:width="{rowProgress(row)}%"
						style:background="var(--canvas-accent)"
						data-library-feeds-progress=""
					></span>
				{/if}
			</button>
		{/snippet}
	</LibraryVirtualList>
</div>
