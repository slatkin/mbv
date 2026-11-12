<script lang="ts">
	// The window's one art painter (gui-change-driven-sections design D7): every
	// `GuiArtRef` paints as an `<img>` whose `src` is the reference's URL. A
	// `loading` reference paints no image, and an `img` error paints the same
	// loading fill, so each caller's box shows its own fill in both cases.
	import type { Snippet } from 'svelte';
	import { capToNaturalSize, type NaturalSizeBounds } from '$lib/bridge/natural-size';
	import type { GuiArtRef } from '$lib/bridge/art';

	interface Props {
		/** The reference the section carries, or `null` while the item has none. */
		art: GuiArtRef | null;
		/** The image's alternative text; `""` for decorative art. */
		alt: string;
		/** The box the image stays inside, as `capToNaturalSize` takes it. */
		bounds?: NaturalSizeBounds;
		/** The `<img>` class names. */
		class?: string;
		/** The `<img>` inline style. */
		style?: string;
		/** The loading fill: painted while the reference has no URL or the
		 * image fails to load. */
		fallback?: Snippet;
	}
	const {
		art,
		alt,
		bounds,
		class: className = '',
		style,
		fallback,
	}: Props = $props();

	/** The URL the reference names, `null` while it names none. */
	const src = $derived(art !== null && art.kind !== 'loading' ? art.src : null);

	/** The URL the webview failed to load. The fill paints while it is the
	 * current URL; a new reference loads afresh. */
	let failed = $state<string | null>(null);

	/** The URL to paint, `null` while the fill paints. */
	const painted = $derived(src !== null && src !== failed ? src : null);
</script>

{#if painted !== null}
	<img
		class={className}
		{style}
		src={painted}
		{alt}
		use:capToNaturalSize={bounds}
		onerror={() => (failed = painted)}
	/>
{:else if fallback}
	{@render fallback()}
{/if}
