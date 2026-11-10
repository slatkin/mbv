<script lang="ts">
	// The Hero's facts block (add-gui-library-panel design D15): the one summary
	// every Library destination's Hero paints — the artwork arm, the title, the
	// meta rows with their duration and progress roles, the hover actions, and
	// the overview. A destination supplies its facts, its own attribute prefix,
	// and the actions it consumes: Podcasts and Feeds paint Play and Add to
	// queue, Shows and Audiobooks paint no action at all because their
	// Workspace owns the row actions, and the rest paint all three.
	//
	// The landscape arm takes the TUI's layout here, once (design D15): the 16:9
	// still fills the Hero content width above the title and meta block, and the
	// portrait and square arms keep the canvas's 300 px box beside the text. A
	// destination that paints a Hero adopts that rule rather than restating it.
	import { Play } from '@lucide/svelte';
	import LibraryHeroArt from './LibraryHeroArt.svelte';
	import type { GuiLibraryArt } from '$lib/bridge/library-types';

	/** The facts every Hero carries, as its frame declares them. */
	interface HeroFacts {
		title: string;
		metaRows: string[];
		durationRow: number | null;
		progressRow: number | null;
		/** The description; a destination with none omits it. */
		overview?: string | null;
		artShape: 'landscape' | 'square' | 'portrait';
		/** The fetched artwork; a destination whose rows carry none omits it. */
		art?: GuiLibraryArt | null;
	}

	interface Props {
		/** The destination's own attribute prefix, as in `shows` or `abs`. */
		attr: string;
		/** The section's own attribute: the hook the destination's tests take. */
		section: string;
		/** The selected row's facts, present exactly while the pane paints. */
		hero: HeroFacts;
		/** Play the selected row; a destination that paints no play omits it. */
		onplay?: () => void;
		/** Play the selected row next in the queue. */
		onplaynext?: () => void;
		/** Add the selected row to the queue. */
		onqueue?: () => void;
	}
	const { attr, section, hero, onplay, onplaynext, onqueue }: Props = $props();

	/** Whether the section stacks the still above the facts. */
	const band = $derived(hero.artShape === 'landscape');

	/** The destination's own attribute, so a destination's tests keep their
	 * hook. `value` is the attribute's value where the tests read one. */
	function own(name: string, value = ''): Record<string, string> {
		return { [`data-library-${attr}-hero-${name}`]: value };
	}
</script>

<section
	class={band ? 'flex flex-col' : 'flex items-start'}
	style:gap={band ? '18px' : '28px'}
	aria-label={hero.title}
	{...{ [section]: '' }}
>
	<!-- The artwork arm every Hero shares (design D15): the 300 px box for a
	portrait or square cover, and the TUI's full-width 16:9 still for a
	landscape one. -->
	<LibraryHeroArt
		shape={hero.artShape}
		art={hero.art ?? null}
		title={hero.title}
		{attr}
	/>
	<div
		class="flex min-w-0 flex-1 flex-col"
		style:gap="14px"
		style:padding-top={band ? '0' : '8px'}
	>
		<h2
			style:margin="0"
			style:font-size="26px"
			style:font-weight="600"
			style:letter-spacing="-0.01em"
			style:color="var(--canvas-heading)"
			{...own('title')}>{hero.title}</h2
		>
		<div class="flex flex-col" style:gap="4px">
			{#each hero.metaRows as meta, index (index)}
				{#if index === hero.durationRow}
					<span
						class="font-mono tabular-nums"
						style:font-size="12px"
						style:color="var(--canvas-muted)"
						{...own('role', 'duration')}>{meta}</span
					>
				{:else if index === hero.progressRow}
					<span
						class="font-mono tabular-nums"
						style:font-size="12px"
						style:color="var(--canvas-accent)"
						{...own('role', 'progress')}>{meta}</span
					>
				{:else}
					<span
						class="font-mono"
						style:font-size="12px"
						style:color="var(--canvas-muted)"
						{...own('role', 'meta')}>{meta}</span
					>
				{/if}
			{/each}
		</div>
		{#if onplay !== undefined}
			<div class="relative flex flex-wrap items-center" style:gap="8px">
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
					style:font-size="14px"
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
						style:font-size="13px"
						style:cursor="pointer"
						{...own('play-next')}
						onclick={onplaynext}
					>
						Play next
						<span class="font-mono" style:font-size="11px" style:color="var(--canvas-muted)"
							>n</span
						>
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
						style:font-size="13px"
						style:cursor="pointer"
						{...own('add')}
						onclick={onqueue}
					>
						Add to queue
						<span class="font-mono" style:font-size="11px" style:color="var(--canvas-muted)"
							>a</span
						>
					</button>
				{/if}
			</div>
		{/if}
		{#if (hero.overview ?? null) !== null}
			<p
				class="min-w-0"
				style:margin="0"
				style:font-size="13px"
				style:line-height="1.5"
				style:color="var(--canvas-muted)"
				{...own('overview')}>{hero.overview}</p
			>
		{/if}
	</div>
</section>
