<script lang="ts">
	// The Shows destination's mode pills (add-gui-library-panel design D12, row
	// 7.1): the view modes New, Upcoming, and All series, single-select — the
	// core's active mode is always one of them, so a press selects its mode and
	// the list loads it. The pills paint the core's carried choice, so the row
	// and the list cannot disagree; a press sends the set-mode command (design
	// D1). The keys' Space and Enter intent (design D3) becomes the same
	// command.
	//
	// The row itself is the shared `LibraryPillRow` (design D15), so the
	// canvas's pill form has one owner.
	import type { GuiShowsList } from '$lib/bridge/types';
	import { applyShowsPillSelect, selectShowsPill, showsUi } from '$lib/library/shows';
	import LibraryPillRow from './LibraryPillRow.svelte';

	interface Props {
		/** The carried Shows content: the pills and the library index the
		 * set-mode command names (design D12). */
		list: GuiShowsList;
	}
	const { list }: Props = $props();

	/** The pills the row paints, keyed by the mode the command names. */
	const pills = $derived(
		list.pills.map((pill) => ({
			key: pill.mode,
			name: pill.name,
			selected: pill.selected,
		})),
	);

	/** The pill the keys' focus holds; it paints the accent outline, and only
	 * while the pill row has the pane focus (design D6). */
	const focused = $derived($showsUi.focusedPill);

	/** Press a pill: select its mode (design D12). */
	function press(key: string): void {
		const pill = list.pills.find((candidate) => candidate.mode === key);
		if (pill !== undefined) {
			selectShowsPill(list.libIdx, pill.mode);
		}
	}

	// The keys' pill request becomes the command a press sends (design D12): the
	// effect reads the intent so a recorded Space runs once, and the state
	// consumes it so a repaint sends nothing.
	$effect(() => {
		void $showsUi.pillSelect;
		applyShowsPillSelect(list.libIdx);
	});
</script>

<LibraryPillRow ariaLabel="Shows modes" {pills} {focused} onpress={press} />
