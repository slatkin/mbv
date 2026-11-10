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
	import { onMount } from 'svelte';
	import type { GuiPodcastsFrame } from '$lib/bridge/types';
	import {
		buildPodcastsRows,
		PODCASTS_ROW_HEIGHT,
		podcastsListWindow,
		podcastsRowsHeight,
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
	import LibraryFilterField from './LibraryFilterField.svelte';

	interface Props {
		/** The carried Podcasts content (design D12, row 10.1): the active
		 * pill's episode rows. */
		podcasts: GuiPodcastsFrame;
	}
	const { podcasts }: Props = $props();

	let listEl: HTMLElement | undefined = $state();

	/** The painted rows of the flat list (row 10.1): the same model
	 * `$lib/library/podcasts` builds from the carried content, narrowed by
	 * the filter's text. */
	const rows = $derived(buildPodcastsRows(podcasts.rows, $podcastsUi.filter));

	/** The rows in view: only these paint. */
	const view = $derived(podcastsListWindow(rows, $podcastsUi.scrollTop, $podcastsUi.viewportHeight));
	const painted = $derived(rows.slice(view.start, view.end));

	/** The list's whole painted height, and the spacers that hold its scroll
	 * range while only the rows in view paint. */
	const total = $derived(podcastsRowsHeight(rows));
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

	/** Report the list's measured height (row 10.1): the rows in view follow
	 * it. */
	function measure(): void {
		measurePodcastsList(listEl?.clientHeight ?? 0);
	}

	/** Report the list's native scroll offset, so the windowed paint follows a
	 * user scroll. */
	function onScroll(event: Event): void {
		scrollPodcastsList((event.currentTarget as HTMLElement).scrollTop);
	}

	// The scroller mirrors the state (row 10.1): a key that moved the cursor
	// set the offset, and the element follows it. A user scroll already wrote
	// the offset back, so this assignment is a no-op for it.
	$effect(() => {
		const el = listEl;
		const top = $podcastsUi.scrollTop;
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

<div class="flex min-h-0 min-w-0 flex-1 flex-col" data-library-podcasts-list-column="">
	<div class="flex flex-none flex-col" style:padding="8px 16px 0">
		<LibraryFilterField
			value={$podcastsUi.filter}
			placeholder="Filter episodes"
			ariaLabel="Filter episodes"
			oninput={setPodcastsFilter}
			onaccept={acceptPodcastsFilter}
		/>
	</div>
	{#if podcasts.loading}
		<!-- The active view's episodes have not landed yet (row 10.1): one
			placeholder line holds the list's place. -->
		<div
			class="flex flex-none items-center"
			style:height="36px"
			style:padding="0 16px"
			style:font-size="13px"
			style:color="var(--canvas-muted)"
			data-library-podcasts-loading=""
		>
			Loading…
		</div>
	{/if}
	{#if podcasts.failed}
		<!-- The show catalog's fetch failed (row 10.1): the failure line, and no
			row until the library opens again. -->
		<div
			class="flex flex-none items-center"
			style:height="36px"
			style:padding="0 16px"
			style:font-size="13px"
			style:color="var(--canvas-muted)"
			data-library-podcasts-failed=""
		>
			Couldn’t load this library.
		</div>
	{/if}
	<ol
		bind:this={listEl}
		class="min-h-0 flex-1"
		style:overflow-y="auto"
		style:list-style="none"
		style:margin="0"
		style:padding="8px 8px 8px 8px"
		data-library-podcasts-list=""
		onscroll={onScroll}
	>
		<li style:height="{above}px" aria-hidden="true"></li>
		{#each painted as row (row.id)}
			<li>
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
				>
					<span class="flex min-w-0 flex-1 flex-col" style:gap="2px">
						<span
							class="truncate"
							style:color={nameColor(row)}
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
							class="flex-none tabular-nums"
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
			</li>
		{/each}
		<li style:height="{below}px" aria-hidden="true"></li>
	</ol>
</div>
