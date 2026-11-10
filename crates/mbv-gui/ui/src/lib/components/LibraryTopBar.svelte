<script lang="ts">
	// The Library top bar (add-gui-library-panel design D2, D12): the canvas's
	// Home icon, then one tab per presented destination in the frame's catalog
	// order, with the active destination's tab marked current. The icon is
	// current while the Home content is present (the destinations are
	// exclusive), and a press on the icon or on a tab sends the
	// open-destination command with that destination's own position.
	//
	// The Search field (row 12.1) sits at the bar's right end. The typing
	// schedules the debounced `searchQuery` command through
	// `$lib/library/search`, the core carries the results back on the frame,
	// and the dropdown paints them in the list-row look — or its landing or
	// failed state — under the field. The keys that act on the results route
	// through `$lib/library/keys` while the field holds the keyboard, so this
	// component never interprets a key: it only follows the focus fact, and a
	// highlight id that a new results frame no longer carries falls to the
	// first row.
	import { roleVar } from '$lib/bridge/theme';
	import { sendLibraryCommand } from '$lib/bridge/library';
	import type { GuiLibraryTab, GuiSearchFrame, GuiSearchRow } from '$lib/bridge/types';
	import {
		activateSearchRow,
		blurSearchField,
		focusLibrarySearch,
		librarySearch,
		mountSearchField,
		setSearchHighlight,
		setSearchQuery,
		unmountSearchField,
	} from '$lib/library/search';

	interface Props {
		/** The presented destination tabs in catalog order (design D2). */
		tabs: GuiLibraryTab[];
		/** The active tab's position, or `null` while the active destination is
		 * not one of the tabs (design D2): Home, and every destination the GUI
		 * does not present yet. */
		currentTab: number | null;
		/** The carried Search facts while a search runs or its results hold
		 * (design D12, row 12.1); `null` while the section is closed. */
		search?: GuiSearchFrame | null;
		/** Whether the Home content is present (design D12): the Home icon
		 * marks itself current on it alone. */
		homeActive?: boolean;
	}
	const { tabs, currentTab, search = null, homeActive = false }: Props = $props();

	/** The current tab's run: the tab bar's underline role in both strokes. */
	const underline = roleVar('tabSelectedUnderline');

	/** Open the destination tab at `position` (design D2): Home's own position
	 * is 0, and the Home icon opens it. */
	function open(position: number): void {
		void sendLibraryCommand({ command: 'openDestination', position });
	}

	let searchEl: HTMLInputElement | undefined = $state();

	// The field's mount fact (row 12.1): the key router arms and routes the
	// field only while this top bar paints it, and the teardown retracts it —
	// an arrangement change can unmount the bar under an armed field, and the
	// focus fact must not outlive the element.
	$effect(() => {
		mountSearchField();
		return () => unmountSearchField();
	});

	// The field follows the focus fact into DOM focus and out of it (design
	// D3, step 2): `Ctrl+K` arms the fact in the key router, and a press
	// elsewhere retracts it through this same element's blur event.
	$effect(() => {
		const el = searchEl;
		if (el === undefined) {
			return;
		}
		const focused = $librarySearch.focused;
		if (focused && document.activeElement !== el) {
			el.focus();
		}
		if (!focused && document.activeElement === el) {
			el.blur();
		}
	});

	// A results frame keeps the held highlight by id (row 12.1): a first
	// landing and an id a new frame no longer carries — a new query's rows, or
	// a closed section's leftovers — fall to the first row. Rows compare by
	// id, never object identity, so a refresh keeps the held id.
	$effect(() => {
		const frame = search;
		const held = $librarySearch.highlight;
		if (frame === null) {
			return;
		}
		if (held !== null && frame.results.some((row: GuiSearchRow) => row.id === held)) {
			return;
		}
		setSearchHighlight(frame.results[0]?.id ?? null);
	});

	/** The highlighted row's painted fill (the list's cursor fill, design
	 * D12): the highlight marks focus, like the list's cursor. */
	function rowBackground(row: GuiSearchRow): string | undefined {
		return row.id === $librarySearch.highlight ? 'var(--canvas-selected)' : undefined;
	}
</script>

<nav
	class="flex flex-none items-center"
	aria-label="Libraries"
	data-library-region="topBar"
	style:height="56px"
	style:gap="2px"
	style:padding="0 12px 0 8px"
	style:border-bottom="1px solid var(--canvas-border)"
	style:background="var(--canvas-dim)"
