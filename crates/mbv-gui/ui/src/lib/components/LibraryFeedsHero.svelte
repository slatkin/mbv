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
	import LibraryHeroActions from './LibraryHeroActions.svelte';
	import { ListEnd } from '@lucide/svelte';
	import LibraryHeroFrame from './LibraryHeroFrame.svelte';
	import LibraryWorkspaceTable from './LibraryWorkspaceTable.svelte';
	import { selectFeedsRow } from '$lib/library/feeds';
	import { episodeProgress, isPlayedEpisode } from '$lib/library/shows-workspace';
	import { heroLayout } from '$lib/library/hero';
	import type { GuiFeedsFrame } from '$lib/bridge/types';

	interface Props {
		/** The carried Feeds content (design D12, row 11.1): the selected
		 * entry and its Hero facts. */
		feeds: GuiFeedsFrame;
	}
	const { feeds }: Props = $props();

	/** The Hero's facts, present exactly while the pane paints. */
	const hero = $derived(feeds.hero);

	/** The frame's header and block for the Hero (invariant 30). */
	const layout = $derived(hero === null ? null : heroLayout(hero, 'feed'));

	/** The feed's entries, newest first, as the Workspace table paints them: the
	 * active pill's rows that belong to the Hero's feed, the selected entry
	 * marked by the table's cursor. A date takes the time cell. */
	const entryRows = $derived(
		hero?.parent == null
			? []
			: feeds.rows
					.filter((row) => row.subtitle === hero.parent)
					.map((row, index) => ({
						id: row.id,
						number: String(index + 1),
						name: row.title,
						duration: row.date ?? '',
						state: isPlayedEpisode(row.state)
							? ('played' as const)
							: episodeProgress(row.state) === null
								? ('ordinary' as const)
								: ('active' as const),
						progress: episodeProgress(row.state),
					})),
	);

	/** The row's hover action: add the entry to the queue. */
	const rowActions = [
		{
			key: 'queue',
			label: 'Add to queue',
			icon: ListEnd,
			run: (id: string) => void sendLibraryCommand({ command: 'feedsQueue', itemId: id }),
		},
	];


	/** Send a Feeds command for the frame's selected entry (design D12, row
	 * 11.1): the row the frame painted, never a position. */
	function act(command: 'feedsPlay' | 'feedsQueue'): void {
		const itemId = feeds.selected;
		if (itemId !== null && hero !== null) {
			void sendLibraryCommand({ command, itemId });
		}
	}
</script>

{#if hero !== null && layout !== null}
	<LibraryHeroFrame
		attr="feeds"
		root="data-library-feeds-hero"
		header={layout.header}
		block={layout.block}
		artShape={hero.artShape}
		art={null}
		artTitle={hero.title}
	>
		{#snippet actions()}
			<LibraryHeroActions
				attr="feeds"
				onplay={() => act('feedsPlay')}
				onqueue={() => act('feedsQueue')}
			/>
		{/snippet}
		<LibraryWorkspaceTable
			root="data-library-feeds-entries"
			label="Entries"
			hook="feeds-entry"
			noun="Entry"
			rows={entryRows}
			cursor={feeds.selected}
			actions={rowActions}
			onpress={selectFeedsRow}
			onplay={(id) => void sendLibraryCommand({ command: 'feedsPlay', itemId: id })}
		/>
	</LibraryHeroFrame>
{/if}
