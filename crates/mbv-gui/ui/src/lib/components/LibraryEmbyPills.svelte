<script lang="ts">
	// The plain Emby-library destination's feed-group pills (add-gui-library-panel
	// design D12, row 8.1): All and each root folder that holds items, as
	// single-select pills — the feed state's selection is always one of them, so
	// a press selects its group and the core loads that group's items. The pills
	// paint the core's carried choice, so the row and the list cannot disagree; a
	// press sends the set-feed-group command (design D1). The keys' Space and
	// Enter intent (design D3) becomes the same command. The row paints the
	// canvas's pill form, as the Music group pills do.
	//
	// A plain Emby library paints no pill row at all (row 8.1: the letter rail
	// replaces the TUI's letter-range pill row), so the panel mounts this
	// component only in the feed-group mode.
	import { roleVar } from '$lib/bridge/theme';
	import type { GuiEmbyFrame, GuiEmbyPill } from '$lib/bridge/types';
	import { embyGroupKey } from '$lib/library/emby-list';
	import { applyEmbyPillSelect, selectEmbyPill, embyUi } from '$lib/library/emby';

	interface Props {
		/** The carried plain Emby-library content: the pills and the library
		 * index the set-feed-group command names (row 8.1). */
		emby: GuiEmbyFrame;
	}
	const { emby }: Props = $props();

	/** The pill the keys' focus holds, as the group's stable key; it paints the
	 * accent outline (design D6). */
	const focused = $derived($embyUi.focusedPill);

	/** Press a pill: select its group (row 8.1). */
	function press(pill: GuiEmbyPill): void {
		selectEmbyPill(emby.libIdx, pill.group);
	}

	// The keys' pill request becomes the command a press sends (row 8.1): the
	// effect reads the intent so a recorded Space runs once, and the state
	// consumes it so a repaint sends nothing.
	$effect(() => {
		void $embyUi.pillSelect;
		applyEmbyPillSelect(emby.libIdx);
	});
</script>

<div
	class="flex flex-none items-center overflow-hidden"
	role="group"
	aria-label="Emby library folders"
	data-library-region="pills"
	style:height="52px"
	style:gap="6px"
	style:padding="0 16px"
	style:border-bottom="1px solid var(--canvas-border)"
>
	{#each emby.pills as pill (embyGroupKey(pill.group))}
		{@const key = embyGroupKey(pill.group)}
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
			data-library-emby-pill={key}
			data-library-pill={key}
			data-library-pill-focused={key === focused ? '' : undefined}
			onclick={() => press(pill)}
		>
			{pill.name}
		</button>
	{/each}
</div>
