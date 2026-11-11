<script lang="ts">
	// The plain Emby-library destination's pills (add-gui-library-panel design
	// D12, row 8.1): in the feed-group mode, the latest scope, `All`, and each
	// root folder that holds items; in a plain library, the latest scope and
	// `All`. A press selects its group and the core loads that group's items.
	// The pills paint the core's carried choice, so the row and the list cannot
	// disagree; a press sends the set-feed-group command (design D1). The keys'
	// Space and Enter intent (design D3) becomes the same command.
	//
	// The row itself is the shared `LibraryPillRow` (design D15), so the
	// canvas's pill form has one owner.
	import type { GuiEmbyFrame } from '$lib/bridge/types';
	import { applyEmbyPillSelect, selectEmbyPill, embyUi } from '$lib/library/emby';
	import { embyGroupKey } from '$lib/library/emby-list';
	import LibraryPillRow from './LibraryPillRow.svelte';

	interface Props {
		/** The carried plain Emby-library content: the pills and the library
		 * index the set-feed-group command names (row 8.1). */
		emby: GuiEmbyFrame;
	}
	const { emby }: Props = $props();

	/** The pills the row paints, keyed by the group key the command's pill
	 * resolves. */
	const pills = $derived(
		emby.pills.map((pill) => ({
			key: embyGroupKey(pill.group),
			name: pill.name,
			selected: pill.selected,
		})),
	);

	/** The pill the keys' focus holds, as the group's stable key; it paints the
	 * accent outline (design D6). */
	const focused = $derived($embyUi.focusedPill);

	/** Press a pill: select its group (row 8.1). */
	function press(key: string): void {
		const pill = emby.pills.find((candidate) => embyGroupKey(candidate.group) === key);
		if (pill !== undefined) {
			selectEmbyPill(emby.libIdx, pill.group);
		}
	}

	// The keys' pill request becomes the command a press sends (row 8.1): the
	// effect reads the intent so a recorded Space runs once, and the state
	// consumes it so a repaint sends nothing.
	$effect(() => {
		void $embyUi.pillSelect;
		applyEmbyPillSelect(emby.libIdx);
	});
</script>

<LibraryPillRow
	ariaLabel="Emby library folders"
	{pills}
	{focused}
	pillAttr="emby"
	onpress={press}
/>
