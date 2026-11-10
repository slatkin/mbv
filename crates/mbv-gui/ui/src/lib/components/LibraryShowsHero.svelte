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
	import LibraryHeroFrame from './LibraryHeroFrame.svelte';
	import type { GuiShowsEpisodeRow, GuiShowsFrame } from '$lib/bridge/types';
	import {
		episodeProgress,
		hoverShowsEpisode,
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

	/** A row's painted fill: the cursor row's focus fill only. A highlight
	 * marks focus, so an active row keeps its own paint. */
	function rowBackground(episode: GuiShowsEpisodeRow): string | undefined {
		return episode.episodeId === cursorEpisode ? 'var(--canvas-selected)' : undefined;
	}

	/** A row's name colour: the canvas's text tone, dimmed once the episode is
	 * played. */
	function nameColor(episode: GuiShowsEpisodeRow): string {
		return isPlayedEpisode(episode.state) ? 'var(--canvas-muted)' : 'var(--canvas-text)';
	}
</script>

{#if hero !== null}
	<LibraryHeroFrame
		attr="shows"
		root="data-library-shows-hero"
		header={{
			kicker: workspace !== null ? 'series' : 'episode',
			title: hero.title,
			facts: hero.metaRows.filter((row) => row !== '').join(' · '),
		}}
		block={{
			label: hero.title,
			name: seasonName,
			facts: seasonFacts,
			overview: hero.overview,
		}}
		artShape={hero.artShape}
		art={hero.art ?? null}
		artTitle={hero.title}
	>
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
			{#if shows.workspaceLoading}
				<span
					class="font-mono"
					style:font-size="13px"
					style:color="var(--canvas-muted)"
					data-library-shows-episodes-loading="">Loading episodes…</span
				>
			{:else}
				<section class="flex flex-col" aria-label="Episodes" data-library-shows-episodes="">
					<div
						class="grid items-center font-mono"
						style:grid-template-columns="40px minmax(0, 1fr) 88px 64px"
						style:height="32px"
						style:padding="0 8px"
						style:border-bottom="1px solid var(--canvas-border)"
						style:font-size="12px"
						style:letter-spacing="0.08em"
						style:color="var(--canvas-muted)"
					>
						<span>#</span>
						<span>TITLE</span>
						<span></span>
						<span style:text-align="right">TIME</span>
					</div>
					<ol class="flex flex-col" style:margin="4px 0 0" style:padding="0" style:list-style="none">
						{#each workspace.episodes as episode (episode.episodeId)}
							<li
								class="group relative grid items-center"
								style:grid-template-columns="40px minmax(0, 1fr) 88px 64px"
								style:height="34px"
								style:padding="0 8px"
								style:border-radius="4px"
								style:background={rowBackground(episode)}
								data-library-shows-episode={episode.episodeId}
								data-library-shows-episode-cursor={episode.episodeId === cursorEpisode
									? ''
									: undefined}
								data-library-shows-episode-state={isPlayedEpisode(episode.state)
									? 'played'
									: episodeProgress(episode.state) === null
										? 'ordinary'
										: 'active'}
								onpointerenter={() => hoverShowsEpisode(episode.episodeId)}
								onpointerleave={() => hoverShowsEpisode(null)}
							>
								<span
									class="flex-none font-mono tabular-nums"
									style:font-size="11px"
									style:color="var(--canvas-ordinal)"
									data-library-shows-episode-number="">{episode.number}</span
								>
								<button
									type="button"
									class="flex min-w-0 items-center"
									style:background="none"
									style:border="0"
									style:padding="0"
									style:text-align="left"
									style:cursor="pointer"
									aria-label="Episode {episode.name}"
									data-library-shows-episode-name=""
									onclick={() => pressShowsEpisode(episode.episodeId)}
									ondblclick={() => play(episode)}
								>
									<span
										class="min-w-0 truncate"
										style:font-size="13.5px"
										style:color={nameColor(episode)}>{episode.name}</span
									>
								</button>
								<span class="flex justify-end" style:gap="2px">
									<span
										class="justify-end"
										style:gap="2px"
										style:display={$showsWorkspace.hover === episode.episodeId ? 'flex' : 'none'}
										data-library-shows-episode-actions=""
									>
										<button
											type="button"
											class="flex items-center justify-center"
											style:width="28px"
											style:height="28px"
											style:background="none"
											style:border="0"
											style:border-radius="4px"
											style:color="var(--canvas-text)"
											style:cursor="pointer"
											aria-label="Play next"
											data-library-shows-episode-action="playNext"
											onclick={() => playNext(episode)}
										>
											<ListEnd size={14} />
										</button>
										<button
											type="button"
											class="flex items-center justify-center"
											style:width="28px"
											style:height="28px"
											style:background="none"
											style:border="0"
											style:border-radius="4px"
											style:color="var(--canvas-text)"
											style:cursor="pointer"
											aria-label="Add to queue"
											data-library-shows-episode-action="queue"
											onclick={() => queue(episode)}
										>
											<Plus size={14} />
										</button>
									</span>
								</span>
								<span
									class="font-mono tabular-nums"
									style:font-size="12px"
									style:color="var(--canvas-muted)"
									style:text-align="right"
									data-library-shows-episode-duration="">{episode.duration}</span
								>
								{#if episodeProgress(episode.state) !== null}
									<!-- The resume position: a thin accent run under the row's
									bottom edge, at the frame's bounded percent. -->
									<span
										class="absolute"
										style:left="0"
										style:bottom="0"
										style:height="2px"
										style:width="{episodeProgress(episode.state)}%"
										style:background="var(--canvas-accent)"
										data-library-shows-episode-progress=""
									></span>
								{/if}
							</li>
						{/each}
					</ol>
				</section>
			{/if}
		</section>
	{/if}
	</LibraryHeroFrame>
{/if}
