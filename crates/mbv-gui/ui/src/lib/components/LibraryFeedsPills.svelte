<script lang="ts">
	// The Feeds destination's view pills (add-gui-library-panel design D12,
	// row 11.1): Latest and one pill per subscription, single-select — the
	// core's active pill is always one of them, so a press selects it and the
	// list loads that view. The pills paint the core's carried choice, so the
	// row and the list cannot disagree; a press sends the set-pill command
	// with the pill object the frame carries (design D1). The keys' Space and
	// Enter intent (design D3) becomes the same command. The row paints the
	// canvas's pill form, as the Music, Shows, Emby, and Podcasts pills do.
	import { roleVar } from '$lib/bridge/theme';
	import type { GuiFeedsFrame, GuiFeedsPill } from '$lib/bridge/types';
	import { feedsPillKey, applyFeedsPillSelect, selectFeedsPill, feedsUi } from '$lib/library/feeds';

	interface Props {
		/** The carried Feeds content: the pills of the one feed tab (row
		 * 11.1). */
		feeds: GuiFeedsFrame;
	}
	const { feeds }: Props = $props();

	/** The key of the pill the keys' focus holds, or `null` while no pill has
	 * it (design D6): the pill's paint key over the state's pill object. */
	const focused = $derived(
		$feedsUi.focusedPill === null ? null : feedsPillKey($feedsUi.focusedPill),
	);

	/** Press a pill: select its view (row 11.1). */
	function press(pill: GuiFeedsPill): void {
		selectFeedsPill(pill.pill);
	}

	// The keys' pill request becomes the command a press sends (row 11.1):
	// the effect reads the intent so a recorded Space runs once, and the state
	// consumes it so a repaint sends nothing.
	$effect(() => {
		void $feedsUi.pillSelect;
		applyFeedsPillSelect();
	});
</script>

<div
	class="flex flex-none items-center overflow-hidden"
	role="group"
	aria-label="Feeds views"
	data-library-region="pills"
	style:height="52px"
	style:gap="6px"
	style:padding="0 16px"
	style:border-bottom="1px solid var(--canvas-border)"
>
	{#each feeds.pills as pill (feedsPillKey(pill.pill))}
		{@const key = feedsPillKey(pill.pill)}
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
			data-library-feeds-pill={key}
			data-library-pill-focused={key === focused ? '' : undefined}
			onclick={() => press(pill)}
		>
			{pill.name}
		</button>
	{/each}
</div>
