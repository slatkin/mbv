<script lang="ts">
	// The Audiobooks Hero (add-gui-library-panel design D12, row 9.1): the
	// selected book's detail pane in the Home and Shows Hero look — the
	// portrait cover-box the book artwork policy declares, the title, and the
	// meta rows with their duration and progress roles. The facts carry no
	// overview, no narrator, and no series: the Hero keeps the TUI's facts.
	//
	// The book's chapters are the Hero's Workspace (the `GuiAudiobooksWorkspace`
	// rows): title and duration, with the chapter the book plays at marked with
	// the tracklist's play glyph. The Workspace's cursor and hover are the
	// window content's own (`$lib/library/audiobooks-workspace`, design D1): a
	// press moves the cursor, a double-click plays or seeks from the chapter,
	// and the hovered row paints its play control. Books have no play next, and
	// a chapter enqueues nothing: only the play control paints.
	//
	// Play, and Add to queue (`a`), send the Audiobooks commands with the
	// frame's selected book — the core owns the selection, so the actions name
	// the book the frame painted and never a position.
	
	import { sendLibraryCommand } from '$lib/bridge/library';
	import { playhead } from '$lib/bridge/playhead';
	import LibraryHeroActions from './LibraryHeroActions.svelte';
	import LibraryHeroFrame from './LibraryHeroFrame.svelte';
	import LibraryWorkspaceTable from './LibraryWorkspaceTable.svelte';
	import { heroLayout } from '$lib/library/hero';
	import type { GuiAudiobooksChapterRow, GuiAudiobooksFrame } from '$lib/bridge/types';
	import {
		audiobooksWorkspace,
		currentChapterRow,
		pressAbsChapter,
		resolveAbsChapterCursor,
	} from '$lib/library/audiobooks-workspace';

	interface Props {
		/** The carried Audiobooks content (design D12, row 9.1): the selected
		 * book, its Hero facts, and its chapter Workspace. */
		audiobooks: GuiAudiobooksFrame;
	}
	const { audiobooks }: Props = $props();

	/** The Hero's facts, present exactly while the pane paints. */
	const hero = $derived(audiobooks.hero);

	/** The frame's header and block for the Hero (invariant 30). */
	const layout = $derived(hero === null ? null : heroLayout(hero, 'audiobook'));

	/** The book's chapter Workspace, or `null` while the detail has not
	 * landed. */
	const workspace = $derived(hero?.workspace ?? null);

	/** The playhead's predicted position (design D5): the mark reads it. */
	const { position: playheadPosition } = playhead;

	/** The chapter the Hero marks: the row whose span holds the predicted
	 * position while the Hero's book is the playing book (design D5); `null`
	 * while another book plays or none does. The marked row's identity is
	 * stable within its span, so the per-frame prediction re-renders the
	 * Workspace only when the mark moves. */
	const markedChapter = $derived.by(() => {
		if (
			audiobooks.playingBookId === null ||
			audiobooks.playingBookId !== audiobooks.selected
		) {
			return null;
		}
		return currentChapterRow(workspace?.rows ?? [], $playheadPosition);
	});

	/** The chapter the cursor rests on: the state's chapter while the carried
	 * Workspace paints it, the marked chapter, or the first chapter otherwise
	 * (design D12). */
	const cursorChapter = $derived(
		resolveAbsChapterCursor(workspace, $audiobooksWorkspace.cursor, markedChapter)?.chapter ?? null,
	);


	/** Send the book command for the frame's selected book (design D12, row
	 * 9.1): the book the frame painted, never a position. */
	function actBook(command: 'absPlay' | 'absQueue'): void {
		const itemId = audiobooks.selected;
		if (itemId !== null && hero !== null) {
			void sendLibraryCommand({ command, libIdx: audiobooks.libIdx, itemId });
		}
	}

	/** Play or seek from one chapter (design D12): the double-click and the
	 * row's hover play control send the chapter the row names — the Service
	 * discriminator the TUI's chapter activation addresses. */
	function play(chapter: GuiAudiobooksChapterRow): void {
		const bookId = audiobooks.selected;
		if (bookId !== null) {
			void sendLibraryCommand({
				command: 'absPlayChapter',
				libIdx: audiobooks.libIdx,
				bookId,
				chapter: chapter.chapter,
			});
		}
	}

	/** The Workspace table's rows: each chapter with its number, its title, its
	 * duration, and the playing mark from the predicted position (design D5). */
	const chapterRows = $derived(
		(workspace?.rows ?? []).map((row) => ({
			id: String(row.chapter),
			number: String(row.chapter),
			name: row.title,
			duration: row.duration,
			playing: row.chapter === markedChapter?.chapter,
		})),
	);

	/** A chapter has no row action beyond play: the table's play control and a
	 * double-click send it. */
	const rowActions: [] = [];

	/** Run `act` for the chapter `id` names, when the Workspace still holds it. */
	function withChapter(id: string, act: (chapter: GuiAudiobooksChapterRow) => void): void {
		const chapter = workspace?.rows.find((known) => String(known.chapter) === id);
		if (chapter !== undefined) {
			act(chapter);
		}
	}
</script>

{#if hero !== null && layout !== null}
	<LibraryHeroFrame
		attr="abs"
		root="data-library-abs-hero"
		header={layout.header}
		block={layout.block}
		artShape={hero.artShape}
		art={hero.art ?? null}
		artTitle={hero.title}
	>
		{#snippet actions()}
			<LibraryHeroActions
				attr="abs"
				onplay={() => actBook('absPlay')}
				onqueue={() => actBook('absQueue')}
			/>
		{/snippet}

		{#if workspace !== null || audiobooks.workspaceLoading}
			<!-- The book Hero's Workspace (design D12): the book's chapter rows, or
			the loading line while the detail fetch has not landed. -->
			<LibraryWorkspaceTable
				root="data-library-abs-workspace"
				label="Chapters"
				hook="abs-chapter"
				noun="Chapter"
				rows={chapterRows}
				cursor={cursorChapter === null ? null : String(cursorChapter)}
				actions={rowActions}
				onpress={(id) => pressAbsChapter(Number(id))}
				onplay={(id) => withChapter(id, play)}
				loading={workspace === null
					? { text: 'Loading chapters…', attr: 'data-library-abs-chapters-loading' }
					: null}
			/>
		{/if}
	</LibraryHeroFrame>
{/if}
