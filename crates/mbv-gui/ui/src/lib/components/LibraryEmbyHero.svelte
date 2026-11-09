<script lang="ts">
	// The plain Emby-library Hero (add-gui-library-panel design D12, row 8.1):
	// the selected row's detail pane in the Home and Shows Hero look — the
	// artwork box the `artShape` arm names (landscape, square, or portrait),
	// the title, the meta rows with their duration and progress roles, the
	// overview, and the hover actions.
	//
	// A plain Emby library carries no Workspace and no tracklist: a folder or a
	// boxset plays through the TUI's folder-play rule, so the pane is one block
	// and the Library opens no nested list.
	//
	// Play, Play next (`n`), and Add to queue (`a`) send the Emby commands with
	// the frame's selected row — the core owns the selection, so the actions
	// name the row the frame painted and never a position.
	import { Play } from '@lucide/svelte';
	import { sendLibraryCommand } from '$lib/bridge/library';
	import type { GuiEmbyFrame } from '$lib/bridge/types';

	interface Props {
		/** The carried plain Emby-library content (design D12, row 8.1): the
		 * selected row and its Hero facts. */
		emby: GuiEmbyFrame;
	}
	const { emby }: Props = $props();

	/** The Hero's facts, present exactly while the pane paints. */
	const hero = $derived(emby.hero);

	/** The artwork box's dimensions per arm (design D12): the Music Hero's
	 * 300 px side where the square fits, and the landscape and portrait
	 * boxes the aspect gives. */
	const ART_WIDTH = 300;
	const ART_HEIGHT = { landscape: 169, square: 300, portrait: 450 } as const;

	/** Send an Emby-library command for the frame's selected row (design D12,
	 * row 8.1): the row the frame painted, never a position. */
	function act(command: 'embyPlay' | 'embyPlayNext' | 'embyQueue'): void {
		const itemId = emby.selected;
		if (itemId !== null && hero !== null) {
			void sendLibraryCommand({ command, libIdx: emby.libIdx, itemId });
		}
	}
</script>

{#if hero !== null}
	{@const artHeight = ART_HEIGHT[hero.artShape]}
	<div class="flex flex-col" style:padding="28px 32px" style:gap="20px" data-library-emby-hero="">
		<section
			class="flex items-start"
			style:gap="28px"
			aria-label={hero.title}
			data-library-emby-hero-summary=""
		>
			<div
				class="flex flex-none items-center justify-center"
				style:width="{ART_WIDTH}px"
				style:height="{artHeight}px"
				style:background="var(--canvas-library-raise)"
				style:border-radius="6px"
				style:overflow="hidden"
				data-library-emby-hero-art={hero.art?.kind ?? 'none'}
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
					data-library-emby-hero-title="">{hero.title}</h2
				>
				<div class="flex flex-col" style:gap="4px">
					{#each hero.metaRows as meta, index (index)}
						{#if index === hero.durationRow}
							<span
								class="font-mono tabular-nums"
								style:font-size="12px"
								style:color="var(--canvas-muted)"
								data-library-emby-hero-role="duration">{meta}</span
							>
						{:else if index === hero.progressRow}
							<span
								class="font-mono tabular-nums"
								style:font-size="12px"
								style:color="var(--canvas-accent)"
								data-library-emby-hero-role="progress">{meta}</span
							>
						{:else}
							<span
								class="font-mono"
								style:font-size="12px"
								style:color="var(--canvas-muted)"
								data-library-emby-hero-role="meta">{meta}</span
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
						data-library-emby-hero-play=""
						onclick={() => act('embyPlay')}
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
						data-library-emby-hero-play-next=""
						onclick={() => act('embyPlayNext')}
					>
						Play next
						<span class="font-mono" style:font-size="11px" style:color="var(--canvas-muted)"
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
						style:font-size="13px"
						style:cursor="pointer"
						data-library-emby-hero-add=""
						onclick={() => act('embyQueue')}
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
						data-library-emby-hero-overview="">{hero.overview}</p
					>
				{/if}
			</div>
		</section>
	</div>
{/if}
