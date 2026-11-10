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
	import type { GuiAudiobooksFrame } from '$lib/bridge/types';
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
	import { resetAbsChapterCursor } from '$lib/library/audiobooks-workspace';
	import { openRowMenuAt } from '$lib/library/state';
	import LibraryListHeader from './LibraryListHeader.svelte';
	import LibraryListNotice from './LibraryListNotice.svelte';
	import LibraryLetterRail from './LibraryLetterRail.svelte';
	import LibraryVirtualList from './LibraryVirtualList.svelte';
	import { canvasVar } from '$lib/bridge/theme';

	interface Props {
		/** The carried Audiobooks content (row 9.1): the catalog's book rows. */
		audiobooks: GuiAudiobooksFrame;
	}
	const { audiobooks }: Props = $props();

	/** The painted rows (row 9.1): the book rows with their letter headings,
	 * narrowed by the filter's text. */
	const rows = $derived(buildAudiobooksRows(audiobooks.rows, $audiobooksUi.filter));

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
		return row.id === audiobooks.selected ? canvasVar('library-raise') : undefined;
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
		const selected = audiobooks.selected;
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
	{#if audiobooks.loading}
		<!-- The book catalog has not landed yet (row 9.1): one placeholder line
			holds the list's place. -->
		<LibraryListNotice text="Loading…" attr="data-library-abs-loading" />
	{/if}
	{#if audiobooks.failed}
		<!-- The catalog fetch failed (row 9.1): the failure line naming what
			failed, and no row until the destination opens again. -->
		<LibraryListNotice
			text="Couldn’t load this catalog."
			attr="data-library-abs-failed"
		/>
	{/if}
	{#if !audiobooks.loading && !audiobooks.failed && audiobooks.rows.length === 0}
		<!-- The catalog landed and holds no book (row 9.1): the TUI's empty
			slot. -->
		<LibraryListNotice text="No audiobooks" attr="data-library-abs-empty" />
	{/if}
	<div class="flex min-h-0 flex-1" style:overflow="hidden">
		<LibraryVirtualList
		{rows}
		scrollTop={$audiobooksUi.scrollTop}
		viewportHeight={$audiobooksUi.viewportHeight}
		attr="abs"
		padding="8px 0 8px 8px"
		onmeasure={measureAudiobooksList}
		onscroll={scrollAudiobooksList}
	>
		{#snippet row(row)}
					{#if row.kind === 'heading'}
					<div
						class="flex items-end font-mono"
						style:height="{row.height}px"
						style:padding="0 8px 2px"
						style:font-size="11px"
						style:color="var(--canvas-source)"
						data-library-abs-letter={row.letter}
					>
						{row.letter}
					</div>
				{:else}
					<button
						type="button"
						class="flex w-full items-center overflow-hidden"
						style:height="{row.height}px"
						style:padding="0 12px"
						style:gap="10px"
						style:border="0"
						style:border-radius="4px"
						style:text-align="left"
						style:font-size="13px"
						style:background={rowBackground(row)}
						style:cursor="pointer"
						data-library-abs-row="book"
						data-library-abs-row-id={row.id}
						data-library-abs-cursor={row.id === $audiobooksUi.cursor ? '' : undefined}
						data-library-abs-selected={row.id === audiobooks.selected ? '' : undefined}
						onclick={() => pressAudiobooksRow(row)}
						ondblclick={() => playAudiobooksRow(row.row.id)}
						oncontextmenu={(event) => rowMenu(event, row)}
					>
						<span class="flex min-w-0 flex-1 flex-col" style:gap="2px">
							<span
								class="truncate"
								style:color={canvasVar('text')}
								style:font-weight={row.id === audiobooks.selected ? 600 : 400}
								data-library-abs-title="">{row.row.title}</span
							>
							{#if row.row.subtitle !== null}
								<span
									class="truncate"
									style:font-size="11px"
									style:color="var(--canvas-muted)"
									data-library-abs-subtitle="">{row.row.subtitle}</span
								>
							{/if}
						</span>
						{#if row.row.duration !== ''}
							<span
								class="flex-none font-mono tabular-nums"
								style:font-size="11px"
								style:color="var(--canvas-muted)"
								data-library-abs-duration="">{row.row.duration}</span
							>
						{/if}
					</button>
				{/if}
			{/snippet}
	</LibraryVirtualList>
		<LibraryLetterRail letters={rail} {marked} jump={jumpAudiobooksToLetter} />
	</div>
</div>
