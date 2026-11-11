<script lang="ts">
	// The plain Emby-library Hero (add-gui-library-panel design D12, row 8.1, and
	// D17): the Music Hero's frame (`LibraryHeroFrame`) over the selected row, as
	// the TUI paints every Emby item through the one `hero_content_emby`
	// producer. A movie is its own largest unit, so the header band carries the
	// item's kind word as the kicker, the title, and the plain facts on one
	// line; the block beside the art holds the duration and progress rows, the
	// action row, and the overview; the movie's credits table is the list band.
	//
	// The `IMDb` meta row is a link the header's facts string cannot hold, so
	// the consumer maps it out of the facts it hands `heroLayout` (invariant
	// 30: the piece is never bent) and paints it as the header's trailing
	// action — the IMDb button carries the url for the core's open-URL command
	// (the click wiring lands with that command).
	//
	// A plain Emby library carries no Workspace and no tracklist: a folder or a
	// boxset plays through the TUI's folder-play rule, so the pane is the frame
	// and no nested list opens.
	//
	// Play, Play next (`n`), and Add to queue (`a`) send the Emby commands with
	// the frame's selected row — the core owns the selection, so the actions
	// name the row the frame painted and never a position.

	import { sendLibraryCommand } from '$lib/bridge/library';
	import LibraryHeroActions from './LibraryHeroActions.svelte';
	import LibraryHeroFrame from './LibraryHeroFrame.svelte';
	import { heroAttr, heroLayout } from '$lib/library/hero';
	import type { HeroLayoutFacts } from '$lib/library/hero';
	import type { GuiEmbyFrame, GuiEmbyHero } from '$lib/bridge/types';

	interface Props {
		/** The carried plain Emby-library content (design D12, row 8.1): the
		 * selected row and its Hero facts. */
		emby: GuiEmbyFrame;
	}
	const { emby }: Props = $props();

	/** The Hero's facts, present exactly while the pane paints. */
	const hero = $derived(emby.hero);

	/** The facts `heroLayout` maps: the `IMDb` meta row out of the plain rows
	 * (the header action paints it as a link), with the duration and progress
	 * indices remapped to the rows that remain. */
	function layoutFacts(facts: GuiEmbyHero): HeroLayoutFacts {
		const metaRows: string[] = [];
		let durationRow: number | null = null;
		let progressRow: number | null = null;
		facts.metaRows.forEach((row, index) => {
			if (facts.imdbUrl !== null && row === 'IMDb') {
				return;
			}
			if (index === facts.durationRow) {
				durationRow = metaRows.length;
			}
			if (index === facts.progressRow) {
				progressRow = metaRows.length;
			}
			metaRows.push(row);
		});
		return { ...facts, metaRows, durationRow, progressRow };
	}

	/** The frame's header and block for the Hero (invariant 30): the item is
	 * its own largest unit, so it titles the header, and its kind word in
	 * upper case is the kicker. */
	const layout = $derived(hero === null ? null : heroLayout(layoutFacts(hero), hero.kind.toUpperCase()));

	/** The Hero part's own attribute, so the tests keep their hook. */
	function own(name: string): Record<string, string> {
		return { [heroAttr('emby', name)]: '' };
	}

	/** Send an Emby-library command for the frame's selected row (design D12,
	 * row 8.1): the row the frame painted, never a position. */
	function act(command: 'embyPlay' | 'embyPlayNext' | 'embyQueue'): void {
		const itemId = emby.selected;
		if (itemId !== null && hero !== null) {
			void sendLibraryCommand({ command, libIdx: emby.libIdx, itemId });
		}
	}
</script>

{#if hero !== null && layout !== null}
	<LibraryHeroFrame
		attr="emby"
		root="data-library-emby-hero"
		header={layout.header}
		block={layout.block}
		artShape={hero.artShape}
		art={hero.art ?? null}
		artTitle={hero.title}
	>
		{#snippet headerAction()}
			{#if hero.imdbUrl !== null}
				<!-- The IMDb link (design D17, Q7): the TUI underlines the `IMDb`
				meta row and a press opens the url; the button carries the url in the
				attribute until the core's open-URL command wires the click. -->
				<button
					type="button"
					class="font-mono"
					style:font-size="13px"
					style:padding="0"
					style:background="none"
					style:border="0"
					style:text-decoration="underline"
					style:color="var(--canvas-muted)"
					style:cursor="pointer"
					data-library-emby-imdb={hero.imdbUrl}>IMDb</button
				>
			{/if}
		{/snippet}
		{#snippet actions()}
			<LibraryHeroActions
				attr="emby"
				onplay={() => act('embyPlay')}
				onplaynext={() => act('embyPlayNext')}
				onqueue={() => act('embyQueue')}
			/>
		{/snippet}
		{#if hero.credits.length > 0}
			<!-- The movie's credits table (design D17, Q7): the list band under
			the overview, the director first as the TUI's table paints it — the
			name at the left, the role at the right, the TUI's every-other-row
			stripe. -->
			<section class="flex flex-col" aria-label="Credits" {...own('credits')}>
				{#each hero.credits as credit, index (index)}
					<div
						class="flex items-baseline justify-between"
						style:gap="24px"
						style:padding="4px 8px"
						style:background={index % 2 === 1 ? 'var(--canvas-library-raise)' : 'none'}
						{...own('credit')}
					>
						<span
							class="min-w-0"
							style:font-size="14px"
							style:color="var(--canvas-text)"
							{...own('credit-name')}>{credit.name}</span
						>
						<span
							class="font-mono"
							style:font-size="13px"
							style:color="var(--canvas-muted)"
							{...own('credit-role')}>{credit.role}</span
						>
					</div>
				{/each}
			</section>
		{/if}
	</LibraryHeroFrame>
{/if}
