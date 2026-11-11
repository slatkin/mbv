<script lang="ts">
	// The Library pill row (add-gui-library-panel design D6): the group pills of
	// the active Grouped Music library. One group is always selected, as the
	// TUI's Grouped Music pill is, so a press selects its pill and the others
	// deselect. The pills paint the core's carried choice, so the row and the
	// list cannot disagree; a press sends the select-collection command (design
	// D1). The keys' Space and Enter intent (design D3) becomes the same command.
	//
	// The row itself is the shared `LibraryPillRow` the other destinations paint
	// too (design D15), so the canvas's pill form has one owner.
	import type { GuiMusicFrame } from '$lib/bridge/types';
	import {
		applyLibraryPillSelect,
		focusedLibraryPill,
		libraryUi,
		selectLibraryPill,
	} from '$lib/library/state';
	import LibraryPillRow from './LibraryPillRow.svelte';

	interface Props {
		/** The carried Music content: the pills and the library index the
		 * select-collection command names (design D2, D6). */
		music: GuiMusicFrame;
	}
	const { music }: Props = $props();

	/** The pills the row paints, keyed by the group id the command names. */
	const pills = $derived(
		music.pills.map((pill) => ({
			key: pill.groupId,
			name: pill.name,
			selected: pill.selected,
		})),
	);

	/** The pill the keys' focus holds, or `null` while no pill has it. */
	const focused = $derived(focusedLibraryPill(music.pills, $libraryUi)?.groupId ?? null);

	/** Press a pill: select it (design D6). */
	function press(key: string): void {
		selectLibraryPill(music.libIdx, key);
	}

	// The keys' pill request becomes the command a press sends (design D6): the
	// effect reads the intent so a recorded Space runs once, and the state
	// consumes it so a repaint sends nothing.
	$effect(() => {
		void $libraryUi.pillSelect;
		applyLibraryPillSelect(music.libIdx);
	});
</script>

<LibraryPillRow ariaLabel="Collections" {pills} {focused} onpress={press} />
