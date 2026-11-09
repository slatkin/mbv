<!--
  The Hero's tracklist (add-gui-library-panel design D5, D8): the canvas's
  header row over the picked album's cached tracks. Each row paints its number,
  title, the hover actions, and the runtime, and the row under the tracklist's
  cursor paints the cursor fill so the movement keys show what Enter and the
  item keys act on.

  The two hover controls run play next and add to queue on that track (design
  D8); a press on either sends the track's own id. Hover is the row's own
  paint, so it takes no state: the canvas shows the controls, the play glyph,
  and the raised fill on the row the pointer is over.

  A double-click on a row adds the track to the queue (the user's contract),
  through the same add helper the list's track rows use, so one gesture cannot
  drift from the other. The row of the track playing at the playhead paints
  the playing state — the canvas's green with dark ink, as the Queue's active
  row paints — over the cursor's fill, from the frame's playing id.
-->
<script lang="ts">
	import { ListEnd, Play, Plus } from '@lucide/svelte';
	import type { GuiMusicHero, GuiMusicTrack } from '$lib/bridge/types';
	import { addLibraryTrackToQueue, openLibraryRowMenu } from '$lib/library/state';
	import { sendLibraryCommand } from '$lib/bridge/library';
	import { heroTracks, resolveTrackCursor } from '$lib/library/hero';
	import { libraryUi } from '$lib/library/state';

	interface Props {
		/** The picked album's Hero facts: the tracklist is the album's cached
		 * tracks (design D5). */
		hero: GuiMusicHero;
		/** The id of the track playing at the playhead, or `null` while idle:
		 * the frame's playing fact (design D8). */
		playingTrackId: string | null;
	}
	const { hero, playingTrackId }: Props = $props();

	const tracks = $derived(heroTracks(hero));

	/** The cursor's track: the row the movement keys rest on (design D3). It is
	 * derived from the tracks that paint, so a tracklist whose cache lands after
	 * the Hero pane took focus shows the cursor its keys act on. */
	const cursor = $derived(resolveTrackCursor(tracks, $libraryUi.trackCursor));

	/** Play next on the track (design D8): it enters directly after the active
	 * slot. */
	function playNext(track: GuiMusicTrack): void {
		void sendLibraryCommand({ command: 'playNext', itemId: track.trackId });
	}

	/** Add the track to the queue's tail (design D8). */
	function addToQueue(track: GuiMusicTrack): void {
		void sendLibraryCommand({ command: 'addToQueue', itemId: track.trackId });
	}

	/** Whether the row is the track playing at the playhead: the row the
	 * playing state paints over the cursor's fill (design D8). */
	function isPlaying(track: GuiMusicTrack): boolean {
		return track.trackId === playingTrackId;
	}

	/** A row's painted fill: the cursor row's focus fill only. A highlight marks
	 * focus, so the playing track keeps its own fill. */
	function rowBackground(track: GuiMusicTrack): string | undefined {
		return track.trackId === cursor ? 'var(--canvas-selected)' : undefined;
	}

	/** A row's title colour: the playing track's heading tone. */
	function rowColor(track: GuiMusicTrack): string {
		return isPlaying(track) ? 'var(--canvas-heading)' : 'var(--canvas-text)';
	}

	/** Open the track's right-click item menu (the row-menu command): the
	 * entries the TUI's library menu offers for a track row, carried in the
	 * next frame's `rowMenu` section for the menu the panel paints at the
	 * pointer. */
	function rowMenu(event: MouseEvent, track: GuiMusicTrack): void {
		event.preventDefault();
		openLibraryRowMenu(track.trackId, event.clientX, event.clientY);
	}
</script>

<section class="flex flex-col" aria-label="Tracklist" data-library-tracklist="">
	<div
		class="grid items-center font-mono"
		style:grid-template-columns="40px minmax(0, 1fr) 88px 64px"
		style:height="32px"
		style:padding="0 8px"
		style:border-bottom="1px solid var(--canvas-border)"
		style:font-size="11px"
		style:letter-spacing="0.08em"
		style:color="var(--canvas-muted)"
	>
		<span>#</span>
		<span>TITLE</span>
		<span></span>
		<span style:text-align="right">TIME</span>
	</div>
	<ol class="flex flex-col" style:margin="4px 0 0" style:padding="0" style:list-style="none">
		{#each tracks as track (track.trackId)}
		<li
			class="group grid items-center"
			style:grid-template-columns="40px minmax(0, 1fr) 88px 64px"
			style:height="34px"
			style:padding="0 8px"
			style:border-radius="4px"
			style:background={rowBackground(track)}
			data-library-track={track.trackId}
			data-library-track-cursor={track.trackId === cursor ? '' : undefined}
			data-library-track-playing={isPlaying(track) ? '' : undefined}
			oncontextmenu={(event) => rowMenu(event, track)}
			ondblclick={() => addLibraryTrackToQueue(track.trackId)}
		>
			<span
				class="flex-none font-mono tabular-nums"
				style:font-size="12px"
				style:color="var(--canvas-ordinal)"
			>
				<span class="group-hover:hidden">{track.number}</span>
				<span class="hidden group-hover:flex">
					<Play size={12} fill="var(--canvas-accent)" color="var(--canvas-accent)" />
				</span>
			</span>
			<span class="flex min-w-0 items-center" style:gap="6px">
				{#if isPlaying(track)}
					<!-- The playing track's play glyph sits beside its title: a highlight marks focus only. -->
					<Play size={12} fill="var(--canvas-accent)" color="var(--canvas-accent)" />
				{/if}
				<span
					class="min-w-0 truncate"
					style:font-size="14px"
					style:color={rowColor(track)}>{track.name}</span
				>
			</span>
				<span class="flex justify-end" style:gap="2px">
					<span class="hidden justify-end group-hover:flex" style:gap="2px">
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
							data-library-track-action="playNext"
							onclick={() => playNext(track)}
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
							data-library-track-action="addToQueue"
							onclick={() => addToQueue(track)}
						>
							<Plus size={14} />
						</button>
					</span>
				</span>
				<span
					class="font-mono tabular-nums"
					style:font-size="12px"
					style:color="var(--canvas-muted)"
					style:text-align="right">{track.duration}</span
				>
			</li>
		{/each}
	</ol>
</section>
