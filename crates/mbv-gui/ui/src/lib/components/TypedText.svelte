<script lang="ts">
	// Typed text (add-gui-settings-screen design D14): colour carries the data
	// type, so a line that joins values of several types is built as typed
	// segments where it is built and painted here, never split in the window.
	// Each kind names one `--canvas-type-*` token; the caller keeps the weight,
	// the size, and the role of the line, and a plain segment inherits them.
	//
	// A list repeats the same types down its rows, so the caller sets
	// `repeat` and a time or a date paints at the repeat opacity. A row that
	// is selected or current keeps its own ink at full strength, so it paints
	// without `repeat`.
	import type { GuiMetaKind, GuiMetaSegment } from '$lib/bridge/library-types';

	interface Props {
		/** The line's typed runs, in display order. */
		segments: GuiMetaSegment[];
		/** Whether the line repeats down a list: a time or a date then paints at
		 * the repeat opacity. */
		repeat?: boolean;
	}
	const { segments, repeat = false }: Props = $props();

	/** The token each kind paints; a plain run inherits the line's own colour. */
	const TOKENS: Record<GuiMetaKind, string> = {
		plain: 'inherit',
		time: 'var(--canvas-type-time)',
		date: 'var(--canvas-type-date)',
		count: 'var(--canvas-type-count)',
		label: 'var(--canvas-type-label)',
		secondary: 'var(--canvas-type-secondary)',
		role: 'var(--canvas-type-role)',
	};

	/** Whether one segment paints at the repeat opacity: only a repeated time or
	 * date fades (design D14). */
	function repeats(segment: GuiMetaSegment): boolean {
		return repeat && (segment.kind === 'time' || segment.kind === 'date');
	}
</script>

{#each segments as segment, index (index)}<span
		data-meta-kind={segment.kind}
		style:color={TOKENS[segment.kind]}
		style:opacity={repeats(segment) ? 'var(--canvas-type-repeat-opacity)' : undefined}
		>{segment.text}</span
	>{/each}
