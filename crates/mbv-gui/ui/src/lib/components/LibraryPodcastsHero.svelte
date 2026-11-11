<script lang="ts">
	// The Podcasts Hero (add-gui-library-panel design D12, row 10.1): the
	// selected episode's detail pane in the Emby Hero look — the artwork box
	// the `artShape` arm names, the title, the meta rows with their duration
	// and progress roles, and the episode description as the overview.
	//
	// A podcast Hero is episode information only: it carries no Workspace and
	// no tracklist. There is no play-next either: `n` is consumed by the front
	// end and does nothing for podcast episodes (design D12, row 10.1), so
	// only Play and Add to queue paint.
	//
	// Play and Add to queue send the Podcasts commands with the frame's
	// selected episode — the core owns the selection, so the actions name the
	// row the frame painted and never a position.
	
	import { sendLibraryCommand } from '$lib/bridge/library';
	import LibraryHeroActions from './LibraryHeroActions.svelte';
	import LibraryHeroFrame from './LibraryHeroFrame.svelte';
	import { heroLayout } from '$lib/library/hero';
	import type { GuiPodcastsFrame } from '$lib/bridge/types';

	interface Props {
		/** The carried Podcasts content (design D12, row 10.1): the selected
		 * episode and its Hero facts. */
		podcasts: GuiPodcastsFrame;
	}
	const { podcasts }: Props = $props();

	/** The Hero's facts, present exactly while the pane paints. */
	const hero = $derived(podcasts.hero);

	/** The frame's header and block for the Hero (invariant 30). */
	const layout = $derived(hero === null ? null : heroLayout(hero, 'podcast'));


	/** Send a Podcasts command for the frame's selected episode (design D12,
	 * row 10.1): the row the frame painted, never a position. */
	function act(command: 'podcastsPlay' | 'podcastsQueue'): void {
		const itemId = podcasts.selected;
		if (itemId !== null && hero !== null) {
			void sendLibraryCommand({ command, libIdx: podcasts.libIdx, itemId });
		}
	}
</script>

{#if hero !== null && layout !== null}
	<LibraryHeroFrame
		attr="podcasts"
		root="data-library-podcasts-hero"
		header={layout.header}
		block={layout.block}
		artShape={hero.artShape}
		art={hero.art ?? null}
		artTitle={hero.title}
	>
		{#snippet actions()}
			<LibraryHeroActions
				attr="podcasts"
				onplay={() => act('podcastsPlay')}
				onqueue={() => act('podcastsQueue')}
			/>
		{/snippet}
	</LibraryHeroFrame>
{/if}
