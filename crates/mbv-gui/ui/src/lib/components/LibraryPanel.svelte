<script lang="ts">
	// The Library panel (add-gui-library-panel design D4, D12): the column
	// beside the Queue panel. It paints the shell and the arrangement's
	// regions — the top bar above the list, and, in Wide, the Hero pane beside
	// the list; in Narrow the Hero mounts as an overlay over the list once an
	// item is picked. The active destination decides which painters mount
	// (design D12): Music paints the pill row, the union list, and the album
	// Hero; Home paints the flat list and its Hero, with no pill row, no
	// letter rail, and no Workspace; Shows paints its mode pills, the series or
	// flat list with the All series mode's rail, and the series Hero with its
	// Workspace; a plain Emby library paints its feed-group pills, its list with
	// the plain mode's rail, and its Hero with no Workspace; Audiobooks paints
	// the book list with its letter rail and the book Hero with its chapters
	// Workspace (row 9.1). The Status bar (row 4.8) closes the column under
	// both. The
	// panel's interaction state lives in `$lib/library/state`,
	// `$lib/library/home`, and `$lib/library/shows`, and its keys in
	// `$lib/library/keys`, so this shell holds no state of its own.
	//
	// The panel reads the `libraryList`, `libraryDetail`, and `rowMenu` sections
	// (gui-change-driven-sections design D8). Each destination's painters take
	// the list part and the detail part they read, narrowed together by
	// `libraryPair`.
	//
	// A press inside the panel sends the take-focus command (design D4): the
	// core sets the focus stack and Panel focus together, and the press that
	// follows acts on the row. The panel paints through the GUI-local
	// `--canvas-*` tokens of `app.css`; `mbv-theme` gains no Library slot.
	//
	// The Narrow Hero overlay covers the list, so it must be dismissible: its
	// backdrop press closes it, and Esc closes it in the key router (design
	// D3). The panel also reports the arrangement it paints, so the router
	// closes an overlay only where one actually paints.
	import type { GuiArrangement } from '$lib/bridge/arrangement';
	import { sendLibraryCommand } from '$lib/bridge/library';
	import { dragListWidth, resetListWidth } from '$lib/bridge/resize';
	import {
		libraryDetail,
		libraryList,
		rowMenu,
		search as searchSection,
		status as statusSection,
	} from '$lib/bridge/sections';
	import LibraryAudiobooksHero from './LibraryAudiobooksHero.svelte';
	import LibraryAudiobooksList from './LibraryAudiobooksList.svelte';
	import LibraryEmbyHero from '$lib/components/LibraryEmbyHero.svelte';
	import LibraryEmbyList from '$lib/components/LibraryEmbyList.svelte';
	import LibraryEmbyPills from '$lib/components/LibraryEmbyPills.svelte';
	import LibraryFeedsHero from '$lib/components/LibraryFeedsHero.svelte';
	import LibraryFeedsList from '$lib/components/LibraryFeedsList.svelte';
	import LibraryFeedsPills from '$lib/components/LibraryFeedsPills.svelte';
	import LibraryHero from '$lib/components/LibraryHero.svelte';
	import LibraryHomeHero from '$lib/components/LibraryHomeHero.svelte';
	import LibraryHomeList from '$lib/components/LibraryHomeList.svelte';
	import LibraryList from '$lib/components/LibraryList.svelte';
	import LibraryPills from '$lib/components/LibraryPills.svelte';
	import LibraryPodcastsHero from '$lib/components/LibraryPodcastsHero.svelte';
	import LibraryPodcastsList from '$lib/components/LibraryPodcastsList.svelte';
	import LibraryPodcastsPills from '$lib/components/LibraryPodcastsPills.svelte';
	import LibraryShowsHero from '$lib/components/LibraryShowsHero.svelte';
	import LibraryShowsList from '$lib/components/LibraryShowsList.svelte';
	import LibraryShowsPills from '$lib/components/LibraryShowsPills.svelte';
	import LibraryStatusBar from '$lib/components/LibraryStatusBar.svelte';
	import LibraryTopBar from '$lib/components/LibraryTopBar.svelte';
	import PanelDivider from '$lib/components/PanelDivider.svelte';
	import QueueRowMenu from '$lib/components/QueueRowMenu.svelte';
	import { libraryPair } from '$lib/library/parts';
	import {
		closeLibraryHero,
		closeLibraryRowMenu,
		libraryUi,
		reportLibraryArrangement,
		runLibraryRowMenuEntry,
	} from '$lib/library/state';

	interface Props {
		/** The arrangement picked from the Library column width (design D4). */
		arrangement: GuiArrangement;
		/** The list column's painted width (design D4): the session's dragged
		 * width, or its default. */
		listWidth: number;
		/** The Library column the list column sits in: the drag's clamp. */
		libraryColumn: number;
	}
	const { arrangement, listWidth, libraryColumn }: Props = $props();

	/** The presented destination tabs and the active one (design D2): the
	 * top bar paints them. */
	const tabs = $derived($libraryList?.tabs ?? []);
	/** The active tab's position, or `null` while the active destination is
	 * not one of the tabs. */
	const currentTab = $derived($libraryList?.currentTab ?? null);
	/** The active destination's list part (design D8); `null` while no Library
	 * panel shows. */
	const list = $derived($libraryList?.destination ?? null);
	/** The active destination's list and detail parts, narrowed together on the
	 * destination's kind (design D8); `null` while no Library panel shows. */
	const pair = $derived(libraryPair(list, $libraryDetail));
	/** The Search section's results (design D12, row 12.1): the results
	 * dropdown the top bar paints under its field; `null` while the section
	 * is closed. */
	const search = $derived($searchSection);
	/** The right-click item menu the window last opened (design D8, D15),
	 * whichever destination's row named it; `null` while no right-click has
	 * named an item. */
	const menu = $derived($rowMenu);
	/** The Status bar section's facts (design D10): the armed prefix and the
	 * live toast, which the Status bar replaces the hints with. */
	const status = $derived($statusSection);

	/** Whether the Hero sits beside the list rather than over it. */
	const wide = $derived(arrangement === 'wide');

	/** The Narrow Hero overlay's facts: the active destination's pair while its
	 * detail paints a Hero (design D4, D12). */
	const overlayHero = $derived(pair !== null && pair.detail.hero !== null ? pair : null);

	let panelEl: HTMLElement | undefined = $state();

	// The press that gives the panel Panel focus (design D4). The listener sits
	// on the panel element rather than in an `onpointerdown` attribute because
	// the shell is a plain container, not a control; the shared input forwarder
	// still receives the same press at the window.
	$effect(() => {
		const el = panelEl;
		if (!el) {
			return;
		}
		const takeFocus = () => void sendLibraryCommand({ command: 'takeFocus' });
		el.addEventListener('pointerdown', takeFocus);
		return () => el.removeEventListener('pointerdown', takeFocus);
	});

	// Report the arrangement the panel paints (design D4): the key router closes
	// the Narrow Hero overlay with Esc only while one paints.
	$effect(() => {
		reportLibraryArrangement(arrangement);
	});

	// The right-click item menu dismisses on a press outside it and on any
	// scroll (the menu is viewport-anchored, so its helper row would drift
	// under a scrolled list). The outside-press rule reads the press target, so
	// a press on the menu's own entries runs the entry instead of closing —
	// including the right-press that opens the menu over another row, which
	// closes this one just before the context-menu event opens the next.
	$effect(() => {
		if ($libraryUi.rowMenu === null) {
			return;
		}
		const outsidePress = (event: PointerEvent) => {
			const target = event.target;
			if (target instanceof Element && target.closest('[data-row-menu]')) {
				return;
			}
			closeLibraryRowMenu();
		};
		const scrolled = () => closeLibraryRowMenu();
		window.addEventListener('pointerdown', outsidePress);
		window.addEventListener('scroll', scrolled, { capture: true });
		return () => {
			window.removeEventListener('pointerdown', outsidePress);
			window.removeEventListener('scroll', scrolled, { capture: true });
		};
	});
