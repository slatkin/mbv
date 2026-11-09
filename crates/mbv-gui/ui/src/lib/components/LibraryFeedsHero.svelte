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
	import { Play } from '@lucide/svelte';
	import { sendLibraryCommand } from '$lib/bridge/library';
	import type { GuiFeedsFrame } from '$lib/bridge/types';

	interface Props {
		/** The carried Feeds content (design D12, row 11.1): the selected
		 * entry and its Hero facts. */
		feeds: GuiFeedsFrame;
	}
	const { feeds }: Props = $props();

	/** The Hero's facts, present exactly while the pane paints. */
	const hero = $derived(feeds.hero);

	/** The artwork box's dimensions per arm (design D12): the square box
	 * keeps the Music Hero's 300 px side, and the landscape box the 16:9
	 * height, with the portrait box the tall arm keeps. The box paints the
	 * placeholder in every arm: a feed entry has no artwork source. */
	const ART_WIDTH = 300;
	const ART_HEIGHT = { landscape: 169, square: 300, portrait: 450 } as const;

	/** Send a Feeds command for the frame's selected entry (design D12, row
	 * 11.1): the row the frame painted, never a position. */
	function act(command: 'feedsPlay' | 'feedsQueue'): void {
		const itemId = feeds.selected;
		if (itemId !== null && hero !== null) {
			void sendLibraryCommand({ command, itemId });
		}
	}
</script>

{#if hero !== null}
	{@const artHeight = ART_HEIGHT[hero.artShape]}
	<div class="flex flex-col" style:padding="28px 32px" style:gap="20px" data-library-feeds-hero="">
		<section
			class="flex items-start"
			style:gap="28px"
			aria-label={hero.title}
			data-library-feeds-hero-summary=""
		>
			<div
				class="flex flex-none items-center justify-center"
				style:width="{ART_WIDTH}px"
				style:height="{artHeight}px"
				style:background="var(--canvas-library-raise)"
				style:border-radius="6px"
				style:overflow="hidden"
				data-library-feeds-hero-art={hero.artShape}
			>
				<span class="font-mono" style:font-size="12px" style:color="var(--canvas-muted)"
					>[No art · the window's placeholder]</span
				>
			</div>
			<div class="flex min-w-0 flex-1 flex-col" style:gap="14px" style:padding-top="8px">
				<h2
					style:margin="0"
					style:font-size="26px"
					style:font-weight="600"
					style:letter-spacing="-0.01em"
					style:color="var(--canvas-heading)"
					data-library-feeds-hero-title="">{hero.title}</h2
				>
				<div class="flex flex-col" style:gap="4px">
					{#each hero.metaRows as meta, index (index)}
						{#if index === hero.durationRow}
							<span
								class="font-mono tabular-nums"
								style:font-size="12px"
								style:color="var(--canvas-muted)"
								data-library-feeds-hero-role="duration">{meta}</span
							>
						{:else if index === hero.progressRow}
							<span
								class="font-mono tabular-nums"
								style:font-size="12px"
								style:color="var(--canvas-accent)"
								data-library-feeds-hero-role="progress">{meta}</span
							>
						{:else}
							<span
								class="font-mono"
								style:font-size="12px"
								style:color="var(--canvas-muted)"
								data-library-feeds-hero-role="meta">{meta}</span
							>
						{/if}
					{/each}
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
						style:font-size="14px"
						style:font-weight="600"
						style:cursor="pointer"
						data-library-feeds-hero-play=""
						onclick={() => act('feedsPlay')}
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
						data-library-feeds-hero-add=""
						onclick={() => act('feedsQueue')}
					>
						Add to queue
						<span class="font-mono" style:font-size="11px" style:color="var(--canvas-muted)"
							>a</span
						>
					</button>
				</div>
			</div>
		</section>
	</div>
{/if}
