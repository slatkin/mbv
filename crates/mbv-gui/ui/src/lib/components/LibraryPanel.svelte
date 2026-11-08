<script lang="ts">
	// The Library panel (add-gui-library-panel design D4): the column beside
	// the Queue panel. It paints the shell and the arrangement's regions — the
	// top bar and the pill row above the list, and, in Wide, the Hero pane
	// beside the list; in Narrow the Hero mounts as an overlay over the list
	// once an album is picked. The list column is row 4.5's canvas tree with its
	// letter rail, the Hero is row 4.7's painters, and the Status bar (row 4.8)
	// closes the column under both. The panel's interaction state lives in
	// `$lib/library/state` and its keys in `$lib/library/keys`, so this shell
	// holds no state of its own.
	//
	// A press inside the panel sends the take-focus command (design D4): the
	// core sets the focus stack and Panel focus together, and the press that
	// follows acts on the row. The panel paints through the GUI-local
	// `--canvas-*` tokens of `app.css`; `mbv-theme` gains no Library slot.
	//
	// The Narrow Hero overlay covers the list, so it must be dismissible: its
	// backdrop press closes it, and Esc closes it in the key router (design D3).
	// The panel also reports the arrangement it paints, so the router closes an
	// overlay only where one actually paints.
	import type { GuiArrangement } from '$lib/bridge/arrangement';
	import { sendLibraryCommand } from '$lib/bridge/library';
	import { dragListWidth, resetListWidth } from '$lib/bridge/resize';
	import type { GuiLibraryTab, GuiMusicFrame, GuiStatusBar } from '$lib/bridge/types';
	import LibraryHero from '$lib/components/LibraryHero.svelte';
	import LibraryList from '$lib/components/LibraryList.svelte';
	import LibraryPills from '$lib/components/LibraryPills.svelte';
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
		 * union list, and the Hero. */
		music: GuiMusicFrame;
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
	const { tabs, currentTab, music, status, arrangement, listWidth, libraryColumn }: Props =
		$props();

	/** Whether the Hero sits beside the list rather than over it. */
	const wide = $derived(arrangement === 'wide');

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
	<LibraryTopBar {tabs} {currentTab} />
	<LibraryPills {music} />
	<div class="relative flex min-h-0 flex-1">
		{#if wide}
			<!-- The Wide arrangement's Hero pane, to the left of the list as the
				canvas orders it, with the Hero's own painters inside. -->
			<div
				data-library-region="hero"
				class="flex min-h-0 min-w-0 flex-1 flex-col"
				style:background="var(--canvas-library-column)"
				style:overflow-y="auto"
			>
				{#if music.hero}
					<LibraryHero {music} />
				{/if}
			</div>
		{/if}
		<!-- The list column: the canvas's index in Wide, the whole Library
			column in Mini and Narrow. Row 4.5 paints the canvas tree and the
			letter rail here. Wide paints the session's width (design D4), and its
			divider sits between the list and the Hero pane. -->
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
			<LibraryList {music} />
		</div>
		{#if arrangement === 'narrow' && music.hero && !$libraryUi.heroDismissed}
			<!-- The Narrow Hero overlay (design D4): picking an item paints the Hero
				over the list, through the same painters the Wide pane uses, so the two
				cannot drift. The backdrop press and Esc close it (design D3), so a
				picked album never leaves an overlay no input dismisses; the card scrolls
				its own content and swallows the presses that land on it. -->
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
					<LibraryHero {music} />
				</div>
			</div>
		{/if}
	</div>
	<!-- The Library Status bar (design D10): under the list region, closing the
		column with the canvas's fixed key hints and the location while the panel
		paints. -->
	<LibraryStatusBar {tabs} {currentTab} {music} {status} />
	<!-- The right-click item menu (the row-menu command): the viewport-anchored
		card at the pointer position, painted only while the frame carries the
		item's own entries. A press runs the entry the core built, resolved by
		that item's stable id. -->
	{#if $libraryUi.rowMenu !== null}
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
