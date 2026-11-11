<script lang="ts">
	// The Shows Hero (add-gui-library-panel design D12, D14, D15): the Music
	// Hero's frame (`LibraryHeroFrame`) over the series, as the TUI paints the
	// TV and Music heroes through one hero and one Workspace. The header band is
	// the series — its genre, years, season count, and network — and the detail
	// block is the series' Primary poster beside the selected season and the
	// series' overview. The Workspace is the list band: the season pills, with
	// the core's selected season pressed, and that season's episodes as rows.
	// The Latest mode's Hero is the selected episode's facts and carries no
	// Workspace, and the Upcoming mode carries no Hero at all, so the pane
	// paints nothing there.
	//
	// The Workspace's cursor and hover are the window content's own
	// (`$lib/library/shows-workspace`, design D1): a press moves the cursor, a
	// double-click plays the row, and the hovered row paints its play-next and
	// add-to-queue controls. Every action sends the episode's stable Service id
	// with the library index the frame carries; no control names a row
	// position.
	import { ListEnd, Plus } from '@lucide/svelte';
	import { sendLibraryCommand } from '$lib/bridge/library';
	import LibraryHeroActions from './LibraryHeroActions.svelte';
	import LibraryHeroFrame from './LibraryHeroFrame.svelte';
	import LibraryWorkspaceTable from './LibraryWorkspaceTable.svelte';
	import { heroLayout } from '$lib/library/hero';
	import type { GuiShowsEpisodeRow, GuiShowsFrame } from '$lib/bridge/types';
	import {
		episodeProgress,
		isPlayedEpisode,
		pressShowsEpisode,
		resolveShowsEpisodeCursor,
		showsWorkspace,
	} from '$lib/library/shows-workspace';
	

	interface Props {
		/** The carried Shows content (design D12): the selected row's Hero, its
		 * Workspace, and the library index the Shows commands name. */
		shows: GuiShowsFrame;
	}
	const { shows }: Props = $props();

	/** The Hero's facts, present exactly while the pane paints. */
	const hero = $derived(shows.hero);

	/** The series Hero's Workspace, or `null` on a Hero that paints none. */
	const workspace = $derived(hero?.workspace ?? null);

	/** A Latest episode's header and block (invariant 30): the series is the
	 * largest unit and titles the header, and the episode is the block's name.
	 * `null` on the series Hero, whose Workspace holds the seasons. */
	const episodeLayout = $derived(
		hero === null || workspace !== null
			? null
			: heroLayout(hero, (hero.parent ?? '') === '' ? 'episode' : 'series'),
	);

	/** The selected season's name: the detail block's name, as the album's is
	 * Music's. Empty on a Hero that paints no Workspace. */
	const seasonName = $derived(
		workspace?.seasons.find((season) => season.selected)?.name ?? '',
	);

	/** The selected season's episode count, once its episodes have landed. */
	const seasonFacts = $derived(
		workspace === null || shows.workspaceLoading || workspace.episodes.length === 0
			? ''
			: `${workspace.episodes.length} ${workspace.episodes.length === 1 ? 'episode' : 'episodes'}`,
	);

	/** The episode the cursor rests on: the state's id while the carried
	 * Workspace paints it, and the first episode otherwise (design D12). */
	const cursorEpisode = $derived(
		resolveShowsEpisodeCursor(workspace, $showsWorkspace.cursor)?.episodeId ?? null,
	);

	/** Select a season (design D12): the select-season command the core
	 * answers with that season's episodes. */
	function selectSeason(seasonId: string): void {
		void sendLibraryCommand({ command: 'showsSelectSeason', libIdx: shows.libIdx, seasonId });
	}

	/** Play an episode (design D12): the double-click, as in Music's
	 * tracklist. The Owner owns the queue, so the shared play path replaces
	 * it. */
	function play(episode: GuiShowsEpisodeRow): void {
		void sendLibraryCommand({ command: 'showsPlay', libIdx: shows.libIdx, itemId: episode.episodeId });
	}

	/** Admit an episode directly after the active slot (design D12). */
	function playNext(episode: GuiShowsEpisodeRow): void {
		void sendLibraryCommand({
			command: 'showsPlayNext',
			libIdx: shows.libIdx,
			itemId: episode.episodeId,
		});
	}

	/** Append an episode to the queue's tail (design D12). */
	function queue(episode: GuiShowsEpisodeRow): void {
		void sendLibraryCommand({
			command: 'showsQueue',
			libIdx: shows.libIdx,
			itemId: episode.episodeId,
		});
	}

	/** The item the Hero's buttons act on (Q6 of the Hero look audit): the
	 * selected season on a series Hero, and the episode on a Latest Hero. A
	 * season is a folder, so the shared folder paths expand it. */
	const actionTarget = $derived(workspace === null ? shows.selected : workspace.selectedSeason);

	/** Send a Shows command for the Hero's target; nothing while it names none. */
	function actOn(command: 'showsPlay' | 'showsPlayNext' | 'showsQueue'): void {
		if (actionTarget !== null) {
			void sendLibraryCommand({ command, libIdx: shows.libIdx, itemId: actionTarget });
		}
	}

	/** The Workspace table's rows: each episode with its played state and its
	 * resume percent. */
	const episodeRows = $derived(
		(workspace?.episodes ?? []).map((episode) => ({
			id: episode.episodeId,
			number: String(episode.number),
			name: episode.name,
			duration: episode.duration,
			state: isPlayedEpisode(episode.state)
				? ('played' as const)
				: episodeProgress(episode.state) === null
					? ('ordinary' as const)
					: ('active' as const),
			progress: episodeProgress(episode.state),
		})),
	);

	/** The row's hover actions (design D12). */
	const rowActions = [
		{
			key: 'playNext',
			label: 'Play next',
			icon: ListEnd,
			run: (id: string) => withEpisode(id, playNext),
		},
		{ key: 'queue', label: 'Add to queue', icon: Plus, run: (id: string) => withEpisode(id, queue) },
	];

	/** Run `act` for the episode `id` names, when the Workspace still holds it. */
	function withEpisode(id: string, act: (episode: GuiShowsEpisodeRow) => void): void {
		const episode = workspace?.episodes.find((known) => known.episodeId === id);
		if (episode !== undefined) {
			act(episode);
		}
	}

	/** Play the episode a row or its play control names. */
	function playById(id: string): void {
		withEpisode(id, play);
	}
