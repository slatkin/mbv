<script lang="ts">
	// The Podcasts Hero (add-gui-library-panel design D12, row 10.1): the
	// selected episode's detail pane in the Emby Hero look — the artwork box
	// the `artShape` arm names, the title, the meta rows with their duration
	// and progress roles, and the episode description as the overview.
	//
	// A podcast Hero is episode information only: it carries no Workspace and
	// no tracklist. There is no play-next either: `n` is consumed by the front
	// end and does nothing for podcast episodes (design D12, row 10.1), so
	// only Play and Add to queue paint.
	//
	// Play and Add to queue send the Podcasts commands with the frame's
	// selected episode — the core owns the selection, so the actions name the
	// row the frame painted and never a position.
	import { Play } from '@lucide/svelte';
	import { sendLibraryCommand } from '$lib/bridge/library';
	import type { GuiPodcastsFrame } from '$lib/bridge/types';

	interface Props {
		/** The carried Podcasts content (design D12, row 10.1): the selected
		 * episode and its Hero facts. */
		podcasts: GuiPodcastsFrame;
	}
	const { podcasts }: Props = $props();

	/** The Hero's facts, present exactly while the pane paints. */
	const hero = $derived(podcasts.hero);

	/** The artwork box's dimensions per arm (design D12): the square box keeps
	 * the Music Hero's 300 px side, and the landscape and portrait boxes the
	 * aspect gives. The Podcasts Hero's artwork policy arm is the square. */
	const ART_WIDTH = 300;
	const ART_HEIGHT = { landscape: 169, square: 300, portrait: 450 } as const;

	/** Send a Podcasts command for the frame's selected episode (design D12,
	 * row 10.1): the row the frame painted, never a position. */
	function act(command: 'podcastsPlay' | 'podcastsQueue'): void {
		const itemId = podcasts.selected;
		if (itemId !== null && hero !== null) {
			void sendLibraryCommand({ command, libIdx: podcasts.libIdx, itemId });
		}
	}
</script>

{#if hero !== null}
	{@const artHeight = ART_HEIGHT[hero.artShape]}
	<div class="flex flex-col" style:padding="28px 32px" style:gap="20px" data-library-podcasts-hero="">
		<section
			class="flex items-start"
			style:gap="28px"
			aria-label={hero.title}
			data-library-podcasts-hero-summary=""
		>
			<div
				class="flex flex-none items-center justify-center"
				style:width="{ART_WIDTH}px"
				style:height="{artHeight}px"
				style:background="var(--canvas-library-raise)"
				style:border-radius="6px"
				style:overflow="hidden"
				data-library-podcasts-hero-art={hero.art?.kind ?? 'none'}
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
					data-library-podcasts-hero-title="">{hero.title}</h2
				>
				<div class="flex flex-col" style:gap="4px">
					{#each hero.metaRows as meta, index (index)}
						{#if index === hero.durationRow}
							<span
								class="font-mono tabular-nums"
								style:font-size="12px"
								style:color="var(--canvas-muted)"
								data-library-podcasts-hero-role="duration">{meta}</span
							>
						{:else if index === hero.progressRow}
							<span
								class="font-mono tabular-nums"
								style:font-size="12px"
								style:color="var(--canvas-accent)"
								data-library-podcasts-hero-role="progress">{meta}</span
							>
						{:else}
							<span
								class="font-mono"
								style:font-size="12px"
								style:color="var(--canvas-muted)"
								data-library-podcasts-hero-role="meta">{meta}</span
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
						data-library-podcasts-hero-play=""
						onclick={() => act('podcastsPlay')}
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
						data-library-podcasts-hero-add=""
						onclick={() => act('podcastsQueue')}
					>
						Add to queue
						<span class="font-mono" style:font-size="11px" style:color="var(--canvas-muted)"
							>a</span
						>
					</button>
				</div>
				{#if hero.overview !== null}
					<p
						class="min-w-0"
						style:margin="0"
						style:font-size="13px"
						style:line-height="1.5"
						style:color="var(--canvas-muted)"
						data-library-podcasts-hero-overview="">{hero.overview}</p
					>
				{/if}
			</div>
		</section>
	</div>
{/if}