</script>

<div
	bind:this={panelEl}
	class="relative flex h-full min-h-0 min-w-0 flex-1 flex-col"
	style:background="var(--canvas-library-column)"
	data-library-panel=""
	data-library-arrangement={arrangement}
>
	<LibraryTopBar {tabs} {currentTab} {search} homeActive={pair?.kind === 'home'} />
	{#if pair?.kind === 'music'}
		<!-- The Music destination's pills (design D6): Home paints none
			(design D12), so the row is absent on it, not collapsed. -->
		<LibraryPills list={pair.list} />
	{:else if pair?.kind === 'shows'}
		<!-- The Shows destination's mode pills (design D12, row 7.1): single
			select over the three view modes. -->
		<LibraryShowsPills list={pair.list} />
	{:else if pair?.kind === 'emby' && pair.list.feedGroup}
		<!-- The feed-group Emby library's pills (design D12, row 8.1): All and
			each root folder that holds items. A plain library paints no pill row at
			all — its letter rail replaces the TUI's letter-range pill row. -->
		<LibraryEmbyPills list={pair.list} />
	{:else if pair?.kind === 'podcasts'}
		<!-- The Podcasts destination's episode pills (design D12, row 10.1):
			single select over Latest and each show. -->
		<LibraryPodcastsPills list={pair.list} />
	{:else if pair?.kind === 'feeds'}
		<!-- The Feeds destination's view pills (design D12, row 11.1):
			single select over Latest and each subscription. -->
		<LibraryFeedsPills list={pair.list} />
	{/if}
	<div class="relative flex min-h-0 flex-1">
		{#if wide}
			<!-- The Wide arrangement's Hero pane, to the left of the list as the
				canvas orders it, with the active destination's Hero inside. -->
			<div
				data-library-region="hero"
				class="flex min-h-0 min-w-0 flex-1 flex-col"
				style:background="var(--canvas-library-column)"
				style:overflow-y="auto"
			>
				{#if pair?.kind === 'music'}
					{#if pair.detail.hero}
						<LibraryHero list={pair.list} detail={pair.detail} />
					{/if}
				{:else if pair?.kind === 'home'}
					{#if pair.detail.hero !== null}
						<LibraryHomeHero detail={pair.detail} />
					{/if}
				{:else if pair?.kind === 'shows'}
					<!-- The series Hero with its Workspace (design D12, row 7.1): the
						Upcoming mode carries no Hero, so the pane stays empty there. -->
					{#if pair.detail.hero !== null}
						<LibraryShowsHero list={pair.list} detail={pair.detail} />
					{/if}
				{:else if pair?.kind === 'emby'}
					<!-- The Emby library's Hero with no Workspace (design D12, row 8.1):
						a frame without one leaves the pane empty. -->
					{#if pair.detail.hero !== null}
						<LibraryEmbyHero list={pair.list} detail={pair.detail} />
					{/if}
				{:else if pair?.kind === 'audiobooks'}
					<!-- The book Hero with its chapters Workspace (design D12, row
						9.1): a frame without one leaves the pane empty. -->
					{#if pair.detail.hero !== null}
						<LibraryAudiobooksHero list={pair.list} detail={pair.detail} />
					{/if}
				{:else if pair?.kind === 'podcasts'}
					<!-- The episode Hero with no Workspace (design D12, row 10.1):
						a frame without one leaves the pane empty. -->
					{#if pair.detail.hero !== null}
						<LibraryPodcastsHero list={pair.list} detail={pair.detail} />
					{/if}
				{:else if pair?.kind === 'feeds'}
					<!-- The entry Hero with no Workspace (design D12, row 11.1):
						a frame without one leaves the pane empty. -->
					{#if pair.detail.hero !== null}
						<LibraryFeedsHero list={pair.list} detail={pair.detail} />
					{/if}
				{/if}
			</div>
		{/if}
		<!-- The list column: the canvas's index in Wide, the whole Library
			column in Mini and Narrow. Row 4.5 paints the Music tree and the
			letter rail here, and row 6.2 paints Home's flat list. Wide paints
			the session's width (design D4), and its divider sits between the
			list and the Hero pane. -->
		{#if wide}
			<PanelDivider
				side="right"
				name="list"
				width={listWidth}
				onDrag={(width) => dragListWidth(width, libraryColumn)}
				onReset={resetListWidth}
			/>
		{/if}
		<div
			data-library-region="list"
			class="flex min-h-0 flex-col {wide ? 'flex-none border-l' : 'min-w-0 flex-1'}"
			style:width={wide ? `${listWidth}px` : undefined}
			style:background="var(--canvas-library-panel)"
			style:border-color="var(--canvas-border)"
		>
			{#if pair?.kind === 'music'}
				<LibraryList list={pair.list} detail={pair.detail} />
			{:else if pair?.kind === 'home'}
				<LibraryHomeList list={pair.list} detail={pair.detail} />
			{:else if pair?.kind === 'shows'}
				<LibraryShowsList list={pair.list} detail={pair.detail} />
			{:else if pair?.kind === 'emby'}
				<LibraryEmbyList list={pair.list} detail={pair.detail} />
			{:else if pair?.kind === 'audiobooks'}
				<LibraryAudiobooksList list={pair.list} detail={pair.detail} />
			{:else if pair?.kind === 'podcasts'}
				<LibraryPodcastsList list={pair.list} detail={pair.detail} />
			{:else if pair?.kind === 'feeds'}
				<LibraryFeedsList list={pair.list} detail={pair.detail} />
			{/if}
		</div>
		{#if arrangement === 'narrow' && overlayHero !== null && !$libraryUi.heroDismissed}
			<!-- The Narrow Hero overlay (design D4): picking an item paints the
				Hero over the list, through the same painters the Wide pane uses, so
				the two cannot drift. The backdrop press and Esc close it (design
				D3), so a picked item never leaves an overlay no input dismisses;
				the card scrolls its own content and swallows the presses that land
				on it. -->
			<div
				data-library-region="heroOverlay"
				class="absolute inset-0 z-10 flex"
				style:background="rgba(0, 0, 0, 0.45)"
			>
				<button
					type="button"
					class="absolute inset-0"
					style:background="none"
					style:border="0"
					aria-label="Close the Hero"
					data-library-hero-backdrop=""
					onclick={closeLibraryHero}
				></button>
				<div
					class="relative m-3 flex min-h-0 min-w-0 flex-1 flex-col overflow-y-auto"
					style:background="var(--canvas-library-column)"
					style:border="1px solid var(--canvas-border)"
					style:border-radius="8px"
					data-library-hero-card=""
				>
					{#if overlayHero.kind === 'music'}
						<LibraryHero list={overlayHero.list} detail={overlayHero.detail} />
					{:else if overlayHero.kind === 'home'}
						<LibraryHomeHero detail={overlayHero.detail} />
					{:else if overlayHero.kind === 'shows'}
						<LibraryShowsHero list={overlayHero.list} detail={overlayHero.detail} />
					{:else if overlayHero.kind === 'audiobooks'}
						<LibraryAudiobooksHero list={overlayHero.list} detail={overlayHero.detail} />
					{:else if overlayHero.kind === 'podcasts'}
						<LibraryPodcastsHero list={overlayHero.list} detail={overlayHero.detail} />
					{:else if overlayHero.kind === 'feeds'}
						<LibraryFeedsHero list={overlayHero.list} detail={overlayHero.detail} />
					{:else}
						<LibraryEmbyHero list={overlayHero.list} detail={overlayHero.detail} />
					{/if}
				</div>
			</div>
		{/if}
	</div>
	<!-- The Library Status bar (design D10): under the list region, closing the
		column with the canvas's fixed key hints and the location while the panel
		paints. -->
	<LibraryStatusBar {tabs} {currentTab} {list} detail={$libraryDetail} {status} />
	<!-- The right-click item menu (the row-menu command): the viewport-anchored
		card at the pointer position, painted while the content carries the
		named item's entries. It belongs to the Library, not to one destination
		(design D15), so a row of any destination opens it. A press runs the
		entry the core built, resolved by that item's stable id. -->
	{#if menu !== null && $libraryUi.rowMenu !== null}
		{#if menu.itemId === $libraryUi.rowMenu.itemId && menu.entries.length > 0}
			<div class="fixed" style:left="0" style:top="0" style:width="0" style:height="0">
				<QueueRowMenu
					cursor={-1}
					entries={menu.entries}
					top="{$libraryUi.rowMenu.y}px"
					left="{$libraryUi.rowMenu.x}px"
					onSelect={runLibraryRowMenuEntry}
				/>
			</div>
		{/if}
	{/if}
</div>
