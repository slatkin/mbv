<script lang="ts">
	// The Shows destination's mode pills (add-gui-library-panel design D12, row
	// 7.1): the three view modes Latest, Upcoming, and All series, single-select
	// — the core's active mode is always one of them, so a press selects its
	// mode and the list loads it. The pills paint the core's carried choice, so
	// the row and the list cannot disagree; a press sends the set-mode command
	// (design D1). The keys' Space and Enter intent (design D3) becomes the same
	// command. The row paints the canvas's pill form, as the Music group pills
	// do.
	import { roleVar } from '$lib/bridge/theme';
	import type { GuiShowsFrame, GuiShowsPill } from '$lib/bridge/types';
	import { applyShowsPillSelect, selectShowsPill, showsUi } from '$lib/library/shows';

	interface Props {
		/** The carried Shows content: the pills and the library index the
		 * set-mode command names (design D12). */
		shows: GuiShowsFrame;
	}
	const { shows }: Props = $props();

	/** The pill the keys' focus holds; it paints the accent outline, and only
	 * while the pill row has the pane focus (design D6). */
	const focused = $derived($showsUi.focusedPill);

	/** Press a pill: select its mode (design D12). */
	function press(pill: GuiShowsPill): void {
		selectShowsPill(shows.libIdx, pill.mode);
	}

	// The keys' pill request becomes the command a press sends (design D12): the
	// effect reads the intent so a recorded Space runs once, and the state
	// consumes it so a repaint sends nothing.
	$effect(() => {
		void $showsUi.pillSelect;
		applyShowsPillSelect(shows.libIdx);
	});
</script>

<div
	class="flex flex-none items-center overflow-hidden"
	role="group"
	aria-label="Shows modes"
	data-library-region="pills"
	style:height="52px"
	style:gap="6px"
	style:padding="0 16px"
	style:border-bottom="1px solid var(--canvas-border)"
>
	{#each shows.pills as pill (pill.mode)}
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
			style:outline={pill.mode === focused ? `2px solid ${roleVar('accentActive')}` : 'none'}
			style:outline-offset="1px"
			aria-pressed={pill.selected}
			data-library-pill={pill.mode}
			data-library-pill-focused={pill.mode === focused ? '' : undefined}
			onclick={() => press(pill)}
		>
			{pill.name}
		</button>
	{/each}
</div>
