<script lang="ts">
	// The Feeds Hero (add-gui-library-panel design D12, row 11.1): the
	// selected entry's detail pane in the Emby Hero look — the artwork box
	// the `artShape` arm names, the title, and the meta rows with their
	// duration role.
	//
	// A feed entry has no artwork source and no overview, so the box paints
	// the window's placeholder for the shape and no overview paragraph
	// paints. There is no Workspace and no tracklist. Play next is not a
	// Feeds command (row 11.1), so only Play and Add to queue paint.
	//
	// Play and Add to queue send the Feeds commands with the frame's
	// selected entry — the core owns the selection, so the actions name the
	// row the frame painted and never a position. A Client holds one feed
	// tab, so no library index rides the commands.
	
	import { sendLibraryCommand } from '$lib/bridge/library';
	import LibraryHeroSummary from './LibraryHeroSummary.svelte';
	import type { GuiFeedsFrame } from '$lib/bridge/types';

	interface Props {
		/** The carried Feeds content (design D12, row 11.1): the selected
		 * entry and its Hero facts. */
		feeds: GuiFeedsFrame;
	}
	const { feeds }: Props = $props();

	/** The Hero's facts, present exactly while the pane paints. */
	const hero = $derived(feeds.hero);


	/** Send a Feeds command for the frame's selected entry (design D12, row
	 * 11.1): the row the frame painted, never a position. */
	function act(command: 'feedsPlay' | 'feedsQueue'): void {
		const itemId = feeds.selected;
		if (itemId !== null && hero !== null) {
			void sendLibraryCommand({ command, itemId });
		}
	}
</script>

{#if hero !== null}
	<div class="flex flex-col" style:padding="28px 32px" style:gap="20px" data-library-feeds-hero="">
		<LibraryHeroSummary
						attr="feeds"
			section="data-library-feeds-hero-summary"
			{hero}
			onplay={() => act('feedsPlay')}
			onqueue={() => act('feedsQueue')}
		/>
	</div>
{/if}
