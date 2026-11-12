<script lang="ts">
	// The Home list (add-gui-library-panel design D12, the USER-APPROVED GUI
	// Home design): the flat list in the Music list slot — the two sections'
	// heading rows (`Continue watching`, `Recently added`), the item rows with
	// their date gutter and their art thumb when the row carries art, and the
	// playing state's styling, and the `/` filter field that narrows the item
	// rows to the titles that contain its text (design D7's filter rule). No
	// letter rail: Home paints none.
	//
	// The rows paint at the fixed heights of `$lib/library/home-list` over
	// native scroll, and only the rows in view paint, so a long list stays
	// fast. The cursor, the scroll offset, and the viewport are the
	// window content's own in `$lib/library/home` (design D1); this component
	// paints that state and reports the two facts only the DOM holds. A press
	// selects the row, and a double-click plays it (design D12): the keys'
	// Enter plays the cursor row, and the press is the selection's half.
	import type { GuiHomeDetail, GuiHomeList } from '$lib/bridge/types';
	import {
		buildHomeRows,
		HOME_ITEM_ROW_HEIGHT,
		homeRowKey,
	} from '$lib/library/home-list';
	import type { HomeListRow } from '$lib/library/home-list';
	import {
		acceptHomeFilter,
		homeUi,
		measureHomeList,
		playHomeRow,
		pressHomeRow,
		scrollHomeList,
		setHomeFilter,
	} from '$lib/library/home';
	import { openRowMenuAt } from '$lib/library/state';
	import ArtImage from './ArtImage.svelte';
	import LibraryListHeader from './LibraryListHeader.svelte';
	import LibraryListNotice from './LibraryListNotice.svelte';
	import LibraryVirtualList from './LibraryVirtualList.svelte';
	import { canvasVar } from '$lib/bridge/theme';

	interface Props {
		/** The carried Home content (design D12): the flat list's rows. */
		list: GuiHomeList;
		detail: GuiHomeDetail;
	}
	const { list, detail }: Props = $props();

	/** Open the item's right-click menu (the row-menu command): the row's own
	 * stable id, whose item the core resolves for the shared menu (design D8,
	 * D15). */
	function rowMenu(event: MouseEvent, row: HomeListRow): void {
		openRowMenuAt(event, row.kind === 'item' ? String(row.rowId.id) : null);
	}

	/** The painted rows of the flat list (design D12): the same model
	 * `$lib/library/home` builds from the carried content, narrowed by the
	 * filter's text. */
	const rows = $derived(buildHomeRows(list.rows, $homeUi.filter));


	/** An item row's name colour: the canvas's text tone, dimmed on a played
	 * row (design D12). */
	function nameColor(row: HomeListRow): string {
		if (row.kind === 'item' && row.state === 'played') {
			return canvasVar('muted');
		}
		return canvasVar('text');
	}

	/** Whether the row is the core's selected one: the item the Hero
	 * shows. */
	function isSelectedRow(row: HomeListRow): boolean {
		return (
			row.kind === 'item' && detail.selected !== null && row.id === homeRowKey(detail.selected)
		);
	}

	/** An item row's painted fill: the cursor row's focus fill, and the
	 * core's selected row the raised fill while the cursor sits elsewhere —
	 * the selection is what the Hero shows, so its row stays marked, as every
	 * Library list's selected row is. */
	function rowBackground(row: HomeListRow): string | undefined {
		if (row.kind === 'item' && row.id === $homeUi.cursor) {
			return canvasVar('selected');
		}
		return isSelectedRow(row) ? canvasVar('library-raise') : undefined;
	}

	/** The resume percent an active item row paints, or `null`. */
	function rowProgress(row: HomeListRow): number | null {
		return row.kind === 'item' && typeof row.state === 'object'
			? row.state.active.progress
			: null;
	}
</script>

