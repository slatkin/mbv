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
	import { onMount } from 'svelte';
	import type { GuiFeedsFrame } from '$lib/bridge/types';
	import {
		buildFeedsRows,
		FEEDS_ROW_HEIGHT,
		feedsListWindow,
		feedsRowsHeight,
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

	interface Props {
		/** The carried Feeds content (design D12, row 11.1): the active pill's
		 * entry rows. */
		feeds: GuiFeedsFrame;
	}
	const { feeds }: Props = $props();

	let listEl: HTMLElement | undefined = $state();

	/** The painted rows of the flat list (row 11.1): the same model
	 * `$lib/library/feeds` builds from the carried content, narrowed by the
	 * filter's text. */
	const rows = $derived(buildFeedsRows(feeds.rows, $feedsUi.filter));

	/** The rows in view: only these paint. */
	const view = $derived(feedsListWindow(rows, $feedsUi.scrollTop, $feedsUi.viewportHeight));
	const painted = $derived(rows.slice(view.start, view.end));

	/** The list's whole painted height, and the spacers that hold its scroll
	 * range while only the rows in view paint. */
	const total = $derived(feedsRowsHeight(rows));
	const above = $derived(painted.length > 0 ? painted[0].top : 0);
	const below = $derived(total - (painted.length > 0 ? lastBottom() : above));

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

	/** Report the list's measured height (row 11.1): the rows in view follow
	 * it. */
	function measure(): void {
		measureFeedsList(listEl?.clientHeight ?? 0);
	}

	/** Report the list's native scroll offset, so the windowed paint follows a
	 * user scroll. */
	function onScroll(event: Event): void {
		scrollFeedsList((event.currentTarget as HTMLElement).scrollTop);
	}

	// The scroller mirrors the state (row 11.1): a key that moved the cursor
	// set the offset, and the element follows it. A user scroll already wrote
	// the offset back, so this assignment is a no-op for it.
	$effect(() => {
		const el = listEl;
		const top = $feedsUi.scrollTop;
		if (el !== undefined && el.scrollTop !== top) {
			el.scrollTop = top;
		}
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
			placeholder line holds the list's place. -->
		<div
			class="flex flex-none items-center"
			style:height="36px"
			style:padding="0 16px"
			style:font-size="13px"
			style:color="var(--canvas-muted)"
			data-library-feeds-loading=""
		>
			Loading…
		</div>
	{/if}
	{#if feeds.failed}
		<!-- A fetch failed (row 11.1): the failure line, and no row until the
			destination opens again. -->
		<div
			class="flex flex-none items-center"
			style:height="36px"
			style:padding="0 16px"
			style:font-size="13px"
			style:color="var(--canvas-muted)"
			data-library-feeds-failed=""
		>
			Couldn’t load this destination.
		</div>
	{/if}
	<ol
		bind:this={listEl}
		class="min-h-0 flex-1"
		style:overflow-y="auto"
		style:list-style="none"
		style:margin="0"
		style:padding="8px 8px 8px 8px"
		data-library-feeds-list=""
		onscroll={onScroll}
	>
		<li style:height="{above}px" aria-hidden="true"></li>
		{#each painted as row (row.id)}
			<li>
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
							class="flex-none tabular-nums"
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
			</li>
		{/each}
		<li style:height="{below}px" aria-hidden="true"></li>
	</ol>
</div>
