<!--
  The Library Hero (add-gui-library-panel design D5, D8): the canvas's detail
  pane for the picked album — the artist header with Shuffle artist, the album
  block with its square artwork and its Play, Play next, Add to queue, and More
  actions controls, and the album's tracklist. The Wide arrangement paints it
  as the pane beside the list, and the Narrow arrangement paints the very same
  painters inside its overlay (design D4), so the two cannot drift.

  Every control sends the command the core applies through its existing `App`
  action (design D8): Play, Play next, Add to queue, and Shuffle artist name
  stable Service ids, and Shuffle artist names the artist's albums in the
  current union. The Hero's controls paint their keys (`n`, `a`).

  More actions opens the picked item's menu: the core builds the entries and
  the frame carries them, so the window paints what the core can run. The menu
  is offered only while the frame carries entries, because
  `execute_context_action` resolves a leaf target through the TUI's library
  cursor — the picked item is the one item the frame's entries are about.
-->
<script lang="ts">
	import { Ellipsis, Play, Shuffle } from '@lucide/svelte';
	import { sendLibraryCommand } from '$lib/bridge/library';
	import type { GuiMusicFrame } from '$lib/bridge/types';
	import { albumFacts, artistFacts, artistUnionAlbums, heroKicker } from '$lib/library/hero';
	import {
		closeLibraryItemMenu,
		libraryUi,
		openLibraryItemMenu,
	} from '$lib/library/state';
	import LibraryTracklist from './LibraryTracklist.svelte';
	import QueueRowMenu from './QueueRowMenu.svelte';

	interface Props {
		/** The carried Music content: the picked album's Hero facts, the union
		 * that names the artist's albums, and the picked item's menu entries
		 * (design D2, D5, D8). */
		music: GuiMusicFrame;
	}
	const { music }: Props = $props();

	/** The Hero's facts, or `null` while no album is picked. */
	const hero = $derived(music.hero);

	/** The Hero's More-actions entries (design D8): the frame carries the
	 * shared builder's entries in the builder's order — the order the
	 * core's execution rebuild indexes. The entries the Hero's own buttons
	 * already send (the Play entries and the add-to-queue entry) drop at
	 * paint time, so the menu names only what no button offers, and an
	 * album whose remainder is empty hides the control entirely. */
	const heroButtonLabels: ReadonlySet<string> = new Set(['Play', 'Play All', 'Add to Queue']);
	const menu = $derived(
		(music.hero?.menu ?? []).filter((entry) => !heroButtonLabels.has(entry.label)),
	);

	/** Play the album (design D8): the shared play-or-activate path, so the
	 * album's tracks reach the Owner as one replacement. */
	function play(albumId: string): void {
		void sendLibraryCommand({ command: 'play', itemId: albumId });
	}

	/** Admit the album directly after the active slot (design D8). */
	function playNext(albumId: string): void {
		void sendLibraryCommand({ command: 'playNext', itemId: albumId });
	}

	/** Append the album to the queue's tail (design D8). */
	function addToQueue(albumId: string): void {
		void sendLibraryCommand({ command: 'addToQueue', itemId: albumId });
	}

	/** Shuffle the artist (design D8): the albums of the current union, played
	 * shuffled. */
	function shuffleArtist(): void {
		if (hero === null) {
			return;
		}
		void sendLibraryCommand({
			command: 'shuffleArtist',
			albumIds: artistUnionAlbums(music, hero),
		});
	}

	/** Run the menu entry the frame carried (design D8): the core rebuilds the
	 * action for the picked item at that index. The menu closes behind the
	 * press. */
	function runMenuEntry(albumId: string, index: number): void {
		void sendLibraryCommand({ command: 'menuEntry', itemId: albumId, index });
		closeLibraryItemMenu();
	}
</script>

