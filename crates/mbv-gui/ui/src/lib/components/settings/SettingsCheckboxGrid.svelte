<script lang="ts">
	// A Settings checkbox grid (add-gui-settings-screen design D13, the canvas
	// My languages card): three columns of 36 px `role="checkbox"` buttons over
	// a count header, closed by the Apply primary button. A checked box paints
	// the success fill and border with an accent check box. The marks are the
	// window's, not this card's: `selected` is the store's held marks (or the
	// frame's selection), and a press on a box or on Apply runs its entry's
	// activation through `onpress`, the same one the keyboard runs. The content
	// cursor paints on the box or the Apply it rests on.
	import { Check } from '@lucide/svelte';
	import { entryId } from '$lib/settings/entries';
	import SettingsButton from './SettingsButton.svelte';
	import SettingsCard from './SettingsCard.svelte';

	interface Props {
		/** The card's heading: the section's name for the list. */
		title: string;
		/** The row's setting name, which names each box's and Apply's entry. */
		rowKey: string;
		/** The boxes, in paint order: `id` is the value the apply sends back. */
		options: { id: string; label: string }[];
		/** The ids the window marks. */
		selected: string[];
		/** The entry id the content cursor rests on, or `null` while the sidebar
		 * holds the keyboard. */
		cursorId: string | null;
		/** A press on a box or on Apply: the entry's id. */
		onpress: (id: string) => void;
		/** The header's count text; the default names the count alone. */
		countLabel?: (count: number) => string;
	}
	const {
		title,
		rowKey,
		options,
		selected,
		cursorId,
		onpress,
		countLabel = (count) => `${count} selected`,
	}: Props = $props();

	const applyId = $derived(entryId.choicesApply(rowKey));
</script>

<SettingsCard {title}>
	{#snippet header()}
		<span style:font-size="12px" style:color="var(--canvas-type-count)">
			{countLabel(selected.length)}
		</span>
	{/snippet}
	<div
		class="grid"
		style:grid-template-columns="repeat(3, minmax(0, 1fr))"
		style:gap="6px"
		data-settings-checkbox-grid=""
	>
		{#each options as option (option.id)}
			{@const id = entryId.choice(rowKey, option.id)}
			{@const on = selected.includes(option.id)}
			{@const cursor = id === cursorId}
			<button
				type="button"
				role="checkbox"
				aria-checked={on}
				class="flex items-center justify-start text-left"
				style:height="36px"
				style:padding="0 10px"
				style:gap="10px"
				style:border-radius="6px"
				style:border="1px solid {on ? 'var(--canvas-success-border)' : 'transparent'}"
				style:background={on ? 'var(--canvas-success-fill)' : 'none'}
				style:color={on ? 'var(--canvas-heading)' : 'var(--canvas-text)'}
				style:font-size="13px"
				style:outline={cursor ? '2px solid var(--canvas-accent)' : undefined}
				style:outline-offset={cursor ? '-2px' : undefined}
				data-settings-checkbox={option.id}
				data-settings-entry={id}
				data-settings-cursor={cursor ? '' : undefined}
				onclick={() => onpress(id)}
			>
				<span
					class="flex flex-none items-center justify-center"
					style:width="16px"
					style:height="16px"
					style:box-sizing="border-box"
					style:border-radius="4px"
					style:border="1.5px solid {on ? 'var(--canvas-accent)' : 'var(--canvas-ordinal)'}"
					style:background={on ? 'var(--canvas-accent)' : 'transparent'}
					aria-hidden="true"
				>
					{#if on}<Check size={11} strokeWidth={3.5} color="var(--canvas-ink)" />{/if}
				</span>
				<span class="min-w-0 truncate">{option.label}</span>
			</button>
		{/each}
	</div>
	<div class="flex justify-end">
		<SettingsButton
			variant="primary"
			entry={applyId}
			cursor={applyId === cursorId}
			onpress={() => onpress(applyId)}>Apply</SettingsButton
		>
	</div>
</SettingsCard>
