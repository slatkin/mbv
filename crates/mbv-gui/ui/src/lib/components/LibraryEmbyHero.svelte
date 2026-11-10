<script lang="ts">
	// The plain Emby-library Hero (add-gui-library-panel design D12, row 8.1):
	// the selected row's detail pane in the Home and Shows Hero look — the
	// artwork box the `artShape` arm names (landscape, square, or portrait),
	// the title, the meta rows with their duration and progress roles, the
	// overview, and the hover actions.
	//
	// A plain Emby library carries no Workspace and no tracklist: a folder or a
	// boxset plays through the TUI's folder-play rule, so the pane is one block
	// and the Library opens no nested list.
	//
	// Play, Play next (`n`), and Add to queue (`a`) send the Emby commands with
	// the frame's selected row — the core owns the selection, so the actions
	// name the row the frame painted and never a position.
	
	import { sendLibraryCommand } from '$lib/bridge/library';
	import LibraryHeroSummary from './LibraryHeroSummary.svelte';
	
	import type { GuiEmbyFrame } from '$lib/bridge/types';

	interface Props {
		/** The carried plain Emby-library content (design D12, row 8.1): the
		 * selected row and its Hero facts. */
		emby: GuiEmbyFrame;
	}
	const { emby }: Props = $props();

	/** The Hero's facts, present exactly while the pane paints. */
	const hero = $derived(emby.hero);

	/** Send an Emby-library command for the frame's selected row (design D12,
	 * row 8.1): the row the frame painted, never a position. */
	function act(command: 'embyPlay' | 'embyPlayNext' | 'embyQueue'): void {
		const itemId = emby.selected;
		if (itemId !== null && hero !== null) {
			void sendLibraryCommand({ command, libIdx: emby.libIdx, itemId });
		}
	}
</script>

{#if hero !== null}
	<div class="flex flex-col" style:padding="28px 32px" style:gap="20px" data-library-emby-hero="">
		<LibraryHeroSummary
						attr="emby"
			section="data-library-emby-hero-summary"
			{hero}
			onplay={() => act('embyPlay')}
			onplaynext={() => act('embyPlayNext')}
			onqueue={() => act('embyQueue')}
		/>
	</div>
{/if}
