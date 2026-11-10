<script lang="ts">
	// The Library Hero's artwork arm (add-gui-library-panel design D15, D14):
	// the canvas's 300 px box for a portrait or square cover, and the TUI's
	// layout for a landscape still — the artwork fills the Hero content width
	// at 16:9, because a 16:9 still shrunk into a square box is not the TUI's
	// hero. Every destination's Hero paints its art through this one component,
	// so the two arms cannot drift between tabs.
	//
	// `attr` names the destination's own attributes (`data-library-shows-hero-art`),
	// so a destination's tests keep their hook.
	import type { GuiLibraryArt } from '$lib/bridge/library-types';

	/** The artwork box policy's shape, as every Hero's frames carry it. */
	type HeroArtShape = 'landscape' | 'square' | 'portrait';

	interface Props {
		/** The artwork box the Hero reserves, from the artwork policy. */
		shape: HeroArtShape;
		/** The fetched artwork, or `null` while the item declares none. */
		art: GuiLibraryArt | null;
		/** The item's title: the image's alternative text. */
		title: string;
		/** The destination's own attribute prefix, as in `shows` or `emby`. */
		attr: string;
	}
	const { shape, art, title, attr }: Props = $props();

	/** The canvas's box width: the Music Hero's 300 px side, which the
	 * portrait and square arms keep. */
	const ART_WIDTH = 300;
	/** The box height per arm: the aspect the artwork shape gives a 300 px
	 * side. A landscape arm ignores it, painting the content width instead. */
	const ART_HEIGHT = { landscape: 169, square: 300, portrait: 450 } as const;

	/** Whether the arm paints the TUI's full-width still. */
	const band = $derived(shape === 'landscape');

	/** A `var(--canvas-<name>)` value a part paints in: the Library's canvas
	 * hexes, which name no theme slot. */
	function canvasVar(name: string): string {
		return `var(--canvas-${name})`;
	}
</script>

<div
	class="flex flex-none items-center justify-center"
	style:width={band ? '100%' : `${ART_WIDTH}px`}
	style:height={band ? 'auto' : `${ART_HEIGHT[shape]}px`}
	style:aspect-ratio={band ? '16 / 9' : undefined}
	style:background={canvasVar('library-raise')}
	style:border-radius="6px"
	style:overflow="hidden"
	{...{ [`data-library-${attr}-hero-art`]: art?.kind ?? 'none' }}
	{...{ [`data-library-${attr}-hero-art-shape`]: shape }}
>
	{#if art !== null && art.kind !== 'loading'}
		<img class="h-full w-full object-cover" src={art.detail} alt={title} />
	{:else}
		<span class="font-mono" style:font-size="12px" style:color={canvasVar('muted')}
			>[No art · the window's placeholder]</span
		>
	{/if}
</div>
