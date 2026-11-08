<script lang="ts">
	// The Library pill row (add-gui-library-panel design D6): the group pills of
	// the active Grouped Music library, the selected count with its clear
	// control, and the rules that turn a gesture into the set-collections
	// command. The pills paint the core's carried selection, so the row and the
	// list cannot disagree; the window content computes the new set and sends it
	// (design D1). A press toggles a collection pill, an Alt-press solos it, and
	// a single-select pill replaces the set. The keys' Space and Alt+Space
	// intent (design D3) becomes the same command a press sends.
	import { Check } from '@lucide/svelte';
	import { roleVar } from '$lib/bridge/theme';
	import type { GuiMusicFrame, GuiMusicPill } from '$lib/bridge/types';
	import { selectedPillIds } from '$lib/library/pills';
	import {
		applyLibraryPillToggle,
		focusedLibraryPill,
		libraryUi,
		selectLibraryPills,
	} from '$lib/library/state';

	interface Props {
		/** The carried Music content: the pills and the library index the
		 * set-collections command names (design D2, D6). */
		music: GuiMusicFrame;
	}
	const { music }: Props = $props();

	/** The selected group ids in pill order: the design's count and the clear
	 * control read them. */
	const selected = $derived(selectedPillIds(music.pills));

	/** The pill the keys' focus holds, or `null` while no pill has it. */
	const focused = $derived(focusedLibraryPill(music.pills, $libraryUi)?.groupId ?? null);

	/** Press a pill: toggle it, or solo it with Alt held (design D6). */
	function press(pill: GuiMusicPill, solo: boolean): void {
		selectLibraryPills(music.libIdx, music.pills, {
			kind: 'press',
			groupId: pill.groupId,
			solo,
		});
	}

	/** The clear control: the first selected pill in pill order stays. */
	function clear(): void {
		selectLibraryPills(music.libIdx, music.pills, { kind: 'clear' });
	}

	// The keys' pill toggle becomes the command a press sends (design D6): the
	// effect reads the intent so a recorded Space or Alt+Space runs once, and
	// the state consumes it so a repaint sends nothing.
	$effect(() => {
		void $libraryUi.pillToggle;
		applyLibraryPillToggle(music.libIdx, music.pills);
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
			style:padding={pill.selected ? '0 12px 0 9px' : '0 12px'}
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
			onclick={(event) => press(pill, event.altKey)}
		>
			{#if pill.selected}
				<Check size={12} strokeWidth={3} color="var(--canvas-ink)" />
			{/if}
			{pill.name}
		</button>
	{/each}
	{#if selected.length > 1}
		<!-- The selected count with its clear control: the canvas's mono run
			past the divider, clear keeping the first selected pill (design D6). -->
		<button
			type="button"
			class="flex-none whitespace-nowrap font-mono"
			style:height="32px"
			style:padding="0 10px"
			style:margin-left="4px"
			style:background="none"
			style:border="0"
			style:border-left="1px solid var(--canvas-border)"
			style:font-size="11px"
			style:color="var(--canvas-muted)"
			data-library-pill-clear=""
			onclick={clear}
		>
			{selected.length} selected · clear
		</button>
	{/if}
</div>
