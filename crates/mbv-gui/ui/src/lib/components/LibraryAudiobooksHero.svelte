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
	import { Play } from '@lucide/svelte';
	import { sendLibraryCommand } from '$lib/bridge/library';
	import type { GuiAudiobooksChapterRow, GuiAudiobooksFrame } from '$lib/bridge/types';
	import {
		audiobooksWorkspace,
		hoverAbsChapter,
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

	/** The book's chapter Workspace, or `null` while the detail has not
	 * landed. */
	const workspace = $derived(hero?.workspace ?? null);

	/** The chapter the cursor rests on: the state's chapter while the carried
	 * Workspace paints it, the playing chapter, or the first chapter otherwise
	 * (design D12). */
	const cursorChapter = $derived(
		resolveAbsChapterCursor(workspace, $audiobooksWorkspace.cursor)?.chapter ?? null,
	);

	/** The artwork box's dimensions per arm (design D12): the Home Hero's
	 * 300 px box, with the portrait height the aspect gives. The cover art is
	 * the book artwork policy's portrait. */
	const ART_WIDTH = 300;
	const ART_HEIGHT = { landscape: 169, square: 300, portrait: 450 } as const;

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

	/** A row's painted fill: the cursor row's focus fill only. A highlight
	 * marks focus, so the playing chapter keeps its own fill. */
	function rowBackground(row: GuiAudiobooksChapterRow): string | undefined {
		return row.chapter === cursorChapter ? 'var(--canvas-selected)' : undefined;
	}

	/** A row's title colour: the playing chapter's heading tone. */
	function nameColor(row: GuiAudiobooksChapterRow): string {
		return row.current ? 'var(--canvas-heading)' : 'var(--canvas-text)';
	}
</script>

{#if hero !== null}
	{@const artHeight = ART_HEIGHT[hero.artShape]}
	<div class="flex flex-col" style:padding="28px 32px" style:gap="20px" data-library-abs-hero="">
		<section
			class="flex items-start"
			style:gap="28px"
			aria-label={hero.title}
			data-library-abs-hero-summary=""
		>
			<div
				class="flex flex-none items-center justify-center"
				style:width="{ART_WIDTH}px"
				style:height="{artHeight}px"
				style:background="var(--canvas-library-raise)"
				style:border-radius="6px"
				style:overflow="hidden"
				data-library-abs-hero-art={hero.art?.kind ?? 'none'}
			>
				{#if hero.art !== null && hero.art.kind !== 'loading'}
					<img class="h-full w-full object-cover" src={hero.art.detail} alt={hero.title} />
				{:else}
					<span class="font-mono" style:font-size="12px" style:color="var(--canvas-muted)"
						>[No art · the window's placeholder]</span
					>
				{/if}
			</div>
			<div class="flex min-w-0 flex-1 flex-col" style:gap="14px" style:padding-top="8px">
				<h2
					style:margin="0"
					style:font-size="26px"
					style:font-weight="600"
					style:letter-spacing="-0.01em"
					style:color="var(--canvas-heading)"
					data-library-abs-hero-title="">{hero.title}</h2
				>
				<div class="flex flex-col" style:gap="4px">
					{#each hero.metaRows as meta, index (index)}
						{#if index === hero.durationRow}
							<span
								class="font-mono tabular-nums"
								style:font-size="12px"
								style:color="var(--canvas-muted)"
								data-library-abs-hero-role="duration">{meta}</span
							>
						{:else if index === hero.progressRow}
							<span
								class="font-mono tabular-nums"
								style:font-size="12px"
								style:color="var(--canvas-accent)"
								data-library-abs-hero-role="progress">{meta}</span
							>
						{:else}
							<span
								class="font-mono"
								style:font-size="12px"
								style:color="var(--canvas-muted)"
								data-library-abs-hero-role="meta">{meta}</span
							>
						{/if}
					{/each}
				</div>
				{#if hero.overview !== null}
					<!-- The catalog's description, which the TUI's Hero paints and
					the GUI dropped before D15. -->
					<p
						style:margin="0"
						style:font-size="13px"
						style:line-height="1.5"
						style:color="var(--canvas-muted)"
						data-library-abs-hero-overview="">{hero.overview}</p
					>
				{/if}
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
						style:font-size="14px"
						style:font-weight="600"
						style:cursor="pointer"
						data-library-abs-hero-play=""
						onclick={() => actBook('absPlay')}
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
						style:font-size="13px"
						style:cursor="pointer"
						data-library-abs-hero-add=""
						onclick={() => actBook('absQueue')}
					>
						Add to queue
						<span class="font-mono" style:font-size="11px" style:color="var(--canvas-muted)"
							>a</span
						>
					</button>
				</div>
			</div>
		</section>

		{#if workspace !== null || audiobooks.workspaceLoading}
			<!-- The book Hero's Workspace (design D12): the book's chapter rows, or
			the loading line while the detail fetch has not landed. -->
			<section
				class="flex flex-col"
				style:gap="10px"
				data-library-abs-workspace=""
				aria-label="Chapters"
			>
				{#if workspace === null}
					<span
						class="font-mono"
						style:font-size="12px"
						style:color="var(--canvas-muted)"
						data-library-abs-chapters-loading="">Loading chapters…</span
					>
				{:else}
					<div
						class="grid items-center font-mono"
						style:grid-template-columns="minmax(0, 1fr) 88px 64px"
						style:height="32px"
						style:padding="0 8px"
						style:border-bottom="1px solid var(--canvas-border)"
						style:font-size="11px"
						style:letter-spacing="0.08em"
						style:color="var(--canvas-muted)"
					>
						<span>TITLE</span>
						<span></span>
						<span style:text-align="right">TIME</span>
					</div>
					<ol class="flex flex-col" style:margin="4px 0 0" style:padding="0" style:list-style="none">
						{#each workspace.rows as row (row.chapter)}
							<li
								class="group grid items-center"
								style:grid-template-columns="minmax(0, 1fr) 88px 64px"
								style:height="34px"
								style:padding="0 8px"
								style:border-radius="4px"
								style:background={rowBackground(row)}
								data-library-abs-chapter={row.chapter}
								data-library-abs-chapter-cursor={row.chapter === cursorChapter ? '' : undefined}
								data-library-abs-chapter-playing={row.current ? '' : undefined}
								onpointerenter={() => hoverAbsChapter(row.chapter)}
								onpointerleave={() => hoverAbsChapter(null)}
							>
								<button
									type="button"
									class="flex min-w-0 items-center"
									style:background="none"
									style:border="0"
									style:padding="0"
									style:text-align="left"
									style:cursor="pointer"
									aria-label="Chapter {row.title}"
									data-library-abs-chapter-name=""
									onclick={() => pressAbsChapter(row.chapter)}
									ondblclick={() => play(row)}
								>
									<span class="flex min-w-0 items-center" style:gap="6px">
										{#if row.current}
											<!-- The playing chapter's play glyph sits beside its title: a
											highlight marks focus only. -->
											<span data-library-abs-chapter-glyph="">
												<Play size={12} fill="var(--canvas-accent)" color="var(--canvas-accent)" />
											</span>
										{/if}
										<span
											class="min-w-0 truncate"
											style:font-size="14px"
											style:color={nameColor(row)}>{row.title}</span
										>
									</span>
								</button>
								<span class="flex justify-end" style:gap="2px">
									<span
										class="justify-end"
										style:gap="2px"
										style:display={$audiobooksWorkspace.hover === row.chapter ? 'flex' : 'none'}
										data-library-abs-chapter-actions=""
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
											aria-label="Play {row.title}"
											data-library-abs-chapter-action="play"
											onclick={() => play(row)}
										>
											<Play size={14} />
										</button>
									</span>
								</span>
								<span
									class="font-mono tabular-nums"
									style:font-size="12px"
									style:color="var(--canvas-muted)"
									style:text-align="right"
									data-library-abs-chapter-duration="">{row.duration}</span
								>
							</li>
						{/each}
					</ol>
				{/if}
			</section>
		{/if}
	</div>
{/if}
