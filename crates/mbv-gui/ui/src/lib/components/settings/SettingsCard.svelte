<script lang="ts">
	// One card of a Settings section (add-gui-settings-screen design D13, the
	// canvas `section`): the raised fill, a border, a 10 px radius, and 20 px
	// of padding. A card that holds setting rows paints the canvas's 8 px by
	// 20 px padding instead, so each row's divider spans the card's inner
	// width. The heading takes trailing content — a state pill or a count —
	// through the `header` snippet.
	import type { Snippet } from 'svelte';

	interface Props {
		/** The card's heading. A card with no heading names itself through
		 * `label` instead. */
		title?: string;
		/** The card's accessible name while it paints no heading. */
		label?: string;
		/** The heading's trailing content: a state pill or a count. */
		header?: Snippet;
		/** Paint the canvas's row-list padding (`8px 20px`) and no gap, for a
		 * card whose body is `SettingsRow`s with their own dividers. */
		rows?: boolean;
		/** The card's body. */
		children?: Snippet;
	}
	const {
		title = undefined,
		label = undefined,
		header = undefined,
		rows = false,
		children,
	}: Props = $props();

	/** Whether the card paints a heading band. */
	const headed = $derived(title !== undefined || header !== undefined);
	/** The card's padding: a headed card whose body is setting rows keeps the
	 * canvas's 20 px side padding, so the heading and the rows share one left
	 * edge, and closes with the canvas's 8 px foot. */
	const padding = $derived(rows ? (headed ? '20px 20px 8px' : '8px 20px') : '20px');
</script>

<section
	class="flex flex-col"
	style:background="var(--canvas-library-panel)"
	style:border="1px solid var(--canvas-border)"
	style:border-radius="10px"
	style:padding={padding}
	style:gap={rows ? '0' : '16px'}
	aria-label={label}
	data-settings-card=""
>
	{#if headed}
		<div
			class="flex items-center justify-between"
			style:gap="12px"
			style:padding-bottom={rows ? '12px' : undefined}
		>
			{#if title !== undefined}
				<h2
					style:margin="0"
					style:font-size="16px"
					style:font-weight="600"
					style:color="var(--canvas-heading)">{title}</h2
				>
			{/if}
			{#if header !== undefined}
				{@render header()}
			{/if}
		</div>
	{/if}
	{@render children?.()}
</section>
