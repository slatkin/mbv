<script lang="ts" generics="Row extends GeometryRow">
	// The Library list's scroll shell (add-gui-library-panel design D7, D12):
	// the one windowed list every destination paints — the fixed-height rows
	// over native scroll, the spacers that hold the scroll range of the rows
	// outside the window, and the two facts only the DOM holds, the measured
	// viewport and the native scroll offset, which it reports to the
	// destination's own state.
	//
	// A destination supplies its rows, the two state values it owns (design D1),
	// its attribute prefix, and a row snippet. Nothing here knows a destination,
	// so a change to measurement or to the window reporting lands once.
	import { onMount } from 'svelte';
	import type { Snippet } from 'svelte';
	import { listWindow, rowsHeight } from '$lib/library/list';
	import type { GeometryRow } from '$lib/library/list';

	interface Props {
		/** The painted rows of the destination's model. */
		rows: readonly Row[];
		/** The scroll offset the destination's state holds, in pixels. */
		scrollTop: number;
		/** The viewport height the destination's state holds, in pixels. */
		viewportHeight: number;
		/** The destination's own attribute prefix, as in `podcasts` or `abs`. */
		attr: string;
		/** The scroller's padding, where a destination's rail narrows the rows. */
		padding?: string;
		/** Report the measured viewport height, so the window follows a resize. */
		onmeasure: (height: number) => void;
		/** Report the element's native scroll offset, so a user scroll follows. */
		onscroll: (top: number) => void;
		/** One painted row, under the shell's own list item. */
		row: Snippet<[Row]>;
	}
	const {
		rows,
		scrollTop,
		viewportHeight,
		attr,
		padding = '8px 8px 8px 8px',
		onmeasure,
		onscroll,
		row: paint,
	}: Props = $props();

	let listEl: HTMLElement | undefined = $state();

	/** The rows in view: only these paint. */
	const view = $derived(listWindow(rows, scrollTop, viewportHeight));
	const painted = $derived(rows.slice(view.start, view.end));

	/** The list's whole painted height, and the spacers that hold its scroll
	 * range while only the rows in view paint. */
	const total = $derived(rowsHeight(rows));
	const above = $derived(painted.length > 0 ? painted[0].top : 0);
	const below = $derived(total - (painted.length > 0 ? lastBottom() : above));

	/** The bottom edge of the last painted row: the below-spacer's pair. */
	function lastBottom(): number {
		const last = painted[painted.length - 1];
		return last === undefined ? 0 : last.top + last.height;
	}

	/** Report the list's measured height: the rows in view follow it. */
	function measure(): void {
		onmeasure(listEl?.clientHeight ?? 0);
	}

	/** Report the list's native scroll offset, so the windowed paint follows a
	 * user scroll. */
	function handleScroll(event: Event): void {
		onscroll((event.currentTarget as HTMLElement).scrollTop);
	}

	// The scroller mirrors the state: a key that moved the cursor set the
	// offset, and the element follows it. A user scroll already wrote the offset
	// back, so this assignment is a no-op for it.
	$effect(() => {
		const el = listEl;
		if (el !== undefined && el.scrollTop !== scrollTop) {
			el.scrollTop = scrollTop;
		}
	});

	onMount(() => {
		measure();
		const observer =
			typeof ResizeObserver === 'undefined' ? undefined : new ResizeObserver(measure);
		if (listEl !== undefined) {
			observer?.observe(listEl);
		}
		return () => observer?.disconnect();
	});
</script>

<ol
	bind:this={listEl}
	class="min-h-0 flex-1"
	style:overflow-y="auto"
	style:list-style="none"
	style:margin="0"
	style:padding
	{...{ [`data-library-${attr}-list`]: '' }}
	onscroll={handleScroll}
>
	<li style:height="{above}px" aria-hidden="true"></li>
	{#each painted as paintedRow (paintedRow.id)}
		<li>{@render paint(paintedRow)}</li>
	{/each}
	<li style:height="{below}px" aria-hidden="true"></li>
</ol>
