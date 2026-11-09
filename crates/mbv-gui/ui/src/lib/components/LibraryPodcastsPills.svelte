<script lang="ts">
	// The Podcasts destination's episode pills (add-gui-library-panel design
	// D12, row 10.1): Latest and one pill per show, single-select — the core's
	// active pill is always one of them, so a press selects it and the list
	// loads that view. The pills paint the core's carried choice, so the row
	// and the list cannot disagree; a press sends the set-pill command with
	// the pill object the frame carries (design D1). The keys' Space and
	// Enter intent (design D3) becomes the same command. The row paints the
	// canvas's pill form, as the Music, Shows, and Emby pills do.
	import { roleVar } from '$lib/bridge/theme';
	import type { GuiPodcastsFrame, GuiPodcastsPill } from '$lib/bridge/types';
	import { podcastPillKey, applyPodcastsPillSelect, selectPodcastsPill, podcastsUi } from '$lib/library/podcasts';

	interface Props {
		/** The carried Podcasts content: the pills and the library index the
		 * set-pill command names (design D12, row 10.1). */
		podcasts: GuiPodcastsFrame;
	}
	const { podcasts }: Props = $props();

	/** The key of the pill the keys' focus holds, or `null` while no pill has
	 * it (design D6): the pill's paint key over the state's pill object. */
	const focused = $derived(
		$podcastsUi.focusedPill === null ? null : podcastPillKey($podcastsUi.focusedPill),
	);

	/** Press a pill: select its view (row 10.1). */
	function press(pill: GuiPodcastsPill): void {
		selectPodcastsPill(podcasts.libIdx, pill.pill);
	}

	// The keys' pill request becomes the command a press sends (row 10.1): the
	// effect reads the intent so a recorded Space runs once, and the state
	// consumes it so a repaint sends nothing.
	$effect(() => {
		void $podcastsUi.pillSelect;
		applyPodcastsPillSelect(podcasts.libIdx);
	});
</script>

<div
	class="flex flex-none items-center overflow-hidden"
	role="group"
	aria-label="Podcasts views"
	data-library-region="pills"
	style:height="52px"
	style:gap="6px"
	style:padding="0 16px"
	style:border-bottom="1px solid var(--canvas-border)"
>
	{#each podcasts.pills as pill (podcastPillKey(pill.pill))}
		{@const key = podcastPillKey(pill.pill)}
		<button
			type="button"
			class="flex flex-none items-center whitespace-nowrap"
			style:height="32px"
			style:gap="5px"
			style:padding="0 12px"
			style:border-radius="16px"
			style:font-size="13px"
			style:font-weight={pill.selected ? 600 : 400}
			style:border={pill.selected
				? '1px solid var(--canvas-source)'
				: '1px solid var(--canvas-border)'}
			style:background={pill.selected ? 'var(--canvas-source)' : 'none'}
			style:color={pill.selected ? 'var(--canvas-ink)' : 'var(--canvas-text)'}
			style:outline={key === focused ? `2px solid ${roleVar('accentActive')}` : 'none'}
			style:outline-offset="1px"
			aria-pressed={pill.selected}
			data-library-pill={key}
			data-library-podcasts-pill={key}
			data-library-pill-focused={key === focused ? '' : undefined}
			onclick={() => press(pill)}
		>
			{pill.name}
		</button>
	{/each}
</div>
