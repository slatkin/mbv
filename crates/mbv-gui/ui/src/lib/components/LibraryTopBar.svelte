<script lang="ts">
	// The Library top bar (add-gui-library-panel design D2): one tab per
	// presented destination, in the frame's catalog order, with the active
	// destination's tab marked current. Phase 1 presents Grouped Music
	// libraries only, so the bar carries no Home icon and no Search field —
	// those join the bar with Home (row 6.2) and Search (row 12.1). A press
	// sends the open-destination command with the tab's own position.
	import { roleVar } from '$lib/bridge/theme';
	import { sendLibraryCommand } from '$lib/bridge/library';
	import type { GuiLibraryTab } from '$lib/bridge/types';

	interface Props {
		/** The presented destination tabs in catalog order (design D2). */
		tabs: GuiLibraryTab[];
		/** The active tab's position, or `null` while the active destination is
		 * not one of the tabs (design D2). */
		currentTab: number | null;
	}
	const { tabs, currentTab }: Props = $props();

	/** The current tab's run: the tab bar's underline role in both strokes. */
	const underline = roleVar('tabSelectedUnderline');

	/** Open the destination tab at `position` (design D2). */
	function open(position: number): void {
		void sendLibraryCommand({ command: 'openDestination', position });
	}
</script>

<nav
	class="flex flex-none items-center"
	aria-label="Libraries"
	data-library-region="topBar"
	style:height="56px"
	style:gap="2px"
	style:padding="0 12px 0 8px"
	style:border-bottom="1px solid var(--canvas-border)"
	style:background="var(--canvas-library-panel)"
>
	{#each tabs as tab (tab.position)}
		{@const current = tab.position === currentTab}
		<button
			type="button"
			class="flex items-center uppercase"
			style:height="56px"
			style:padding="0 12px"
			style:font-size="13px"
			style:font-weight={current ? 700 : 500}
			style:letter-spacing="0.06em"
			style:color={current ? 'var(--canvas-heading)' : 'var(--canvas-muted)'}
			aria-current={current ? 'page' : undefined}
			data-library-tab={tab.position}
			onclick={() => open(tab.position)}
		>
			{#if current}
				<!-- The current tab's run: the canvas's two underline strokes around
					the label. -->
				<span
					class="block"
					style:border-top="2px solid {underline}"
					style:border-bottom="2px solid {underline}"
					style:padding="3px 1px 2px"
					style:line-height="1">{tab.label}</span
				>
			{:else}
				{tab.label}
			{/if}
		</button>
	{/each}
</nav>
