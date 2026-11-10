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
	import { onMount } from 'svelte';
	import type { GuiHomeFrame } from '$lib/bridge/types';
	import {
		buildHomeRows,
		HOME_ITEM_ROW_HEIGHT,
		homeListWindow,
		homeRowsHeight,
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
	import LibraryFilterField from './LibraryFilterField.svelte';

	interface Props {
		/** The carried Home content (design D12): the flat list's rows. */
		home: GuiHomeFrame;
	}
	const { home }: Props = $props();

	let listEl: HTMLElement | undefined = $state();

	/** The painted rows of the flat list (design D12): the same model
	 * `$lib/library/home` builds from the carried content, narrowed by the
	 * filter's text. */
	const rows = $derived(buildHomeRows(home.rows, $homeUi.filter));

	/** The rows in view: only these paint. */
	const view = $derived(homeListWindow(rows, $homeUi.scrollTop, $homeUi.viewportHeight));
	const painted = $derived(rows.slice(view.start, view.end));

	/** The list's whole painted height, and the spacers that hold its scroll
	 * range while only the rows in view paint. */
	const total = $derived(homeRowsHeight(rows));
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

	/** An item row's name colour: the canvas's text tone, dimmed on a played
	 * row (design D12). */
	function nameColor(row: HomeListRow): string {
		if (row.kind === 'item' && row.state === 'played') {
			return canvasVar('muted');
		}
		return canvasVar('text');
	}

	/** An item row's painted fill: the cursor row's focus fill only. A
	 * highlight marks focus, so the active row keeps its own paint. */
	function rowBackground(row: HomeListRow): string | undefined {
		return row.kind === 'item' && row.id === $homeUi.cursor ? canvasVar('selected') : undefined;
	}

	/** The resume percent an active item row paints, or `null`. */
	function rowProgress(row: HomeListRow): number | null {
		return row.kind === 'item' && typeof row.state === 'object'
			? row.state.active.progress
			: null;
	}

	/** Report the list's measured height (design D12): the rows in view follow
	 * it. */
	function measure(): void {
		measureHomeList(listEl?.clientHeight ?? 0);
	}

	/** Report the list's native scroll offset, so the windowed paint follows a
	 * user scroll. */
	function onScroll(event: Event): void {
		scrollHomeList((event.currentTarget as HTMLElement).scrollTop);
	}

	// The scroller mirrors the state (design D12): a key that moved the cursor
	// set the offset, and the element follows it. A user scroll already wrote
	// the offset back, so this assignment is a no-op for it.
	$effect(() => {
		const el = listEl;
		const top = $homeUi.scrollTop;
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

<div class="flex min-h-0 min-w-0 flex-1 flex-col" data-library-home-list-column="">
	<div class="flex flex-none flex-col" style:padding="8px 16px 0">
		<LibraryFilterField
			value={$homeUi.filter}
			placeholder="Filter"
			ariaLabel="Filter home"
			oninput={setHomeFilter}
			onaccept={acceptHomeFilter}
		/>
	</div>
	<ol
		bind:this={listEl}
		class="min-h-0 flex-1"
		style:overflow-y="auto"
		style:list-style="none"
		style:margin="0"
		style:padding="8px 8px 8px 8px"
		data-library-home-list=""
		onscroll={onScroll}
	>
		<li style:height="{above}px" aria-hidden="true"></li>
		{#each painted as row (row.id)}
			<li>
				{#if row.kind === 'heading'}
					<div
						class="flex items-end"
						style:height="{row.height}px"
						style:padding="0 8px 4px"
						style:font-size="13px"
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
						style:font-size="13px"
						style:background={rowBackground(row)}
						style:cursor="pointer"
						data-library-home-row="item"
						data-library-home-row-id={row.id}
						data-library-home-cursor={row.id === $homeUi.cursor ? '' : undefined}
						onclick={() => pressHomeRow(row)}
						ondblclick={() => playHomeRow(row.rowId)}
					>
						{#if row.art !== null}
							<!-- The row's card art (design D12): the frame's data URL, or
								the art box while the cache still encodes it. The rows without
								art paint no box at all. -->
							<span
								class="flex flex-none items-center justify-center"
								style:width="40px"
								style:height="40px"
								style:background="var(--canvas-library-raise)"
								style:border-radius="4px"
								style:overflow="hidden"
								data-library-home-art={row.art.kind}
							>
								{#if row.art.kind !== 'loading'}
									<img class="h-full w-full object-cover" src={row.art.detail} alt="" />
								{/if}
							</span>
						{/if}
						<span class="flex min-w-0 flex-1 flex-col" style:gap="2px">
							<span
								class="truncate"
								style:color={nameColor(row)}
								data-library-home-title="">{row.title}</span
							>
							{#if row.subtitle !== null}
								<span
									class="truncate"
									style:font-size="11px"
									style:color="var(--canvas-muted)"
									data-library-home-subtitle="">{row.subtitle}</span
								>
							{/if}
						</span>
						{#if row.date !== null}
							<span
								class="flex-none tabular-nums"
								style:font-size="11px"
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
			</li>
		{/each}
		<li style:height="{below}px" aria-hidden="true"></li>
	</ol>
</div>
