<script lang="ts">
	// The Home Hero (add-gui-library-panel design D12, the USER-APPROVED GUI
	// Home design): the selected row's detail pane in the Music Hero's look —
	// the artwork box the `artShape` arm names (landscape, square, or
	// portrait), the title, the meta rows with their duration and progress
	// roles, the overview, and the hover actions. No Workspace and no
	// tracklist paint here: Home has neither, so the pane is one block.
	//
	// Play, Play next (`n`), and Add to queue (`a`) send the Home commands
	// with the frame's selected row — the core owns the selection, so the
	// actions name the row the frame painted and never a position.
	
	import { sendLibraryCommand } from '$lib/bridge/library';
	import LibraryHeroSummary from './LibraryHeroSummary.svelte';
	
	import type { GuiHomeFrame } from '$lib/bridge/types';

	interface Props {
		/** The carried Home content (design D12): the selected row and its
		 * Hero facts. */
		home: GuiHomeFrame;
	}
	const { home }: Props = $props();

	/** The Hero's facts, present exactly while the pane paints. */
	const hero = $derived(home.hero);

	/** Send a Home command for `row` when an action has one (design D12): the
	 * frame's selected row, never a position. */
	function act(
		command: 'homePlay' | 'homePlayNext' | 'homeQueue',
		row: Props['home']['selected'],
	): void {
		if (row !== null && hero !== null) {
			void sendLibraryCommand({ command, row });
		}
	}
</script>

{#if hero !== null}
	<div class="flex flex-col" style:padding="28px 32px" style:gap="20px" data-library-hero="">
		<LibraryHeroSummary
						attr="home"
			section="data-library-home-hero"
			{hero}
			onplay={() => act('homePlay', home.selected)}
			onplaynext={() => act('homePlayNext', home.selected)}
			onqueue={() => act('homeQueue', home.selected)}
		/>
	</div>
{/if}
