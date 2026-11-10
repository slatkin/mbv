<script lang="ts">
	// The Podcasts list (add-gui-library-panel design D12, row 10.1): the
	// active pill's flat episode rows — the episode's title over its show, and
	// the publish date in the gutter — with no letter headings, because the
	// list is sorted by date, and so no letter rail. The rows paint at the
	// fixed heights of `$lib/library/podcasts-list` over native scroll, and
	// only the rows in view paint, so a long Latest merge stays fast.
	//
	// The cursor, the scroll offset, and the viewport are the window content's
	// own in `$lib/library/podcasts` (design D1); this component paints that
	// state and reports the two facts only the DOM holds. A press selects the
	// episode the Hero shows, and a double-click plays it (design D12): the
	// keys' Enter plays the cursor row, and the press is the selection's half.
	// The `/` filter field narrows the episodes to the titles that contain
	// its text (design D7's filter rule).
	import type { GuiPodcastsFrame } from '$lib/bridge/types';
	import {
		buildPodcastsRows,
		PODCASTS_ROW_HEIGHT,
	} from '$lib/library/podcasts-list';
	import type { PodcastsItemRow } from '$lib/library/podcasts-list';
	import {
		acceptPodcastsFilter,
		measurePodcastsList,
		playPodcastsRow,
		pressPodcastsRow,
		podcastsUi,
		scrollPodcastsList,
		setPodcastsFilter,
	} from '$lib/library/podcasts';
	import { openRowMenuAt } from '$lib/library/state';
	import LibraryVirtualList from './LibraryVirtualList.svelte';
	import LibraryListHeader from './LibraryListHeader.svelte';
	import LibraryListNotice from './LibraryListNotice.svelte';
	import { canvasVar } from '$lib/bridge/theme';

	interface Props {
		/** The carried Podcasts content (design D12, row 10.1): the active
		 * pill's episode rows. */
		podcasts: GuiPodcastsFrame;
	}
	const { podcasts }: Props = $props();

	/** The header's selection label (design D15): the active pill's name, so the
	 * column names the view it holds. */
	const selectionLabel = $derived(
		podcasts.pills.find((pill) => pill.selected)?.name ?? 'Latest',
	);

	/** The filter field's placeholder, naming the active view. */
	const filterPlaceholder = $derived(`Filter ${selectionLabel}`);

	/** Open the episode's right-click menu (the row-menu command): the row's
	 * stable id, which the core resolves for the shared menu (design D8, D15). */
	function rowMenu(event: MouseEvent, row: { id: string }): void {
		openRowMenuAt(event, row.id);
	}

	/** The painted rows of the flat list (row 10.1): the same model
	 * `$lib/library/podcasts` builds from the carried content, narrowed by
	 * the filter's text. */
	const rows = $derived(buildPodcastsRows(podcasts.rows, $podcastsUi.filter));


	/** An episode row's title colour: the canvas's text tone, dimmed on a
	 * played row (design D12), as a Home item row. */
	function nameColor(row: PodcastsItemRow): string {
		return row.row.state === 'played' ? canvasVar('muted') : canvasVar('text');
	}

	/** An episode row's painted fill: the cursor row's focus fill, and the
	 * core's selected episode the raised fill while the cursor sits
	 * elsewhere — the selection is what the Hero shows, so its row stays
	 * marked. */
	function rowBackground(row: PodcastsItemRow): string | undefined {
		if (row.id === $podcastsUi.cursor) {
			return canvasVar('selected');
		}
		return row.id === podcasts.selected ? canvasVar('library-raise') : undefined;
	}

	/** The resume percent an active episode row paints, or `null`. */
	function rowProgress(row: PodcastsItemRow): number | null {
		return typeof row.row.state === 'object' ? row.row.state.active.progress : null;
	}
</script>

<div class="flex min-h-0 min-w-0 flex-1 flex-col" data-library-podcasts-list-column="">
	<!-- The Music list's header (design D15): the list names itself, the active
	pill is the carried scope, and the `/` filter field narrows it. -->
	<div class="flex flex-none flex-col">
		<LibraryListHeader
			title="Podcasts"
			label={selectionLabel}
			labelAttr="data-library-podcasts-selection-label"
			filter={$podcastsUi.filter}
			placeholder={filterPlaceholder}
			ariaLabel="Filter episodes"
			oninput={setPodcastsFilter}
			onaccept={acceptPodcastsFilter}
		/>
	</div>
	{#if podcasts.loading}
		<!-- The active view's episodes have not landed yet (row 10.1): one
			placeholder line holds the list's place. -->
		<LibraryListNotice text="Loading…" attr="data-library-podcasts-loading" />
	{/if}
	{#if podcasts.failed}
		<!-- The show catalog's fetch failed (row 10.1): the failure line, and no
			row until the library opens again. -->
		<LibraryListNotice
			text="Couldn’t load this library."
			attr="data-library-podcasts-failed"
		/>
	{/if}
	{#if !podcasts.loading && !podcasts.failed && podcasts.rows.length === 0}
		<!-- The active view holds no episode (row 10.1): the TUI's empty slot. -->
		<LibraryListNotice text="No episodes" attr="data-library-podcasts-empty" />
	{/if}
	<LibraryVirtualList
		{rows}
		scrollTop={$podcastsUi.scrollTop}
		viewportHeight={$podcastsUi.viewportHeight}
		attr="podcasts"
		onmeasure={measurePodcastsList}
		onscroll={scrollPodcastsList}
	>
		{#snippet row(row)}
				<button
					type="button"
					class="relative flex w-full items-center overflow-hidden"
					style:height="{PODCASTS_ROW_HEIGHT}px"
					style:padding="0 10px"
					style:gap="10px"
					style:border="0"
					style:border-radius="4px"
					style:text-align="left"
					style:font-size="13px"
					style:background={rowBackground(row)}
					style:cursor="pointer"
					data-library-podcasts-row="episode"
					data-library-podcasts-row-id={row.id}
					data-library-podcasts-cursor={row.id === $podcastsUi.cursor ? '' : undefined}
					data-library-podcasts-selected={row.id === podcasts.selected ? '' : undefined}
					onclick={() => pressPodcastsRow(row)}
					ondblclick={() => playPodcastsRow(row.row.id)}
					oncontextmenu={(event) => rowMenu(event, row)}
				>
					<span class="flex min-w-0 flex-1 flex-col" style:gap="2px">
						<span
							class="truncate"
							style:color={nameColor(row)}
							style:font-weight={row.id === podcasts.selected ? 600 : 400}
							data-library-podcasts-title="">{row.row.title}</span
						>
						{#if row.row.subtitle !== null}
							<span
								class="truncate"
								style:font-size="11px"
								style:color="var(--canvas-muted)"
								data-library-podcasts-subtitle="">{row.row.subtitle}</span
							>
						{/if}
					</span>
					{#if row.row.date !== null}
						<span
							class="flex-none font-mono tabular-nums"
							style:font-size="11px"
							style:color="var(--canvas-muted)"
							data-library-podcasts-date="">{row.row.date}</span
						>
					{/if}
					{#if rowProgress(row) !== null}
						<!-- The resume position (row 10.1): a thin accent run under the
							row's bottom edge, at the frame's bounded percent. -->
						<span
							class="absolute"
							style:left="0"
							style:bottom="0"
							style:height="2px"
							style:width="{rowProgress(row)}%"
							style:background="var(--canvas-accent)"
							data-library-podcasts-progress=""
						></span>
					{/if}
				</button>
		{/snippet}
	</LibraryVirtualList>
</div>
