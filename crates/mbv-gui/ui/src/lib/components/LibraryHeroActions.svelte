<script lang="ts">
	// The Hero's action buttons (add-gui-library-panel design D15): Play, Play
	// next (`n`), and Add to queue (`a`), each painting only when the
	// destination supplies its handler. The one set every Hero's action row
	// paints, inside the row `LibraryHeroSummary` or `LibraryHeroFrame` owns.
	import { Play } from '@lucide/svelte';
	import { heroAttr } from '$lib/library/hero';

	interface Props {
		/** The destination's own attribute prefix, as in `shows` or `abs`. */
		attr: string;
		/** Play the selected row; a destination that paints no play omits it. */
		onplay?: () => void;
		/** Play the selected row next in the queue. */
		onplaynext?: () => void;
		/** Add the selected row to the queue. */
		onqueue?: () => void;
	}
	const { attr, onplay, onplaynext, onqueue }: Props = $props();

	/** The destination's own attribute, so a destination's tests keep their hook. */
	function own(name: string): Record<string, string> {
		return { [heroAttr(attr, name)]: '' };
	}
</script>

{#if onplay !== undefined}
	<button
		type="button"
		class="flex items-center"
		style:height="44px"
		style:padding="0 18px 0 14px"
		style:gap="8px"
		style:background="var(--canvas-accent)"
		style:border="0"
		style:border-radius="22px"
		style:color="var(--canvas-ink)"
		style:font-size="15px"
		style:font-weight="600"
		style:cursor="pointer"
		{...own('play')}
		onclick={onplay}
	>
		<Play size={16} fill="var(--canvas-ink)" strokeWidth={0} />
		Play
	</button>
	{#if onplaynext !== undefined}
		<button
			type="button"
			class="flex items-center"
			style:height="44px"
			style:padding="0 14px"
			style:gap="8px"
			style:background="var(--canvas-library-raise)"
			style:border="1px solid var(--canvas-library-control)"
			style:border-radius="22px"
			style:color="var(--canvas-text)"
			style:font-size="14px"
			style:cursor="pointer"
			{...own('play-next')}
			onclick={onplaynext}
		>
			Play next
			<span class="font-mono" style:font-size="12px" style:color="var(--canvas-muted)">n</span>
		</button>
	{/if}
	{#if onqueue !== undefined}
		<button
			type="button"
			class="flex items-center"
			style:height="44px"
			style:padding="0 14px"
			style:gap="8px"
			style:background="var(--canvas-library-raise)"
			style:border="1px solid var(--canvas-library-control)"
			style:border-radius="22px"
			style:color="var(--canvas-text)"
			style:font-size="14px"
			style:cursor="pointer"
			{...own('add')}
			onclick={onqueue}
		>
			Add to queue
			<span class="font-mono" style:font-size="12px" style:color="var(--canvas-muted)">a</span>
		</button>
	{/if}
{/if}
