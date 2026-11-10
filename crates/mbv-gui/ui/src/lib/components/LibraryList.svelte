<script lang="ts">
	// The Library list (add-gui-library-panel design D7): the canvas's artist
	// index in the list column — the *Artists* line over the selected
	// collection names (design D6), the `/` filter field, the canvas tree of
	// artist, album, and track rows under its letter headings, and the A–Z
	// letter rail beside it. The rows paint at the canvas's fixed heights over
	// native scroll, and only the rows in view paint, so a long artist list
	// stays fast.
	//
	// The cursor, the open nodes, the scroll offset, and the filter text are
	// window-content state in `$lib/library/state` (design D1), and the row
	// model is `$lib/library/list`'s. This component paints that state and
	// reports the two facts only the DOM holds — its measured viewport height
	// and its native scroll offset. A press picks and opens: an artist or album
	// row opens or closes and picks, an album without a cached tracklist asks
	// the core to load it, and a track picks its album. A press moves no
	// cursor; the design's keys do that.
	//
	// The filter field is the panel's open text field (design D3, step 2): the
	// key router types into it without checking focus, so this field owns its
	// focus. It takes the keyboard while the filter is open and closes the
	// filter when it loses focus, keeping its text as Enter does.
	import { onMount } from 'svelte';
	import { ChevronRight } from '@lucide/svelte';
	import type { GuiMusicFrame } from '$lib/bridge/types';
	import {
		LETTER_ROW_HEIGHT,
		NODE_ROW_HEIGHT,
		TRACK_ROW_HEIGHT,
		buildLibraryRows,
		letterRail,
		listWindow,
		rowsHeight,
	} from '$lib/library/list';
	import type { LibraryRow } from '$lib/library/list';
	import {
		acceptLibraryFilter,
		playLibraryTrack,
		libraryUi,
		measureLibraryList,
		openLibraryRowMenu,
		pressLibraryRow,
		revealPickedAlbum,
		scrollLibraryList,
		setLibraryFilter,
	} from '$lib/library/state';
	import LibraryListHeader from './LibraryListHeader.svelte';
	import LibraryListNotice from './LibraryListNotice.svelte';
	import LibraryLetterRail from './LibraryLetterRail.svelte';
	import { canvasVar } from '$lib/bridge/theme';

	interface Props {
		/** The carried Music content: the union the list paints and the pills
		 * whose names the header line reads (design D2, D6). */
		music: GuiMusicFrame;
	}
	const { music }: Props = $props();

	let listEl: HTMLElement | undefined = $state();

	/** The album the list last revealed. A pick reveals once, so a later frame
	 * carrying the same pick does not move the cursor again. */
	let revealedAlbum: string | null = null;
	$effect(() => {
		const albumId = music.hero?.album.albumId ?? null;
		if (albumId === null || albumId === revealedAlbum) {
			return;
		}
		revealedAlbum = albumId;
		revealPickedAlbum(music);
	});

	/** The painted rows of the canvas tree (design D7): the same model
	 * `$lib/library/state` builds from the carried content, over the open nodes
	 * and the filter this component paints. */
	const rows = $derived(
		buildLibraryRows({
			artists: music.artists,
			open: $libraryUi.openNodes,
			closed: $libraryUi.filterClosed,
			filter: $libraryUi.filter,
		}),
	);

	/** The rows in view: only these paint. */
	const view = $derived(listWindow(rows, $libraryUi.scrollTop, $libraryUi.viewportHeight));
	const painted = $derived(rows.slice(view.start, view.end));

	/** The list's whole painted height, and the spacers that hold its scroll
	 * range while only the rows in view paint. */
	const total = $derived(rowsHeight(rows));
	const above = $derived(painted.length > 0 ? painted[0].top : 0);
	const below = $derived(total - (painted.length > 0 ? lastBottom() : above));

	/** The selected collection names in pill order (design D6): the core's one
	 * derivation, carried on the Music frame so the list header and the Status
	 * bar location cannot disagree. An empty selection is the whole library:
	 * the header names it `All` instead of an empty label. */
	const selectedNames = $derived(
		music.collectionNames.length > 0 ? music.collectionNames : ['All'],
	);

	/** The header's right line: the canvas's selection label — the names joined
	 * by ` + ` for several, one name with its sort rule for one, and `All` for
	 * the whole-library empty selection. */
	const selectionLabel = $derived(
		selectedNames.length > 1
			? selectedNames.join(' + ')
			: selectedNames.length === 1 && music.collectionNames.length > 0
				? `${selectedNames[0]} · sort name`
				: 'All',
	);

	/** The filter field's placeholder, naming the selection as the canvas
	 * does. */
	const filterPlaceholder = $derived(
		selectedNames.length > 0 ? `Filter ${selectedNames.join(' + ')}` : 'Filter',
	);

	/** The rail's A–Z letters over the painted rows. */
	const rail = $derived(letterRail(rows));

	/** The letter the rail marks: the one the cursor row belongs to. */
	const marked = $derived(
		rows.find((row) => row.id === $libraryUi.cursor)?.letter ?? null,
	);

	/** The album the Hero shows: the list paints its row in the canvas's picked
	 * fill (design D7). */
	const picked = $derived(music.hero?.album.albumId ?? null);

	/** The bottom edge of the last painted row: the below-spacer's pair. */
	function lastBottom(): number {
		const last = painted[painted.length - 1];
		return last === undefined ? 0 : last.top + last.height;
	}

	/** The stable Service id a row's menu names: an artist's Service item, an
	 * album's own id, and a track's id — the same id the row's actions send. */
	function rowMenuItemId(row: LibraryRow): string | null {
		switch (row.kind) {
			case 'letter':
				return null;
			case 'artist':
				return row.itemId;
			case 'album':
				return row.albumId;
			case 'track':
				return row.trackId;
		}
	}

	/** Open the row's right-click item menu (the row-menu command): the next
	 * frame carries the item's full entries, and the menu paints at the
	 * pointer. A letter heading names no item, so its menu opens nothing. */
	function rowMenu(event: MouseEvent, row: LibraryRow): void {
		event.preventDefault();
		const itemId = rowMenuItemId(row);
		if (itemId === null) {
			return;
		}
		openLibraryRowMenu(itemId, event.clientX, event.clientY);
	}


	/** Whether the row is the album the Hero shows. */
	function isPicked(row: LibraryRow): boolean {
		return row.kind === 'album' && row.albumId === picked;
	}

	/** A row's painted fill: the cursor row's focus fill only. A highlight marks
	 * focus, so the playing track and the picked album keep their own fill
	 * (design D7, D8). */
	function rowBackground(row: LibraryRow): string | undefined {
		return row.id === $libraryUi.cursor ? canvasVar('selected') : undefined;
	}

	/** A row's name colour: the canvas's tone by depth — the artist heading,
	 * the album text, the muted track. */
	function rowColor(row: LibraryRow): string {
		return row.kind === 'artist'
			? canvasVar('heading')
			: row.kind === 'album'
				? canvasVar('text')
				: canvasVar('muted');
	}

	/** A row's name weight: an artist row is bold, an album and a track regular,
	 * as the canvas draws them. */
	function rowWeight(row: LibraryRow): number {
		return row.kind === 'artist' ? 600 : 400;
	}

	/** A row's indent: the canvas's tree inset by depth. */
	function rowPadding(row: LibraryRow): string {
		if (row.kind === 'artist') {
			return '0 8px 0 4px';
		}
		return row.kind === 'album' ? '0 8px 0 22px' : '0 8px 0 40px';
	}

	/** Report the list's measured height (design D7): the rows in view follow
	 * it. */
	function measure(): void {
		measureLibraryList(listEl?.clientHeight ?? 0);
	}

	/** Report the list's native scroll offset, so the windowed paint follows a
	 * user scroll. */
	function onScroll(event: Event): void {
		scrollLibraryList((event.currentTarget as HTMLElement).scrollTop);
	}

	// The scroller mirrors the state (design D7): a key that moved the cursor
	// set the offset, and the element follows it. A user scroll already wrote
	// the offset back, so this assignment is a no-op for it.
	$effect(() => {
		const el = listEl;
		const top = $libraryUi.scrollTop;
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

<div class="flex min-h-0 min-w-0 flex-1 flex-col" data-library-list-column="">
	<div class="flex flex-none flex-col">
		<LibraryListHeader
			title="Artists"
			label={selectionLabel}
			filter={$libraryUi.filter}
			placeholder={filterPlaceholder}
			ariaLabel="Filter artists"
			oninput={setLibraryFilter}
			onaccept={acceptLibraryFilter}
		/>
	</div>
	{#if music.loading}
		<!-- A selected group's album list has not landed yet (design D5): one
			placeholder line holds the list's place. -->
		<LibraryListNotice text="Loading…" attr="data-library-loading" />
	{/if}
	{#if music.failure !== null}
		<!-- The selected groups whose album list failed (design D5): the
			frame's failure line names them. -->
		<LibraryListNotice text={music.failure} attr="data-library-failure" />
	{/if}
	{#if !music.loading && music.artists.length === 0}
		<!-- The selected collections hold no artist (the spec's empty text):
			the row area is empty, so one line says so. -->
		<LibraryListNotice text="No artists" attr="data-library-empty" />
	{/if}
	<div class="flex min-h-0 flex-1" style:overflow="hidden">
		<ol
			bind:this={listEl}
			class="min-h-0 flex-1"
			style:overflow-y="auto"
			style:list-style="none"
			style:margin="0"
			style:padding="0 0 0 8px"
			data-library-list=""
			onscroll={onScroll}
		>
			<li style:height="{above}px" aria-hidden="true"></li>
			{#each painted as row (row.id)}
				<li>
					{#if row.kind === 'letter'}
						<div
							class="flex items-end font-mono"
							style:height="{LETTER_ROW_HEIGHT}px"
							style:padding="0 8px 2px"
							style:font-size="11px"
							style:color="var(--canvas-source)"
							data-library-letter={row.letter}
						>
							{row.letter}
						</div>
					{:else if row.kind === 'track'}
						<button
							type="button"
							class="flex w-full items-center overflow-hidden"
							style:height="{TRACK_ROW_HEIGHT}px"
							style:padding={rowPadding(row)}
							style:gap="8px"
							style:border="0"
							style:border-radius={isPicked(row) ? '4px' : '0'}
							style:text-align="left"
							style:font-size="13px"
							style:background={rowBackground(row)}
										style:cursor="pointer"
										data-library-row={row.kind}
										data-library-row-id={row.id}
										data-library-cursor={row.id === $libraryUi.cursor ? '' : undefined}
										onclick={() => pressLibraryRow(row)}
						oncontextmenu={(event) => rowMenu(event, row)}
					ondblclick={() => playLibraryTrack(row.trackId)}
									>
										<span
											class="flex flex-none items-center justify-end font-mono tabular-nums"
											style:width="18px"
											style:font-size="11px"
											style:color="var(--canvas-ordinal)">{row.number}</span
										>
							<span
								class="min-w-0 flex-1 truncate"
								style:color={rowColor(row)}>{row.name}</span
							>
							<span
								class="flex-none font-mono tabular-nums"
								style:font-size="11px"
								style:color="var(--canvas-muted)">{row.duration}</span
							>
						</button>
					{:else}
						<button
							type="button"
							class="flex w-full items-center overflow-hidden"
							style:height="{NODE_ROW_HEIGHT}px"
							style:padding={rowPadding(row)}
							style:gap="8px"
							style:border="0"
							style:border-radius={isPicked(row) ? '4px' : '0'}
							style:text-align="left"
							style:font-size="13.5px"
							style:background={rowBackground(row)}
							style:cursor="pointer"
							aria-expanded={row.open}
							data-library-row={row.kind}
							data-library-row-id={row.id}
							data-library-cursor={row.id === $libraryUi.cursor ? '' : undefined}
							onclick={() => pressLibraryRow(row)}
							oncontextmenu={(event) => rowMenu(event, row)}
						>
							<span
								class="flex flex-none items-center justify-center"
								style:width="18px"
								style:color={rowColor(row)}
							>
								<span
									class="flex"
									style:transform="rotate({row.open ? 90 : 0}deg)"
									style:transition="transform 120ms ease"
								>
									<ChevronRight size={10} strokeWidth={3} />
								</span>
							</span>
							<span
								class="min-w-0 flex-1 truncate"
								style:color={rowColor(row)}
								style:font-weight={rowWeight(row)}>{row.name}</span
							>
							{#if row.kind === 'album' && row.year !== ''}
								<span
									class="flex-none font-mono tabular-nums"
									style:font-size="11px"
									style:color="var(--canvas-muted)">{row.year}</span
								>
							{/if}
						</button>
					{/if}
				</li>
			{/each}
			<li style:height="{below}px" aria-hidden="true"></li>
		</ol>
		<LibraryLetterRail letters={rail} {marked} />
	</div>
</div>
