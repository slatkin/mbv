<script lang="ts">
	// One setting row of a Settings card (add-gui-settings-screen design D13,
	// the canvas row): 60 px high, the label on the left at the canvas's
	// 14 px heading colour, the control on the right, and the canvas's
	// divider under every row but its card's last one. The cursor row takes
	// the picked row's fill while the content pane holds the keyboard: a 6 px
	// rounded fill that reaches 12 px past the row's text edges (the canvas's
	// picked-row radius) and hides the divider beneath it.
	import type { Snippet } from 'svelte';

	interface Props {
		/** The row's label. */
		label: string;
		/** Whether the row paints its divider; the last row of a card
		 * passes `false`. */
		divider?: boolean;
		/** Whether the content cursor rests on this row while the content
		 * pane holds the keyboard. */
		cursor?: boolean;
		/** The row's control, flush right: a switch, a pick button, or
		 * text. */
		children?: Snippet;
	}
	const { label, divider = true, cursor = false, children }: Props = $props();
</script>

<div
	class="flex items-center justify-between"
	style:height="60px"
	style:gap="16px"
	style:margin="0 -12px"
	style:padding="0 12px"
	style:border-radius="6px"
	style:background={cursor ? 'var(--canvas-selected)' : undefined}
	style:border-bottom={divider ? `1px solid ${cursor ? 'transparent' : 'var(--canvas-border)'}` : undefined}
	data-settings-setting-row=""
	data-settings-cursor={cursor ? '' : undefined}
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
