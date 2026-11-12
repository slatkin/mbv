<script lang="ts">
	// The Audiobooks book list (add-gui-library-panel design D12, row 9.1):
	// the catalog's book rows in the TUI's sort — author sort key, then title —
	// under their letter headings, with the A–Z letter rail beside them. No
	// second mode rides on the destination and no pill row: the rail replaces
	// the TUI's surname buckets. The rows paint at the fixed heights of
	// `$lib/library/audiobooks-list` over native scroll, and only the rows in
	// view paint, so a long catalog stays fast.
	//
	// The cursor, the scroll offset, and the viewport are the window content's
	// own in `$lib/library/audiobooks` (design D1); this component paints that
	// state and reports the two facts only the DOM holds. A press selects the
	// book the Hero shows, and a double-click plays it from its saved position —
	// the same play command the keys' Enter sends. A book has no play-next: `n`
	// is consumed and does nothing. The `/` filter field narrows the books to
	// the titles that contain its text, the rail's keyboard path (design D7).
	//
	// The Hero's chapter cursor belongs to the selected book (the
	// `audiobooks-workspace` contract): a selection change starts it over,
	// because the chapters it rested on belong to the previous book.
	import type { GuiAudiobooksDetail, GuiAudiobooksList } from '$lib/bridge/types';
	import {
		buildAudiobooksRows,
		audiobooksLetterRail,
		audiobooksRowLetter,
	} from '$lib/library/audiobooks-list';
	import type { AudiobooksListRow } from '$lib/library/audiobooks-list';
	import {
		acceptAudiobooksFilter,
		audiobooksUi,
		jumpAudiobooksToLetter,
		measureAudiobooksList,
		playAudiobooksRow,
		pressAudiobooksRow,
		scrollAudiobooksList,
		setAudiobooksFilter,
	} from '$lib/library/audiobooks';
	import { audiobooksScroll } from '$lib/library/scroll';
	import { resetAbsChapterCursor } from '$lib/library/audiobooks-workspace';
	import { openRowMenuAt } from '$lib/library/state';
	import LibraryListHeader from './LibraryListHeader.svelte';
	import LibraryListNotice from './LibraryListNotice.svelte';
	import LibraryLetterRail from './LibraryLetterRail.svelte';
	import LibraryItemRow from './LibraryItemRow.svelte';
	import LibraryLetterHeading from './LibraryLetterHeading.svelte';
	import LibraryVirtualList from './LibraryVirtualList.svelte';
	import { canvasVar } from '$lib/bridge/theme';

	interface Props {
		/** The carried Audiobooks content (row 9.1): the catalog's book rows. */
		list: GuiAudiobooksList;
		detail: GuiAudiobooksDetail;
	}
	const { list, detail }: Props = $props();

	/** The painted rows (row 9.1): the book rows with their letter headings,
	 * narrowed by the filter's text. */
	const rows = $derived(buildAudiobooksRows(list.rows, $audiobooksUi.filter));

	/** The rail's A–Z letters over the painted rows (row 9.1). */
	const rail = $derived(audiobooksLetterRail(rows));

	/** The row the cursor rests on, or `null` while none carries it. */
	const cursorRow = $derived(rows.find((row) => row.id === $audiobooksUi.cursor) ?? null);

	/** The letter the rail marks: the one the cursor row belongs to. */
	const marked = $derived(cursorRow === null ? null : audiobooksRowLetter(cursorRow));


	/** The row's painted fill (row 9.1): the cursor row's focus fill, and the
	 * core's selected book the raised fill while the cursor sits elsewhere —
	 * the selection is what the Hero shows, so its row stays marked. */
	function rowBackground(row: AudiobooksListRow): string | undefined {
		if (row.kind === 'heading') {
			return undefined;
		}
		if (row.id === $audiobooksUi.cursor) {
			return canvasVar('selected');
		}
		return row.id === detail.selected ? canvasVar('library-raise') : undefined;
	}

	/** Open the book's right-click menu (the row-menu command): the row's
	 * stable id, which the core resolves for the shared menu (design D8, D15). */
	function rowMenu(event: MouseEvent, row: { id: string }): void {
		openRowMenuAt(event, row.id);
	}

	// The Hero's chapter cursor belongs to the selected book (row 9.1): a
	// selection change starts it over, because the chapters it rested on belong
	// to the previous book.
	let lastSelected: string | null = null;
	$effect(() => {
		const selected = detail.selected;
		if (selected === lastSelected) {
			return;
		}
		lastSelected = selected;
		resetAbsChapterCursor();
	});
</script>

<div class="flex min-h-0 min-w-0 flex-1 flex-col" data-library-abs-list-column="">
	<!-- The Music list's header (design D15): the list names itself, `All` is the
	whole catalog, and the `/` filter field narrows it. -->
	<div class="flex flex-none flex-col">
		<LibraryListHeader
			title="Audiobooks"
			label="All"
			filter={$audiobooksUi.filter}
			placeholder="Filter books"
			ariaLabel="Filter books"
			oninput={setAudiobooksFilter}
			onaccept={acceptAudiobooksFilter}
		/>
	</div>
	{#if list.loading}
		<!-- The book catalog has not landed yet (row 9.1): one placeholder line
			holds the list's place. -->
		<LibraryListNotice text="Loading…" attr="data-library-abs-loading" />
	{/if}
	{#if list.failed}
		<!-- The catalog fetch failed (row 9.1): the failure line naming what
			failed, and no row until the destination opens again. -->
		<LibraryListNotice
			text="Couldn’t load this catalog."
			attr="data-library-abs-failed"
		/>
	{/if}
	{#if !list.loading && !list.failed && list.rows.length === 0}
		<!-- The catalog landed and holds no book (row 9.1): the TUI's empty
			slot. -->
		<LibraryListNotice text="No audiobooks" attr="data-library-abs-empty" />
	{/if}
	<div class="flex min-h-0 flex-1" style:overflow="hidden">
		<LibraryVirtualList
		{rows}
		scrollTop={$audiobooksScroll.scrollTop}
		viewportHeight={$audiobooksScroll.viewportHeight}
		attr="abs"
		padding="8px 0 8px 8px"
		onmeasure={measureAudiobooksList}
		onscroll={scrollAudiobooksList}
	>
		{#snippet row(row)}
			{#if row.kind === 'heading'}
				<LibraryLetterHeading attr="abs" letter={row.letter} height={row.height} />
			{:else}
				<LibraryItemRow
					attr="abs"
					kind="book"
					id={row.id}
					height={row.height}
					title={row.row.title}
					subtitle={row.row.subtitle}
					trail={row.row.duration}
					trailPart="duration"
					titleColor={canvasVar('text')}
					background={rowBackground(row)}
					cursor={row.id === $audiobooksUi.cursor}
					selected={row.id === detail.selected}
					progress={null}
					onpress={() => pressAudiobooksRow(row)}
					onplay={() => playAudiobooksRow(row.row.id)}
					oncontext={(event) => rowMenu(event, row)}
				/>
			{/if}
		{/snippet}
	</LibraryVirtualList>
		<LibraryLetterRail letters={rail} {marked} jump={jumpAudiobooksToLetter} />
	</div>
</div>
