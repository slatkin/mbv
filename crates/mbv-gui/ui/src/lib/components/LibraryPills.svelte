<script lang="ts">
	// The Library pill row (add-gui-library-panel design D6): the group pills of
	// the active Grouped Music library. One group is always selected, as the
	// TUI's Grouped Music pill is, so a press selects its pill and the others
	// deselect. The pills paint the core's carried choice, so the row and the
	// list cannot disagree; a press sends the select-collection command (design
	// D1). The keys' Space and Enter intent (design D3) becomes the same command.
	import { roleVar } from '$lib/bridge/theme';
	import type { GuiMusicFrame, GuiMusicPill } from '$lib/bridge/types';
	import {
		applyLibraryPillSelect,
		focusedLibraryPill,
		libraryUi,
		selectLibraryPill,
	} from '$lib/library/state';

	interface Props {
		/** The carried Music content: the pills and the library index the
		 * select-collection command names (design D2, D6). */
		music: GuiMusicFrame;
	}
	const { music }: Props = $props();

	/** The pill the keys' focus holds, or `null` while no pill has it. */
	const focused = $derived(focusedLibraryPill(music.pills, $libraryUi)?.groupId ?? null);

	/** Press a pill: select it (design D6). */
	function press(pill: GuiMusicPill): void {
		selectLibraryPill(music.libIdx, pill.groupId);
	}

	// The keys' pill request becomes the command a press sends (design D6): the
	// effect reads the intent so a recorded Space runs once, and the state
	// consumes it so a repaint sends nothing.
	$effect(() => {
		void $libraryUi.pillSelect;
		applyLibraryPillSelect(music.libIdx);
	});
</script>

<div
	class="flex flex-none items-center overflow-hidden"
	role="group"
	aria-label="Collections"
	data-library-region="pills"
	style:height="52px"
	style:gap="6px"
	style:padding="0 16px"
	style:border-bottom="1px solid var(--canvas-border)"
>
	{#each music.pills as pill (pill.groupId)}
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
			style:outline={pill.groupId === focused
				? `2px solid ${roleVar('accentActive')}`
				: 'none'}
			style:outline-offset="1px"
			aria-pressed={pill.selected}
			data-library-pill={pill.groupId}
			data-library-pill-focused={pill.groupId === focused ? '' : undefined}
			onclick={() => press(pill)}
		>
			{pill.name}
		</button>
	{/each}
</div>
