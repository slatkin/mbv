<!--
  The Hero's tracklist (add-gui-library-panel design D5, D8, D15): the picked
  album's cached tracks, painted by the one Workspace table every destination
  shares. The row under the tracklist's cursor paints the cursor fill so the
  movement keys show what Enter and the item keys act on.

  The two hover controls run play next and add to queue on that track (design
  D8); a press on either sends the track's own id. A track plays from its row's
  play button, which replaces the track number on hover, and from a
  double-click on the row: the queue is replaced, and under autoload the album
  plays from that track onward. The row of the track playing at the playhead
  shows a play glyph beside its title (the `GuiQueue` frame names `active`); a
  highlight marks focus only, so it keeps its own fill.
-->
<script lang="ts">
	import { ListEnd, Plus } from '@lucide/svelte';
	import type { GuiMusicHero } from '$lib/bridge/types';
	import {
		libraryUi,
		openLibraryRowMenu,
		playLibraryTrack,
		pressLibraryTrack,
	} from '$lib/library/state';
	import { sendLibraryCommand } from '$lib/bridge/library';
	import { heroTracks, resolveTrackCursor } from '$lib/library/hero';
	import LibraryWorkspaceTable from './LibraryWorkspaceTable.svelte';

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

	const rows = $derived(
		tracks.map((track) => ({
			id: track.trackId,
			number: String(track.number),
			name: track.name,
			duration: track.duration,
			playing: track.trackId === playingTrackId,
		})),
	);

	/** The hover actions (design D8): play next enters directly after the active
	 * slot, and add to queue appends to the tail. */
	const actions = [
		{
			key: 'playNext',
			label: 'Play next',
			icon: ListEnd,
			run: (trackId: string) =>
				void sendLibraryCommand({ command: 'playNext', itemId: trackId }),
		},
		{
			key: 'addToQueue',
			label: 'Add to queue',
			icon: Plus,
			run: (trackId: string) =>
				void sendLibraryCommand({ command: 'addToQueue', itemId: trackId }),
		},
	];

	/** Open the track's right-click item menu (the row-menu command): the
	 * entries the TUI's library menu offers for a track row, carried in the
	 * next frame's `rowMenu` section for the menu the panel paints at the
	 * pointer. */
	function rowMenu(event: MouseEvent, trackId: string): void {
		event.preventDefault();
		openLibraryRowMenu(trackId, event.clientX, event.clientY);
	}
</script>

<LibraryWorkspaceTable
	root="data-library-tracklist"
	label="Tracklist"
	hook="track"
	noun="Track"
	{rows}
	{cursor}
	{actions}
	onpress={pressLibraryTrack}
	onplay={playLibraryTrack}
	oncontext={rowMenu}
/>
