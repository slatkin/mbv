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
import type {
GuiAudiobooksFrame,
GuiEmbyFrame,
GuiFeedsFrame,
GuiHomeFrame,
GuiLibraryTab,
GuiMusicFrame,
GuiPodcastsFrame,
GuiSearchFrame,
GuiShowsFrame,
GuiStatusBar,
} from '$lib/bridge/types';
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
	import {
		closeLibraryHero,
		closeLibraryRowMenu,
		libraryUi,
		reportLibraryArrangement,
		runLibraryRowMenuEntry,
	} from '$lib/library/state';

	interface Props {
		/** The presented destination tabs and the active one (design D2): the
		 * top bar paints them. */
		tabs: GuiLibraryTab[];
		currentTab: number | null;
		/** The carried Music content (design D2): the picked collections, the
		 * union list, and the Hero; `null` while another destination is
		 * active. */
		music: GuiMusicFrame | null;
		/** The carried Home content (design D12): the flat list and the
		 * Hero; `null` while another destination is active. */
		home?: GuiHomeFrame | null;
		/** The carried Shows content (design D12, row 7.1): the mode pills, the
		 * active mode's list, and the selected row's Hero with its Workspace;
		 * `null` while another destination is active. */
		shows?: GuiShowsFrame | null;
		/** The carried plain Emby-library content (design D12, row 8.1): the
		 * feed-group mode's pills, the active list, and the selected row's Hero
		 * with no Workspace; `null` while another destination is active. */
		emby?: GuiEmbyFrame | null;
		/** The carried Audiobooks content (design D12, row 9.1): the catalog's
		 * book list and the selected book's Hero with its chapters Workspace;
		 * `null` while another destination is active. */
		audiobooks?: GuiAudiobooksFrame | null;
		/** The carried Podcasts content (design D12, row 10.1): the episode
		 * pills, the active view's date-gutter list, and the selected episode's
		 * Hero with no Workspace; `null` while another destination is active. */
		podcasts?: GuiPodcastsFrame | null;
		/** The carried Feeds content (design D12, row 11.1): the view pills,
		 * the active pill's date-gutter list, and the selected entry's Hero
		 * with no Workspace; `null` while another destination is active. */
		feeds?: GuiFeedsFrame | null;
		/** The carried Search facts (design D12, row 12.1): the results dropdown
		 * the top bar paints under its field; `null` while the section is
		 * closed. */
		search?: GuiSearchFrame | null;
		/** The frame's Status bar facts (design D10): the armed prefix and the
		 * live toast, which the Status bar replaces the hints with. */
		status: GuiStatusBar;
		/** The arrangement picked from the Library column width (design D4). */
		arrangement: GuiArrangement;
		/** The list column's painted width (design D4): the session's dragged
		 * width, or its default. */
		listWidth: number;
		/** The Library column the list column sits in: the drag's clamp. */
		libraryColumn: number;
	}
	const {
		tabs,
		currentTab,
		music,
		home = null,
		shows = null,
		emby = null,
		audiobooks = null,
		podcasts = null,
		feeds = null,
		search = null,
		status,
		arrangement,
		listWidth,
		libraryColumn,
	}: Props = $props();

	/** Whether the Hero sits beside the list rather than over it. */
	const wide = $derived(arrangement === 'wide');

	/** The Narrow Hero overlay's facts, whichever destination carries them:
	 * Music's picked album, the Home frame's selected row, the Shows frame's
	 * selected row with its Workspace, or a plain Emby library's selected row
	 * (design D12). */
