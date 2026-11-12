<script lang="ts">
	// The Feeds destination's view pills (add-gui-library-panel design D12,
	// row 11.1): the latest scope and one pill per subscription, single-select
	// — the core's active pill is always one of them, so a press selects it and
	// the list loads that view. The pills paint the core's carried choice, so
	// the row and the list cannot disagree; a press sends the set-pill command
	// with the pill object the frame carries (design D1). The keys' Space and
	// Enter intent (design D3) becomes the same command.
	//
	// The row itself is the shared `LibraryPillRow` (design D15), so the
	// canvas's pill form has one owner.
	import type { GuiFeedsList } from '$lib/bridge/types';
	import { applyFeedsPillSelect, feedsPillKey, selectFeedsPill, feedsUi } from '$lib/library/feeds';
	import LibraryPillRow from './LibraryPillRow.svelte';

	interface Props {
		/** The carried Feeds content: the pills of the one feed tab (row
		 * 11.1). */
		list: GuiFeedsList;
	}
	const { list }: Props = $props();

	/** The pills the row paints, keyed by the pill's paint key. */
	const pills = $derived(
		list.pills.map((pill) => ({
			key: feedsPillKey(pill.pill),
			name: pill.name,
			selected: pill.selected,
		})),
	);

	/** The key of the pill the keys' focus holds, or `null` while no pill has
	 * it (design D6): the pill's paint key over the state's pill object. */
	const focused = $derived(
		$feedsUi.focusedPill === null ? null : feedsPillKey($feedsUi.focusedPill),
	);

	/** Press a pill: select its view (row 11.1). */
	function press(key: string): void {
		const pill = list.pills.find((candidate) => feedsPillKey(candidate.pill) === key);
		if (pill !== undefined) {
			selectFeedsPill(pill.pill);
		}
	}

	// The keys' pill request becomes the command a press sends (row 11.1):
	// the effect reads the intent so a recorded Space runs once, and the state
	// consumes it so a repaint sends nothing.
	$effect(() => {
		void $feedsUi.pillSelect;
		applyFeedsPillSelect();
	});
</script>

<LibraryPillRow ariaLabel="Feeds views" {pills} {focused} pillAttr="feeds" onpress={press} />
