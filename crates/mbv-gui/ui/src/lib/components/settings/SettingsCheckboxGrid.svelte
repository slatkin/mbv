<script lang="ts">
	// A Settings checkbox grid (add-gui-settings-screen design D13, the canvas
	// My languages card): three columns of 36 px `role="checkbox"` buttons over
	// a count header, closed by the Apply primary button. A checked box paints
	// the success fill and border with an accent check box. The held marks seed
	// from `selected` and follow the frame whenever its own selection changes,
	// so a landed apply repaints and a frame that changes nothing keeps the
	// user's marks in place.
	import { Check } from '@lucide/svelte';
	import SettingsButton from './SettingsButton.svelte';
	import SettingsCard from './SettingsCard.svelte';

	interface Props {
		/** The card's heading: the section's name for the list. */
		title: string;
		/** The boxes, in paint order: `id` is the value the apply sends back. */
		options: { id: string; label: string }[];
		/** The ids the frame marks. */
		selected: string[];
		/** A press on Apply: the marked ids. */
		onapply: (ids: string[]) => void;
		/** The header's count text; the default names the count alone. */
		countLabel?: (count: number) => string;
	}
	const {
		title,
		options,
		selected,
		onapply,
		countLabel = (count) => `${count} selected`,
	}: Props = $props();

	/** The user's held marks, seeded from the frame's selection. */
	let marked = $state<string[]>([]);
	/** The selection signature the marks were seeded from; a plain value, so
	 * only the frame's own selection re-seeds them. */
	let seeded: string | null = null;

	$effect(() => {
		const signature = selected.join('\u0000');
		if (signature !== seeded) {
			seeded = signature;
			marked = [...selected];
		}
	});

	/** Flip one box's mark. */
	function toggle(id: string): void {
		marked = marked.includes(id)
			? marked.filter((candidate) => candidate !== id)
			: [...marked, id];
	}
</script>

<SettingsCard {title}>
	{#snippet header()}
		<span style:font-size="12px" style:color="var(--canvas-muted)">
			{countLabel(marked.length)}
		</span>
	{/snippet}
	<div
		class="grid"
		style:grid-template-columns="repeat(3, minmax(0, 1fr))"
		style:gap="6px"
		data-settings-checkbox-grid=""
	>
		{#each options as option (option.id)}
			{@const on = marked.includes(option.id)}
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
				data-settings-checkbox={option.id}
				onclick={() => toggle(option.id)}
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
		<SettingsButton variant="primary" onpress={() => onapply(marked)}>Apply</SettingsButton>
	</div>
</SettingsCard>
