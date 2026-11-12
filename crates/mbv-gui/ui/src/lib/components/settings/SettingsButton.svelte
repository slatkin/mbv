<script lang="ts">
	// One Settings button (add-gui-settings-screen design D13, the canvas
	// buttons): 36 px high with a 6 px radius in four kinds — secondary (the
	// raised fill and the control border), primary (the accent fill and ink
	// text), destructive (the destructive outline), and ghost (muted text with
	// no fill). A disabled button paints as unavailable, so a press on it never
	// runs its handler (design D11's pointer contract). A button that activates
	// a content entry carries the entry's id and the content cursor's ring, so
	// the keyboard and the pointer rest on the same control.
	import type { Snippet } from 'svelte';

	type Variant = 'secondary' | 'primary' | 'destructive' | 'ghost';

	interface Props {
		/** The button's kind; secondary is the default. */
		variant?: Variant;
		/** Whether the action is currently unavailable. */
		disabled?: boolean;
		/** The content entry this button activates (`entryId` in
		 * `$lib/settings/entries`), painted for the section's press handler and
		 * the cursor fact. */
		entry?: string;
		/** The button's accessible name, for a button whose label is a glyph. */
		label?: string;
		/** Whether the content cursor rests on this button while the content
		 * pane holds the keyboard. */
		cursor?: boolean;
		/** A press on the button. */
		onpress?: () => void;
		/** The button's label. */
		children?: Snippet;
	}
	const {
		variant = 'secondary',
		disabled = false,
		entry = undefined,
		label = undefined,
		cursor = false,
		onpress = undefined,
		children,
	}: Props = $props();

	/** Each kind's fill, border, text colour, padding, and weight. */
	const VARIANTS: Record<
		Variant,
		{ background: string; border: string; color: string; padding: string; weight?: string }
	> = {
		secondary: {
			background: 'var(--canvas-library-raise)',
			border: '1px solid var(--canvas-library-control)',
			color: 'var(--canvas-text)',
			padding: '0 14px',
		},
		primary: {
			background: 'var(--canvas-accent)',
			border: '0',
			color: 'var(--canvas-ink)',
			padding: '0 16px',
			weight: '600',
		},
		destructive: {
			background: 'none',
			border: '1px solid var(--canvas-destructive-border)',
			color: 'var(--canvas-destructive)',
			padding: '0 14px',
		},
		ghost: {
			background: 'none',
			border: '0',
			color: 'var(--canvas-muted)',
			padding: '0 14px',
		},
	};
	const paint = $derived(VARIANTS[variant]);
</script>

<button
	type="button"
	class="flex-none whitespace-nowrap"
	style:height="36px"
	style:padding={paint.padding}
	style:background={paint.background}
	style:border={paint.border}
	style:border-radius="6px"
	style:color={disabled ? 'var(--canvas-ordinal)' : paint.color}
	style:font-size="13px"
	style:font-weight={paint.weight}
	style:opacity={disabled ? '0.7' : undefined}
	style:outline={cursor ? '2px solid var(--canvas-accent)' : undefined}
	style:outline-offset={cursor ? '-2px' : undefined}
	data-settings-button={variant}
	data-settings-entry={entry}
	data-settings-cursor={cursor ? '' : undefined}
	aria-label={label}
	{disabled}
	onclick={() => onpress?.()}
>
	{@render children?.()}
</button>
