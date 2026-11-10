<script lang="ts">
	// The Podcasts destination's episode pills (add-gui-library-panel design
	// D12, row 10.1): the latest scope, the TUI's three state pills, and one
	// pill per show, single-select — the core's active pill is always one of
	// them, so a press selects it and the list loads that view. The pills paint
	// the core's carried choice, so the row and the list cannot disagree; a
	// press sends the set-pill command with the pill object the frame carries
	// (design D1). The keys' Space and Enter intent (design D3) becomes the
	// same command.
	//
	// The row itself is the shared `LibraryPillRow` (design D15), so the
	// canvas's pill form has one owner.
	import type { GuiPodcastsFrame } from '$lib/bridge/types';
	import {
		applyPodcastsPillSelect,
		podcastPillKey,
		selectPodcastsPill,
		podcastsUi,
	} from '$lib/library/podcasts';
	import LibraryPillRow from './LibraryPillRow.svelte';

	interface Props {
		/** The carried Podcasts content: the pills and the library index the
		 * set-pill command names (design D12, row 10.1). */
		podcasts: GuiPodcastsFrame;
	}
	const { podcasts }: Props = $props();

	/** The pills the row paints, keyed by the pill's paint key. */
	const pills = $derived(
		podcasts.pills.map((pill) => ({
			key: podcastPillKey(pill.pill),
			name: pill.name,
			selected: pill.selected,
		})),
	);

	/** The key of the pill the keys' focus holds, or `null` while no pill has
	 * it (design D6): the pill's paint key over the state's pill object. */
	const focused = $derived(
		$podcastsUi.focusedPill === null ? null : podcastPillKey($podcastsUi.focusedPill),
	);

	/** Press a pill: select its view (row 10.1). */
	function press(key: string): void {
		const pill = podcasts.pills.find((candidate) => podcastPillKey(candidate.pill) === key);
		if (pill !== undefined) {
			selectPodcastsPill(podcasts.libIdx, pill.pill);
		}
	}

	// The keys' pill request becomes the command a press sends (row 10.1): the
	// effect reads the intent so a recorded Space runs once, and the state
	// consumes it so a repaint sends nothing.
	$effect(() => {
		void $podcastsUi.pillSelect;
		applyPodcastsPillSelect(podcasts.libIdx);
	});
</script>

<LibraryPillRow
	ariaLabel="Podcasts views"
	{pills}
	{focused}
	pillAttr="podcasts"
	onpress={press}
/>