type OverlayHero =
		| { kind: 'music'; music: GuiMusicFrame }
		| { kind: 'home'; home: GuiHomeFrame }
		| { kind: 'shows'; shows: GuiShowsFrame }
		| { kind: 'emby'; emby: GuiEmbyFrame }
		| { kind: 'audiobooks'; audiobooks: GuiAudiobooksFrame }
		| { kind: 'podcasts'; podcasts: GuiPodcastsFrame }
		| { kind: 'feeds'; feeds: GuiFeedsFrame }
		| null;
	const overlayHero: OverlayHero = $derived(
		music !== null && music.hero !== null
			? { kind: 'music', music }
			: home !== null && home.hero !== null
				? { kind: 'home', home }
				: shows !== null && shows.hero !== null
					? { kind: 'shows', shows }
					: emby !== null && emby.hero !== null
						? { kind: 'emby', emby }
						: audiobooks !== null && audiobooks.hero !== null
							? { kind: 'audiobooks', audiobooks }
							: podcasts !== null && podcasts.hero !== null
								? { kind: 'podcasts', podcasts }
								: feeds !== null && feeds.hero !== null
									? { kind: 'feeds', feeds }
									: null,
	);

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
	<LibraryTopBar {tabs} {currentTab} {search} homeActive={home !== null} />
	{#if music !== null}
		<!-- The Music destination's pills (design D6): Home paints none
			(design D12), so the row is absent on it, not collapsed. -->
		<LibraryPills {music} />
	{:else if shows !== null}
		<!-- The Shows destination's mode pills (design D12, row 7.1): single
			select over the three view modes. -->
		<LibraryShowsPills {shows} />
	{:else if emby !== null && emby.feedGroup}
		<!-- The feed-group Emby library's pills (design D12, row 8.1): All and
			each root folder that holds items. A plain library paints no pill row at
			all — its letter rail replaces the TUI's letter-range pill row. -->
		<LibraryEmbyPills {emby} />
	{:else if podcasts !== null}
		<!-- The Podcasts destination's episode pills (design D12, row 10.1):
			single select over Latest and each show. -->
		<LibraryPodcastsPills {podcasts} />
	{:else if feeds !== null}
		<!-- The Feeds destination's view pills (design D12, row 11.1):
			single select over Latest and each subscription. -->
		<LibraryFeedsPills {feeds} />
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
				{#if music !== null}
					{#if music.hero}
						<LibraryHero {music} />
					{/if}
				{:else if home !== null}
					{#if home.hero !== null}
						<LibraryHomeHero {home} />
					{/if}
				{:else if shows !== null}
					<!-- The series Hero with its Workspace (design D12, row 7.1): the
						Upcoming mode carries no Hero, so the pane stays empty there. -->
					{#if shows.hero !== null}
						<LibraryShowsHero {shows} />
					{/if}
				{:else if emby !== null}
					<!-- The Emby library's Hero with no Workspace (design D12, row 8.1):
						a frame without one leaves the pane empty. -->
					{#if emby.hero !== null}
						<LibraryEmbyHero {emby} />
					{/if}
				{:else if audiobooks !== null}
					<!-- The book Hero with its chapters Workspace (design D12, row
						9.1): a frame without one leaves the pane empty. -->
					{#if audiobooks.hero !== null}
						<LibraryAudiobooksHero {audiobooks} />
					{/if}
				{:else if podcasts !== null}
					<!-- The episode Hero with no Workspace (design D12, row 10.1):
						a frame without one leaves the pane empty. -->
					{#if podcasts.hero !== null}
						<LibraryPodcastsHero {podcasts} />
					{/if}
				{:else if feeds !== null}
					<!-- The entry Hero with no Workspace (design D12, row 11.1):
						a frame without one leaves the pane empty. -->
					{#if feeds.hero !== null}
						<LibraryFeedsHero {feeds} />
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
			{#if music !== null}
				<LibraryList {music} />
			{:else if home !== null}
				<LibraryHomeList {home} />
			{:else if shows !== null}
				<LibraryShowsList {shows} />
			{:else if emby !== null}
				<LibraryEmbyList {emby} />
			{:else if audiobooks !== null}
				<LibraryAudiobooksList {audiobooks} />
			{:else if podcasts !== null}
				<LibraryPodcastsList {podcasts} />
			{:else if feeds !== null}
				<LibraryFeedsList {feeds} />
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
						<LibraryHero music={overlayHero.music} />
					{:else if overlayHero.kind === 'home'}
						<LibraryHomeHero home={overlayHero.home} />
					{:else if overlayHero.kind === 'shows'}
						<LibraryShowsHero shows={overlayHero.shows} />
					{:else if overlayHero.kind === 'audiobooks'}
						<LibraryAudiobooksHero audiobooks={overlayHero.audiobooks} />
					{:else if overlayHero.kind === 'podcasts'}
						<LibraryPodcastsHero podcasts={overlayHero.podcasts} />
					{:else if overlayHero.kind === 'feeds'}
						<LibraryFeedsHero feeds={overlayHero.feeds} />
					{:else}
						<LibraryEmbyHero emby={overlayHero.emby} />
					{/if}
				</div>
			</div>
		{/if}
	</div>
	<!-- The Library Status bar (design D10): under the list region, closing the
		column with the canvas's fixed key hints and the location while the panel
		paints. -->
	<LibraryStatusBar
		{tabs}
		{currentTab}
		{music}
		{home}
		{shows}
		{emby}
		{audiobooks}
		{podcasts}
		{feeds}
		{status}
	/>
	<!-- The right-click item menu (the row-menu command): the viewport-anchored
		card at the pointer position, painted only while the Music frame carries
		the item's own entries. A press runs the entry the core built, resolved by
		that item's stable id. Home's rows name no Music item, so the menu paints
		only under Music. -->
	{#if music !== null && $libraryUi.rowMenu !== null}
		{@const carried = music.rowMenu}
		{#if carried?.itemId === $libraryUi.rowMenu.itemId && carried.entries.length > 0}
			<div class="fixed" style:left="0" style:top="0" style:width="0" style:height="0">
				<QueueRowMenu
					cursor={-1}
					entries={carried.entries}
					top="{$libraryUi.rowMenu.y}px"
					left="{$libraryUi.rowMenu.x}px"
					onSelect={runLibraryRowMenuEntry}
				/>
			</div>
		{/if}
	{/if}
</div>