{#if hero !== null}
	<div class="flex flex-col" style:padding="28px 32px" style:gap="28px" data-library-hero="">
		<!-- The artist header: the artist, the selected collections, the album
			count, and the track count once the artist's tracklists have loaded. -->
		<div
			class="flex items-end justify-between"
			style:gap="24px"
			style:padding-bottom="20px"
			style:border-bottom="1px solid var(--canvas-border)"
		>
			<div class="flex min-w-0 flex-col" style:gap="6px">
				<span
					class="font-mono"
					style:font-size="12px"
					style:letter-spacing="0.08em"
					style:color="var(--canvas-muted)"
					data-library-hero-kicker="">{heroKicker(hero)}</span
				>
				<h1
					style:margin="0"
					style:font-size="32px"
					style:font-weight="600"
					style:letter-spacing="-0.01em"
					style:color="var(--canvas-heading)"
					data-library-hero-artist="">{hero.artist}</h1
				>
				<span
					class="font-mono"
					style:font-size="13px"
					style:color="var(--canvas-muted)"
					data-library-hero-facts="">{artistFacts(hero)}</span
				>
			</div>
			<button
				type="button"
				class="flex flex-none items-center"
				style:height="40px"
				style:padding="0 14px"
				style:gap="8px"
				style:background="none"
				style:border="1px solid var(--canvas-library-control)"
				style:border-radius="6px"
				style:color="var(--canvas-text)"
				style:font-size="14px"
				style:cursor="pointer"
				data-library-hero-shuffle=""
				onclick={shuffleArtist}
			>
				<Shuffle size={15} />
				Shuffle artist
			</button>
		</div>

		<!-- The album block: the square artwork and the picked album's facts
			with its controls. -->
		<section
			class="flex items-start"
			style:gap="32px"
			aria-label={hero.album.name}
			data-library-hero-album=""
		>
			<div
				class="flex flex-none items-center justify-center"
				style:width="300px"
				style:height="300px"
				style:background="var(--canvas-library-raise)"
				style:border-radius="6px"
				style:overflow="hidden"
				data-library-hero-art={music.art?.kind ?? 'none'}
			>
				{#if music.art !== null && music.art.kind !== 'loading'}
					<img
						class="h-full w-full object-cover"
						src={music.art.detail}
						alt={hero.album.name}
					/>
				{:else}
					<span class="font-mono" style:font-size="13px" style:color="var(--canvas-muted)"
						>[Album art · Emby Primary]</span
					>
				{/if}
			</div>
			<div class="flex min-w-0 flex-1 flex-col" style:gap="18px" style:padding-top="8px">
				<div class="flex flex-col" style:gap="6px">
					{#if hero.album.year !== ''}
						<span
							class="font-mono"
							style:font-size="12px"
							style:letter-spacing="0.08em"
							style:color="var(--canvas-muted)">{hero.album.year}</span
						>
					{/if}
					<h2
						style:margin="0"
						style:font-size="24px"
						style:font-weight="600"
						style:color="var(--canvas-heading)"
						data-library-hero-album-name="">{hero.album.name}</h2
					>
					{#if albumFacts(hero) !== ''}
						<span
							class="font-mono"
							style:font-size="13px"
							style:color="var(--canvas-muted)"
							data-library-hero-album-facts="">{albumFacts(hero)}</span
						>
					{/if}
				</div>
				<div class="relative flex flex-wrap items-center" style:gap="8px">
					<button
						type="button"
						class="flex items-center"
						style:height="44px"
						style:padding="0 18px 0 14px"
						style:gap="8px"
						style:background="var(--canvas-accent)"
						style:border="0"
						style:border-radius="22px"
						style:color="var(--canvas-ink)"
						style:font-size="15px"
						style:font-weight="600"
						style:cursor="pointer"
						data-library-hero-play=""
						onclick={() => play(hero.album.albumId)}
					>
						<Play size={16} fill="var(--canvas-ink)" strokeWidth={0} />
						Play
					</button>
					<button
						type="button"
						class="flex items-center"
						style:height="44px"
						style:padding="0 14px"
						style:gap="8px"
						style:background="var(--canvas-library-raise)"
						style:border="1px solid var(--canvas-library-control)"
						style:border-radius="22px"
						style:color="var(--canvas-text)"
						style:font-size="14px"
						style:cursor="pointer"
						data-library-hero-play-next=""
						onclick={() => playNext(hero.album.albumId)}
					>
						Play next
						<span class="font-mono" style:font-size="12px" style:color="var(--canvas-muted)"
							>n</span
						>
					</button>
					<button
						type="button"
						class="flex items-center"
						style:height="44px"
						style:padding="0 14px"
						style:gap="8px"
						style:background="var(--canvas-library-raise)"
						style:border="1px solid var(--canvas-library-control)"
						style:border-radius="22px"
						style:color="var(--canvas-text)"
						style:font-size="14px"
						style:cursor="pointer"
						data-library-hero-add=""
						onclick={() => addToQueue(hero.album.albumId)}
					>
						Add to queue
						<span class="font-mono" style:font-size="12px" style:color="var(--canvas-muted)"
							>a</span
						>
					</button>
					{#if menu.length > 0}
						<!-- The picked item's More actions: the dropped entries are the
						ones the Hero's own buttons already send, so an album whose menu
						left empty paints no control at all. -->
						<button
							type="button"
							class="flex items-center justify-center"
							style:width="44px"
							style:height="44px"
							style:background="none"
							style:border="0"
							style:color="var(--canvas-muted)"
							style:cursor="pointer"
							aria-label="More album actions"
							aria-expanded={$libraryUi.itemMenuOpen}
							data-library-hero-more=""
							onclick={openLibraryItemMenu}
						>
							<Ellipsis size={16} />
						</button>
					{/if}
					{#if $libraryUi.itemMenuOpen && menu.length > 0}
						<!-- The picked item's menu in the row-menu face (`QueueRowMenu`),
							dropped under the actions row as the canvas places a menu under
							its anchor. A press runs the entry the core built. -->
						<QueueRowMenu
							cursor={-1}
							entries={menu}
							top="52px"
							right="0"
							onSelect={(index) => runMenuEntry(hero.album.albumId, index)}
						/>
					{/if}
				</div>
			</div>
		</section>

		<LibraryTracklist {hero} playingTrackId={music.playingTrackId} />
	</div>
{/if}
