<script lang="ts">
	// The plain Emby-library list (add-gui-library-panel design D12, row 8.1):
	// the plain mode's name-sorted rows under their letter headings, with the
	// A–Z letter rail beside them, and the feed-group mode's flat rows with
	// their date gutter and no rail. The rows paint at the fixed heights of
	// `$lib/library/emby-list` over native scroll, and only the rows in view
	// paint, so a long library stays fast.
	//
	// The cursor, the scroll offset, and the viewport are the window content's
	// own in `$lib/library/emby` (design D1); this component paints that state
	// and reports the two facts only the DOM holds. A press selects the row the
	// Hero shows, a double-click plays it, and the keys' Enter plays the row
	// under the cursor — every row is playable, because the core's folder-play
	// rule expands a folder or a boxset row. The `/` filter field narrows the
	// rows to the titles that contain its text, the rail's keyboard path in
	// the plain mode (design D7).
	//
	// A collection row paints its title only: the Music and Home lists paint no
	// folder glyph, so the Emby list adds none and the row's own decoration (the
	// folder's item count) carries the distinction, with the kind on
	// `data-library-emby-row`.
	import type { GuiEmbyDetail, GuiEmbyList } from '$lib/bridge/types';
	import {
		buildEmbyRows,
		embyLetterRail,
		embyRowLetter,
	} from '$lib/library/emby-list';
	import type { EmbyListRow } from '$lib/library/emby-list';
	import {
		acceptEmbyFilter,
		embyUi,
		jumpEmbyToLetter,
		measureEmbyList,
		playEmbyRow,
		pressEmbyRow,
		scrollEmbyList,
		setEmbyFilter,
	} from '$lib/library/emby';
	import { embyScroll } from '$lib/library/scroll';
	import { openRowMenuAt } from '$lib/library/state';
	import LibraryLetterRail from './LibraryLetterRail.svelte';
	import LibraryListHeader from './LibraryListHeader.svelte';
	import LibraryListNotice from './LibraryListNotice.svelte';
	import LibraryItemRow from './LibraryItemRow.svelte';
	import LibraryLetterHeading from './LibraryLetterHeading.svelte';
	import LibraryVirtualList from './LibraryVirtualList.svelte';
	import { canvasVar } from '$lib/bridge/theme';

	interface Props {
		/** The carried plain Emby-library content (row 8.1): the active mode's
		 * rows. */
		list: GuiEmbyList;
		detail: GuiEmbyDetail;
	}
	const { list, detail }: Props = $props();

	/** The painted rows (row 8.1): the plain mode's rows with their headings, or
	 * the feed-group mode's flat rows with no heading, narrowed by the
	 * filter's text. */
	const rows = $derived(buildEmbyRows(list.rows, list.feedGroup, $embyUi.filter));

	/** The header's selection label (design D15): the feed-group mode's
	 * selected group, and `All` for a plain library, whose list is the whole
	 * library — the Music header's own rule. */
	const selectionLabel = $derived(
		list.collectionNames.length > 0 ? list.collectionNames.join(' + ') : 'All',
	);

	/** The filter field's placeholder, naming the selection it narrows. */
	const filterPlaceholder = $derived(
		list.collectionNames.length > 0 ? `Filter ${list.collectionNames.join(' + ')}` : 'Filter',
	);

	/** Open the row's right-click item menu (the row-menu command): the next
	 * frame carries the item's entries and the panel paints the card at the
	 * pointer. */
	function rowMenu(event: MouseEvent, row: EmbyListRow): void {
		openRowMenuAt(event, row.kind === 'item' ? row.row.id : null);
	}

	/** The rail's A–Z letters over the painted rows (row 8.1): the plain mode
	 * paints it, and the date-ordered feed-group mode paints none. */
	const rail = $derived(embyLetterRail(rows));

	/** The row the cursor rests on, or `null` while none carries it. */
	const cursorRow = $derived(rows.find((row) => row.id === $embyUi.cursor) ?? null);

	/** The letter the rail marks: the one the cursor row belongs to. */
	const marked = $derived(cursorRow === null ? null : embyRowLetter(cursorRow));


	/** The row's painted fill (row 8.1): the cursor row's focus fill, and the
	 * core's selected row the raised fill while the cursor sits elsewhere — the
	 * selection is what the Hero shows, so its row stays marked. */
	function rowBackground(row: EmbyListRow): string | undefined {
		if (row.kind === 'heading') {
			return undefined;
		}
		if (row.id === $embyUi.cursor) {
			return canvasVar('selected');
		}
		return row.id === detail.selected ? canvasVar('library-raise') : undefined;
	}
</script>

<div class="flex min-h-0 min-w-0 flex-1 flex-col" data-library-emby-list-column="">
	<!-- The Music list's header (design D15): the library's name, the carried
	selection's label, and the `/` filter field. -->
	<div class="flex flex-none flex-col">
		<LibraryListHeader
			title={list.libraryName}
			label={selectionLabel}
			labelAttr="data-library-emby-selection-label"
			filter={$embyUi.filter}
			placeholder={filterPlaceholder}
			ariaLabel="Filter items"
			oninput={setEmbyFilter}
			onaccept={acceptEmbyFilter}
		/>
	</div>
	{#if list.loading}
		<!-- The active list has not landed yet (row 8.1): one placeholder line
			holds the list's place. -->
		<LibraryListNotice text="Loading…" attr="data-library-emby-loading" />
	{/if}
	{#if list.failed}
		<!-- The plain mode's root-list fetch failed (row 8.1): the failure line
			naming what failed, and no row until the library opens again. -->
		<LibraryListNotice
			text="Couldn’t load this library."
			attr="data-library-emby-failed"
		/>
	{/if}
	{#if !list.loading && !list.failed && list.rows.length === 0}
		<!-- The library landed and holds no item (row 8.1): one line says so,
			as the Music list's empty line does. -->
		<LibraryListNotice text="No items" attr="data-library-emby-empty" />
	{/if}
	<div class="flex min-h-0 flex-1" style:overflow="hidden">
		<LibraryVirtualList
		{rows}
		scrollTop={$embyScroll.scrollTop}
		viewportHeight={$embyScroll.viewportHeight}
		attr="emby"
		padding="8px 0 8px 8px"
		onmeasure={measureEmbyList}
		onscroll={scrollEmbyList}
	>
		{#snippet row(row)}
			{#if row.kind === 'heading'}
				<LibraryLetterHeading attr="emby" letter={row.letter} height={row.height} />
			{:else}
				<LibraryItemRow
					attr="emby"
					kind={row.row.kind}
					id={row.id}
					height={row.height}
					title={row.row.title}
					subtitle={row.row.subtitle}
					trail={row.row.date}
					trailPart="date"
					titleColor={canvasVar('text')}
					background={rowBackground(row)}
					cursor={row.id === $embyUi.cursor}
					selected={row.id === detail.selected}
					progress={null}
					onpress={() => pressEmbyRow(row)}
					onplay={() => playEmbyRow(row.row.id)}
					oncontext={(event) => rowMenu(event, row)}
				/>
			{/if}
		{/snippet}
	</LibraryVirtualList>
		{#if !list.feedGroup}
			<LibraryLetterRail letters={rail} {marked} jump={jumpEmbyToLetter} />
		{/if}
	</div>
</div>
