<script lang="ts">
	// One setting row of a Settings card (add-gui-settings-screen design D13,
	// the canvas row): 60 px high, the label on the left at the canvas's
	// 14 px heading colour, the control on the right, and the canvas's
	// divider under every row but its card's last one.
	import type { Snippet } from 'svelte';

	interface Props {
		/** The row's label. */
		label: string;
		/** Whether the row paints its divider; the last row of a card
		 * passes `false`. */
		divider?: boolean;
		/** The row's control, flush right: a switch, a pick button, or
		 * text. */
		children?: Snippet;
	}
	const { label, divider = true, children }: Props = $props();
</script>

<div
	class="flex items-center justify-between"
	style:height="60px"
	style:gap="16px"
	style:border-bottom={divider ? '1px solid var(--canvas-border)' : undefined}
	data-settings-setting-row=""
>
	<span
		class="min-w-0 flex-1 truncate"
		style:font-size="14px"
		style:color="var(--canvas-heading)">{label}</span
	>
	<span class="flex flex-none items-center">
		{@render children?.()}
	</span>
</div>
