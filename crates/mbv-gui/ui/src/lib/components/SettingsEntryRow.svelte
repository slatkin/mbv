<script lang="ts">
	// One activatable row of a Settings section (add-gui-settings-screen
	// design D11): the label, an optional leading mark, and an optional
	// trailing value, in the one row look every section paints. The cursor
	// fill paints only while the content pane holds the keyboard (`cursor`
	// names the row the cursor rests on). A press reports to the screen's
	// press handler, which moves the cursor and runs the entry's activation.
	import { Check } from '@lucide/svelte';

	interface Props {
		/** The entry's id (`entryId` in `$lib/settings/entries`), which the
		 * press handler and the cursor fact read. */
		id: string;
		/** Whether the content cursor rests on this row while the content
		 * pane holds the keyboard. */
		cursor: boolean;
		/** A press on the row. */
		onpress: (id: string) => void;
		/** The row's label. */
		label: string;
		/** The right-side value text, if any. */
		trailing?: string | null;
		/** A choice mark: checked, unchecked, or absent. */
		marked?: boolean | null;
		/** Whether the row activates nothing (a device with no endpoint). */
		disabled?: boolean;
		/** Indent the row under its group header. */
		indent?: boolean;
	}
	const {
		id,
		cursor,
		onpress,
		label,
		trailing = null,
		marked = null,
		disabled = false,
		indent = false,
	}: Props = $props();
</script>

<button
	type="button"
	class="flex w-full items-center text-left"
	style:height="36px"
	style:padding="0 16px"
	style:padding-left={indent ? '32px' : undefined}
	style:gap="8px"
	style:font-size="14px"
	style:background={cursor ? 'var(--canvas-selected)' : 'none'}
	style:border="0"
	style:color={disabled ? 'var(--canvas-ordinal)' : 'var(--canvas-text)'}
	style:opacity={disabled ? '0.7' : undefined}
	{disabled}
	data-settings-entry={id}
	data-settings-cursor={cursor ? '' : undefined}
	onclick={() => onpress(id)}
>
	{#if marked !== null}
		<span
			class="flex flex-none items-center"
			style:width="18px"
			style:color={marked ? 'var(--canvas-accent)' : 'var(--canvas-muted)'}
			aria-hidden="true"
		>
			{#if marked}<Check size={14} />{/if}
		</span>
	{/if}
	<span class="min-w-0 flex-1 truncate">{label}</span>
	{#if trailing !== null}
		<span class="flex-none truncate" style:color="var(--canvas-muted)">{trailing}</span>
	{/if}
</button>