>
	<!-- The canvas's Home icon, before the tabs (design D12): the current
		destination marker follows the tabs' `aria-current` rule while the Home
		content is present. -->
	<button
		type="button"
		class="flex flex-none items-center justify-center"
		style:width="44px"
		style:height="44px"
		style:color={homeActive ? 'var(--canvas-heading)' : 'var(--canvas-muted)'}
		style:cursor="pointer"
		aria-label="Home"
		aria-current={homeActive ? 'page' : undefined}
		data-library-home=""
		onclick={() => open(0)}
	>
		<!-- The canvas's Home glyph, verbatim. -->
		<svg
			width="18"
			height="18"
			viewBox="0 0 24 24"
			fill="none"
			stroke="currentColor"
			stroke-width="1.8"
			stroke-linejoin="round"
			aria-hidden="true"
		><path d="M3 11 12 4l9 7v9a1 1 0 0 1-1 1h-5v-6H9v6H4a1 1 0 0 1-1-1z" /></svg>
	</button>
	{#each tabs as tab (tab.position)}
		{@const current = tab.position === currentTab}
		<button
			type="button"
			class="flex items-center uppercase"
			style:height="56px"
			style:padding="0 12px"
			style:font-size="13px"
			style:font-weight={current ? 700 : 500}
			style:letter-spacing="0.06em"
			style:color={current ? 'var(--canvas-heading)' : 'var(--canvas-muted)'}
			aria-current={current ? 'page' : undefined}
			data-library-tab={tab.position}
			onclick={() => open(tab.position)}
		>
			{#if current}
				<!-- The current tab's run: the canvas's two underline strokes around
					the label. -->
				<span
					class="block"
					style:border-top="2px solid {underline}"
					style:border-bottom="2px solid {underline}"
					style:padding="3px 1px 2px"
					style:line-height="1">{tab.label}</span
				>
			{:else}
				{tab.label}
			{/if}
		</button>
	{/each}
	<!-- The Search field at the bar's right end (row 12.1). The key router owns
		every key while it holds the keyboard, so the field never answers a key
		itself. -->
	<div
		class="relative flex-none"
		style:margin-left="auto"
		style:width="220px"
		data-library-search=""
	>
		<input
			bind:this={searchEl}
			type="text"
			class="w-full"
			style:height="34px"
			style:padding="0 10px"
			style:box-sizing="border-box"
			style:background="var(--canvas-library-column)"
			style:border="1px solid var(--canvas-border)"
			style:border-radius="6px"
			style:color="var(--canvas-heading)"
			style:font-size="13px"
			style:outline="none"
			placeholder="Search"
			aria-label="Search"
			value={$librarySearch.query}
			data-library-search-input=""
			onfocus={() => focusLibrarySearch()}
			onblur={() => blurSearchField()}
			oninput={(event) => setSearchQuery(event.currentTarget.value)}
		/>
		{#if search}
			<!-- The results dropdown, in the list-row look (row 12.1): the rows
				under the field while the frame carries them, and the loading and
				failed states while the query lands. -->
			<div
				class="absolute"
				style:left="0"
				style:right="0"
				style:top="calc(100% + 4px)"
				style:max-height="320px"
				style:overflow-y="auto"
				style:background="var(--canvas-library-panel)"
				style:border="1px solid var(--canvas-border)"
				style:border-radius="6px"
				style:padding="2px 0"
				style:z-index="30"
				data-library-search-results=""
				role="listbox"
			>
				{#if search.loading}
					<div
						class="font-mono"
						style:padding="8px 10px"
						style:font-size="11px"
						style:color="var(--canvas-muted)"
						data-library-search-loading=""
					>
						Searching…
					</div>
				{:else if search.failed}
					<div
						class="font-mono"
						style:padding="8px 10px"
						style:font-size="11px"
						style:color="var(--canvas-muted)"
						data-library-search-failed=""
					>
						Search failed
					</div>
				{:else if search.results.length === 0}
					<div
						class="font-mono"
						style:padding="8px 10px"
						style:font-size="11px"
						style:color="var(--canvas-muted)"
						data-library-search-empty=""
					>
						No results
					</div>
				{:else}
					{#each search.results as row (row.id)}
						<button
							type="button"
							class="flex w-full items-center overflow-hidden"
							style:height="36px"
							style:padding="0 10px"
							style:gap="8px"
							style:border="0"
							style:text-align="left"
							style:font-size="13px"
							style:background={rowBackground(row)}
							style:cursor="pointer"
							data-library-search-row={row.id}
							data-library-search-highlight={row.id === $librarySearch.highlight ? '' : undefined}
							onclick={() => activateSearchRow(row)}
						>
							<span
								class="min-w-0 flex-1 truncate"
								style:color="var(--canvas-text)"
							>{row.title}</span
							>
							<span
								class="font-mono"
								style:font-size="11px"
								style:color="var(--canvas-muted)"
								style:max-width="45%"
								style:flex="0 1 auto"
								style:overflow="hidden"
								style:text-overflow="ellipsis"
								style:white-space="nowrap"
							>{row.subtitle}</span
							>
						</button>
					{/each}
				{/if}
			</div>
		{/if}
	</div>
</nav>
