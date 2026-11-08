<script lang="ts">
	// The Library panel (add-gui-library-panel design D4): the column beside
	// the Queue panel. It paints the shell and the arrangement's regions — the
	// top bar and the pill row above the list, and, in Wide, the Hero pane
	// beside the list; in Narrow the Hero mounts as an overlay over the list
	// once an album is picked. The section interiors still to come are the list
	// rows and the rail (row 4.5), the Hero (row 4.7), and the Status bar (row
	// 4.8). The panel's interaction state lives in `$lib/library/state` and its
	// keys in `$lib/library/keys`, so this shell holds no state of its own.
	//
	// A press inside the panel sends the take-focus command (design D4): the
	// core sets the focus stack and Panel focus together, and the press that
	// follows acts on the row. The panel paints through the GUI-local
	// `--canvas-*` tokens of `app.css`; `mbv-theme` gains no Library slot.
	import type { GuiArrangement } from '$lib/bridge/arrangement';
	import { sendLibraryCommand } from '$lib/bridge/library';
	import type { GuiLibraryTab, GuiMusicFrame } from '$lib/bridge/types';
	import LibraryPills from '$lib/components/LibraryPills.svelte';
	import LibraryTopBar from '$lib/components/LibraryTopBar.svelte';

	interface Props {
		/** The presented destination tabs and the active one (design D2): the
		 * top bar paints them. */
		tabs: GuiLibraryTab[];
		currentTab: number | null;
		/** The carried Music content (design D2): the picked collections, the
		 * union list, and the Hero. */
		music: GuiMusicFrame;
		/** The arrangement picked from the Library column width (design D4). */
		arrangement: GuiArrangement;
	}
	const { tabs, currentTab, music, arrangement }: Props = $props();

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
				canvas orders it. Row 4.7 paints the Hero here. -->
			<div
				data-library-region="hero"
				class="flex min-h-0 min-w-0 flex-1 flex-col"
				style:background="var(--canvas-library-column)"
			></div>
		{/if}
		<!-- The list column: the canvas's 320 px index in Wide, the whole
			Library column in Mini and Narrow. Row 4.5 paints the list here. -->
		<div
			data-library-region="list"
			class="flex min-h-0 flex-col {wide
				? 'w-[320px] flex-none border-l'
				: 'min-w-0 flex-1'}"
			style:background="var(--canvas-library-panel)"
			style:border-color="var(--canvas-border)"
		></div>
		{#if arrangement === 'narrow' && music.hero}
			<!-- The Narrow Hero overlay: picking an album opens the Hero over the
				list. Row 4.7 paints the same Hero painters here. -->
			<div
				data-library-region="heroOverlay"
				class="absolute inset-0"
				style:background="var(--canvas-library-column)"
			></div>
		{/if}
	</div>
</div>
