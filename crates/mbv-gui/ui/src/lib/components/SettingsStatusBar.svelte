<script lang="ts">
	// The Settings screen's status line (add-gui-settings-screen design D9,
	// the spec's *The Settings screen shows the toasts*; the canvas's status
	// bar, design D13): the 36 px band that closes the screen, painting the
	// shared Status bar projection — the armed prefix and the live toast —
	// because neither the footer line nor the Library Status bar shows while
	// the screen is open. The left side is the screen's fixed key hints in the
	// canvas's source tone while no toast or prefix is live; the right side is
	// the shown section.
	import { toastColor } from '$lib/bridge/status';
	import type { GuiStatusBar } from '$lib/bridge/types';

	interface Props {
		/** The frame's Status bar facts: the armed prefix and the live toast. */
		status: GuiStatusBar;
		/** The shown section's sidebar label. */
		section: string;
	}
	const { status, section }: Props = $props();

	/** The screen's fixed keys (design D11), which `[keys]` does not
	 * configure, so the text is fixed. */
	const HINTS: readonly { key: string; label: string }[] = [
		{ key: '↑↓', label: 'move' },
		{ key: '⏎', label: 'open' },
		{ key: 'space', label: 'toggle' },
		{ key: 'tab', label: 'pane' },
		{ key: 'esc', label: 'back' },
	];
</script>

<div
	class="flex flex-none items-center justify-between overflow-hidden font-mono whitespace-nowrap"
	style:height="36px"
	style:padding-inline="16px"
	style:font-size="12px"
	style:border-top="1px solid var(--canvas-border)"
	style:background="var(--canvas-library-panel)"
	data-settings-status=""
>
	<span class="flex items-center overflow-hidden" style:gap="12px" data-settings-status-hints="">
		{#if status.prefixArmed}
			<span class="font-bold" style:color="var(--mbv-accent)" data-settings-status-prefix=""
				>PREFIX</span
			>
		{/if}
		{#if status.toast}
			<span style:color={toastColor(status.toast.severity)} data-settings-status-toast=""
				>{status.toast.text}</span
			>
		{/if}
		{#if status.toast === null && !status.prefixArmed}
			<span class="flex items-center" style:gap="12px" style:color="var(--canvas-muted)">
				{#each HINTS as hint (hint.key)}
					<span data-settings-hint=""
						><span style:color="var(--canvas-source)">{hint.key}</span> {hint.label}</span
					>
				{/each}
			</span>
		{/if}
	</span>
	<span class="overflow-hidden" style:color="var(--canvas-muted)" data-settings-status-section=""
		>{section}</span
	>
</div>
