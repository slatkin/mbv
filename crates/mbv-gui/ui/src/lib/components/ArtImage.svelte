<script lang="ts">
	// The window's one art painter (gui-change-driven-sections design D7): every
	// `GuiArtRef` paints as an `<img>` whose `src` is the reference's URL. A
	// `loading` reference paints no image, and an `img` error paints the same
	// loading fill. The fill then retries the same URL with capped backoff while
	// mounted: the `mbv-art` 404 is `no-store`, so a retry asks the core again
	// (design D7). A new URL starts a fresh load.
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
		/** The loading fill: painted while the reference has no URL, the image
		 * fails to load, or its retry waits. */
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

	/** The first retry's wait; each further failure doubles it. */
	const RETRY_BASE_MS = 1000;
	/** The longest wait between two retries of one URL. */
	const RETRY_CAP_MS = 30_000;

	/** The URL the reference names, `null` while it names none. */
	const src = $derived(art !== null && art.kind !== 'loading' ? art.src : null);

	/** The load that failed: its URL, how many times it failed in a row, and
	 * whether its retry still waits. */
	let failure = $state<{ src: string; count: number; waiting: boolean } | null>(null);
	let retryTimer: ReturnType<typeof setTimeout> | undefined;

	// A new URL starts a fresh load: drop the old failure and its pending retry.
	// Unmount runs the same cleanup, so no retry outlives the box.
	$effect(() => {
		void src;
		failure = null;
		return () => clearTimeout(retryTimer);
	});

	/** The URL to paint, `null` while the fill paints. */
	const painted = $derived(
		src !== null && !(failure?.waiting && failure.src === src) ? src : null,
	);

	/** The wait before the retry after the `count`-th failure in a row. */
	function retryDelay(count: number): number {
		return Math.min(RETRY_BASE_MS * 2 ** (count - 1), RETRY_CAP_MS);
	}

	/** Record a failed load of `url` and schedule its retry: the fill paints
	 * until the timer clears the wait, then the image element is recreated and
	 * the webview requests the URL again. */
	function onError(url: string) {
		const count = (failure?.src === url ? failure.count : 0) + 1;
		failure = { src: url, count, waiting: true };
		clearTimeout(retryTimer);
		retryTimer = setTimeout(() => {
			failure = { src: url, count, waiting: false };
		}, retryDelay(count));
	}
</script>

{#if painted !== null}
	<img
		class={className}
		{style}
		src={painted}
		{alt}
		use:capToNaturalSize={bounds}
		onerror={() => onError(painted)}
	/>
{:else if fallback}
	{@render fallback()}
{/if}
