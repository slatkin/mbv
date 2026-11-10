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
	import { onMount } from 'svelte';
	import type { GuiEmbyFrame } from '$lib/bridge/types';
	import {
		buildEmbyRows,
		embyLetterRail,
		embyListWindow,
		embyRowLetter,
		embyRowsHeight,
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
	import { openLibraryRowMenu } from '$lib/library/state';
	import LibraryLetterRail from './LibraryLetterRail.svelte';
	import LibraryListHeader from './LibraryListHeader.svelte';
	import LibraryListNotice from './LibraryListNotice.svelte';

	interface Props {
		/** The carried plain Emby-library content (row 8.1): the active mode's
		 * rows. */
		emby: GuiEmbyFrame;
	}
	const { emby }: Props = $props();

	let listEl: HTMLElement | undefined = $state();

	/** The painted rows (row 8.1): the plain mode's rows with their headings, or
	 * the feed-group mode's flat rows with no heading, narrowed by the
	 * filter's text. */
	const rows = $derived(buildEmbyRows(emby.rows, emby.feedGroup, $embyUi.filter));

	/** The header's selection label (design D15): the feed-group mode's
	 * selected group, and `All` for a plain library, whose list is the whole
	 * library — the Music header's own rule. */
	const selectionLabel = $derived(
		emby.collectionNames.length > 0 ? emby.collectionNames.join(' + ') : 'All',
	);

	/** The filter field's placeholder, naming the selection it narrows. */
	const filterPlaceholder = $derived(
		emby.collectionNames.length > 0 ? `Filter ${emby.collectionNames.join(' + ')}` : 'Filter',
	);

	/** Open the row's right-click item menu (the row-menu command): the next
	 * frame carries the item's entries and the panel paints the card at the
	 * pointer. */
	function rowMenu(event: MouseEvent, row: EmbyListRow): void {
		event.preventDefault();
		if (row.kind === 'item') {
			openLibraryRowMenu(row.row.id, event.clientX, event.clientY);
		}
	}

	/** The rows in view: only these paint. */
	const view = $derived(embyListWindow(rows, $embyUi.scrollTop, $embyUi.viewportHeight));
	const painted = $derived(rows.slice(view.start, view.end));

	/** The list's whole painted height, and the spacers that hold its scroll
	 * range while only the rows in view paint. */
	const total = $derived(embyRowsHeight(rows));
	const above = $derived(painted.length > 0 ? painted[0].top : 0);
	const below = $derived(total - (painted.length > 0 ? lastBottom() : above));

	/** The rail's A–Z letters over the painted rows (row 8.1): the plain mode
	 * paints it, and the date-ordered feed-group mode paints none. */
	const rail = $derived(embyLetterRail(rows));

	/** The row the cursor rests on, or `null` while none carries it. */
	const cursorRow = $derived(rows.find((row) => row.id === $embyUi.cursor) ?? null);

	/** The letter the rail marks: the one the cursor row belongs to. */
	const marked = $derived(cursorRow === null ? null : embyRowLetter(cursorRow));

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
		return row.id === emby.selected ? canvasVar('library-raise') : undefined;
	}

	/** Report the list's measured height (row 8.1): the rows in view follow it. */
	function measure(): void {
		measureEmbyList(listEl?.clientHeight ?? 0);
	}

	/** Report the list's native scroll offset, so the windowed paint follows a
	 * user scroll. */
	function onScroll(event: Event): void {
		scrollEmbyList((event.currentTarget as HTMLElement).scrollTop);
	}

	// The scroller mirrors the state (row 8.1): a key that moved the cursor set
	// the offset, and the element follows it. A user scroll already wrote the
	// offset back, so this assignment is a no-op for it.
	$effect(() => {
		const el = listEl;
		const top = $embyUi.scrollTop;
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

<div class="flex min-h-0 min-w-0 flex-1 flex-col" data-library-emby-list-column="">
	<!-- The Music list's header (design D15): the library's name, the carried
	selection's label, and the `/` filter field. -->
	<div class="flex flex-none flex-col">
		<LibraryListHeader
			title={emby.libraryName}
			label={selectionLabel}
			labelAttr="data-library-emby-selection-label"
			filter={$embyUi.filter}
			placeholder={filterPlaceholder}
			ariaLabel="Filter items"
			oninput={setEmbyFilter}
			onaccept={acceptEmbyFilter}
		/>
	</div>
	{#if emby.loading}
		<!-- The active list has not landed yet (row 8.1): one placeholder line
			holds the list's place. -->
		<LibraryListNotice text="Loading…" attr="data-library-emby-loading" />
	{:else if emby.failed}
		<!-- The plain mode's root-list fetch failed (row 8.1): the failure line
			naming what failed, and no row until the library opens again. -->
		<LibraryListNotice
			text="Couldn’t load this library."
			attr="data-library-emby-failed"
		/>
	{:else if emby.rows.length === 0}
		<!-- The library landed and holds no item (row 8.1): one line says so,
			as the Music list's empty line does. -->
		<LibraryListNotice text="No items" attr="data-library-emby-empty" />
	{/if}
	<div class="flex min-h-0 flex-1" style:overflow="hidden">
		<ol
			bind:this={listEl}
			class="min-h-0 flex-1"
			style:overflow-y="auto"
			style:list-style="none"
			style:margin="0"
			style:padding="8px 0 8px 8px"
			data-library-emby-list=""
			onscroll={onScroll}
		>
			<li style:height="{above}px" aria-hidden="true"></li>
			{#each painted as row (row.id)}
				<li>
					{#if row.kind === 'heading'}
						<div
							class="flex items-end font-mono"
							style:height="{row.height}px"
							style:padding="0 8px 2px"
							style:font-size="11px"
							style:color="var(--canvas-source)"
							data-library-emby-letter={row.letter}
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
							data-library-emby-row={row.row.kind}
							data-library-emby-row-id={row.id}
							data-library-emby-cursor={row.id === $embyUi.cursor ? '' : undefined}
							data-library-emby-selected={row.id === emby.selected ? '' : undefined}
							onclick={() => pressEmbyRow(row)}
							ondblclick={() => playEmbyRow(row.row.id)}
							oncontextmenu={(event) => rowMenu(event, row)}
						>
							<span class="flex min-w-0 flex-1 flex-col" style:gap="2px">
								<span
									class="truncate"
									style:color={canvasVar('text')}
									style:font-weight={row.id === emby.selected ? 600 : 400}
									data-library-emby-title="">{row.row.title}</span
								>
								{#if row.row.subtitle !== null}
									<span
										class="truncate"
										style:font-size="11px"
										style:color="var(--canvas-muted)"
										data-library-emby-subtitle="">{row.row.subtitle}</span
									>
								{/if}
							</span>
							{#if row.row.date !== null}
								<span
									class="flex-none tabular-nums"
									style:font-size="11px"
									style:color="var(--canvas-muted)"
									data-library-emby-date="">{row.row.date}</span
								>
							{/if}
						</button>
					{/if}
				</li>
			{/each}
			<li style:height="{below}px" aria-hidden="true"></li>
		</ol>
		{#if !emby.feedGroup}
			<LibraryLetterRail letters={rail} {marked} jump={jumpEmbyToLetter} />
		{/if}
	</div>
</div>