</script>

{#if hero !== null}
	<LibraryHeroFrame
		attr="shows"
		root="data-library-shows-hero"
		header={episodeLayout?.header ?? {
			kicker: 'series',
			title: hero.title,
			facts: hero.metaRows.filter((row) => row !== '').join(' · '),
		}}
		block={episodeLayout?.block ?? {
			label: hero.title,
			name: seasonName,
			facts: seasonFacts,
			overview: hero.overview,
		}}
		artShape={hero.artShape}
		art={hero.art ?? null}
		artTitle={hero.title}
	>
		{#snippet actions()}
			<LibraryHeroActions
				attr="shows"
				onplay={() => actOn('showsPlay')}
				onplaynext={() => actOn('showsPlayNext')}
				onqueue={() => actOn('showsQueue')}
			/>
		{/snippet}
	{#if workspace !== null}
		<!-- The series Hero's Workspace (design D12): the season pills, then
		the selected season's episodes. -->
		<section class="flex flex-col" style:gap="10px" data-library-shows-workspace="">
			<div
				class="flex flex-wrap items-center"
				role="group"
				aria-label="Seasons"
				style:gap="6px"
			>
				{#each workspace.seasons as season (season.seasonId)}
					<button
						type="button"
						class="flex flex-none items-center whitespace-nowrap"
						style:height="28px"
						style:padding="0 12px"
						style:border-radius="14px"
						style:font-size="13px"
						style:font-weight={season.selected ? 600 : 400}
						style:border={season.selected
							? '1px solid var(--canvas-source)'
							: '1px solid var(--canvas-border)'}
						style:background={season.selected ? 'var(--canvas-source)' : 'none'}
						style:color={season.selected ? 'var(--canvas-ink)' : 'var(--canvas-text)'}
						style:cursor="pointer"
						aria-pressed={season.selected}
						data-library-shows-season={season.seasonId}
						onclick={() => selectSeason(season.seasonId)}
					>
						{season.name}
					</button>
				{/each}
			</div>
			<LibraryWorkspaceTable
				root="data-library-shows-episodes"
				label="Episodes"
				hook="shows-episode"
				noun="Episode"
				rows={episodeRows}
				cursor={cursorEpisode}
				actions={rowActions}
				onpress={pressShowsEpisode}
				onplay={playById}
				loading={shows.workspaceLoading
					? { text: 'Loading episodes…', attr: 'data-library-shows-episodes-loading' }
					: null}
			/>
		</section>
	{/if}
	</LibraryHeroFrame>
{/if}