<div class="flex min-h-0 min-w-0 flex-1 flex-col" data-library-home-list-column="">
	<!-- The Music list's header (design D15): the list names itself, the
	carried list is the whole destination, and the `/` filter field narrows it. -->
	<div class="flex flex-none flex-col">
		<LibraryListHeader
			title="Home"
			label="All"
			filter={$homeUi.filter}
			placeholder="Filter home"
			ariaLabel="Filter home"
			oninput={setHomeFilter}
			onaccept={acceptHomeFilter}
		/>
	</div>
	{#if list.loading}
		<!-- The Home content has not landed yet (design D12): one placeholder
		line holds the list's place, as the TUI's loading slot does. -->
		<LibraryListNotice text="Loading…" attr="data-library-home-loading" />
	{/if}
	{#if !list.loading && list.rows.length === 0}
		<!-- Home holds nothing to play: one line says so, as the Music list's
		empty line does. -->
		<LibraryListNotice text="Nothing to play" attr="data-library-home-empty" />
	{/if}
	<LibraryVirtualList
		{rows}
		scrollTop={$homeUi.scrollTop}
		viewportHeight={$homeUi.viewportHeight}
		attr="home"
		onmeasure={measureHomeList}
		onscroll={scrollHomeList}
	>
		{#snippet row(row)}
			{#if row.kind === 'heading'}
				<div
					class="flex items-end"
					style:height="{row.height}px"
					style:padding="0 8px 4px"
					style:font-size="14px"
					style:font-weight="600"
					style:color="var(--canvas-heading)"
					data-library-home-heading={row.text}
				>
					{row.text}
				</div>
			{:else if row.kind === 'item'}
				<button
					type="button"
					class="relative flex w-full items-center overflow-hidden"
					style:height="{HOME_ITEM_ROW_HEIGHT}px"
					style:padding="0 10px"
					style:gap="10px"
					style:border="0"
					style:border-radius="4px"
					style:text-align="left"
					style:font-size="14px"
					style:background={rowBackground(row)}
					style:cursor="pointer"
					data-library-home-row="item"
					data-library-home-row-id={row.id}
					data-library-home-cursor={row.id === $homeUi.cursor ? '' : undefined}
					data-library-home-selected={isSelectedRow(row) ? '' : undefined}
					onclick={() => pressHomeRow(row)}
					ondblclick={() => playHomeRow(row.rowId)}
					oncontextmenu={(event) => rowMenu(event, row)}
				>
					{#if row.art !== null}
						<!-- The row's card art (design D12): the frame's image reference,
							or the empty box while it is loading. The rows without art paint
							no box at all. -->
						<span
							class="flex flex-none items-center justify-center"
							style:width="40px"
							style:height="40px"
							style:background="var(--canvas-library-raise)"
							style:border-radius="4px"
							style:overflow="hidden"
							data-library-home-art={row.art.kind}
						>
							<ArtImage art={row.art} alt="" class="h-full w-full object-cover" />
						</span>
					{/if}
					<span class="flex min-w-0 flex-1 flex-col" style:gap="2px">
						<span
							class="truncate"
							style:color={nameColor(row)}
							style:font-weight={isSelectedRow(row) ? 600 : 400}
							data-library-home-title="">{row.title}</span
						>
						{#if row.subtitle !== null}
							<span
								class="truncate"
								style:font-size="12px"
								style:color="var(--canvas-muted)"
								data-library-home-subtitle="">{row.subtitle}</span
							>
						{/if}
					</span>
					{#if row.date !== null}
						<span
							class="flex-none font-mono tabular-nums"
							style:font-size="12px"
							style:color="var(--canvas-muted)"
							data-library-home-date="">{row.date}</span
						>
					{/if}
					{#if rowProgress(row) !== null}
						<!-- The resume position (design D12): a thin accent run under
							the row's bottom edge, at the frame's bounded percent. -->
						<span
							class="absolute"
							style:left="0"
							style:bottom="0"
							style:height="2px"
							style:width="{rowProgress(row)}%"
							style:background="var(--canvas-accent)"
							data-library-home-progress=""
						></span>
					{/if}
				</button>
			{:else}
				<!-- The spacer between two non-empty sections (design D12). -->
				<div style:height="{row.height}px" aria-hidden="true"></div>
			{/if}
		{/snippet}
	</LibraryVirtualList>
</div>
