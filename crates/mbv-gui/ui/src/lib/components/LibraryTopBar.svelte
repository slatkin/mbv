<script lang="ts">
	// The Library top bar (add-gui-library-panel design D2, D12): the canvas's
	// Home icon, then one tab per presented destination in the frame's catalog
	// order, with the active destination's tab marked current. The icon is
	// current while the Home content is present (the destinations are
	// exclusive), and a press on the icon or on a tab sends the
	// open-destination command with that destination's own position. The
	// Search field joins the bar with Search (row 12.1).
	import { roleVar } from '$lib/bridge/theme';
	import { sendLibraryCommand } from '$lib/bridge/library';
	import type { GuiLibraryTab } from '$lib/bridge/types';

	interface Props {
		/** The presented destination tabs in catalog order (design D2). */
		tabs: GuiLibraryTab[];
		/** The active tab's position, or `null` while the active destination is
		 * not one of the tabs (design D2): Home, and every destination the GUI
		 * does not present yet. */
		currentTab: number | null;
		/** Whether the Home content is present (design D12): the Home icon
		 * marks itself current on it alone. */
		homeActive?: boolean;
	}
	const { tabs, currentTab, homeActive = false }: Props = $props();

	/** The current tab's run: the tab bar's underline role in both strokes. */
	const underline = roleVar('tabSelectedUnderline');

	/** Open the destination tab at `position` (design D2): Home's own position
	 * is 0, and the Home icon opens it. */
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
	<!-- The canvas's Home icon, before the tabs (design D12): the current
		destination marker follows the tabs' `aria-current` rule while the Home
		content is present. -->
	<button
		type="button"
		class="flex flex-none items-center justify-center"
		style:width="44px"
		style:height="44px"
		style:color={homeActive ? 'var(--canvas-heading)' : 'var(--canvas-muted)'}
		style:cursor="pointer"
		aria-label="Home"
		aria-current={homeActive ? 'page' : undefined}
		data-library-home=""
		onclick={() => open(0)}
	>
		<!-- The canvas's Home glyph, verbatim. -->
		<svg
			width="18"
			height="18"
			viewBox="0 0 24 24"
			fill="none"
			stroke="currentColor"
			stroke-width="1.8"
			stroke-linejoin="round"
			aria-hidden="true"
		><path d="M3 11 12 4l9 7v9a1 1 0 0 1-1 1h-5v-6H9v6H4a1 1 0 0 1-1-1z" /></svg>
	</button>
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
