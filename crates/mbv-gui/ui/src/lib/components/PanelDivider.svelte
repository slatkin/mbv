<script lang="ts">
	// A column divider (add-gui-library-panel design D4): the Queue panel's
	// column and the Wide arrangement's list column are resized by dragging.
	// The drag reports the sized column's new width in pixels and the session
	// keeps it (`$lib/bridge/resize`), so this component holds nothing beyond
	// the drag itself. A double-click reports the reset to the default width,
	// and the pointer affordance is the column-resize cursor over a band that
	// lights while hovered or dragged.
	//
	// The window listens for the drag's moves: the pointer leaves the band's
	// few pixels as soon as the drag starts. The sized column sits on one side
	// of the divider, so a drag right widens a right column and narrows a left
	// one.
	import { DIVIDER_WIDTH_PX } from '$lib/bridge/resize';
	interface Props {
		/** The side of the divider the sized column sits on. */
		side: 'left' | 'right';
		/** The sized column's width when a drag starts, in pixels. */
		width: number;
		/** What the divider resizes, for the accessible name and the tests. */
		name: 'queue' | 'list';
		/** A drag's new width for the sized column, in pixels. */
		onDrag: (width: number) => void;
		/** A double-click: the sized column returns to its default width. */
		onReset: () => void;
	}
	let { side, width, name, onDrag, onReset }: Props = $props();
	const label = $derived(
		name === 'queue' ? 'Resize the Queue panel' : 'Resize the Library list',
	);

	/** Whether a drag is under way: the band paints its drag look. */
	let dragging = $state(false);

	/** The live drag's teardown, so a divider that unmounts mid-drag (a
	 * rearrangement or the collapse toggle) leaves no listener behind. */
	let stopDrag: (() => void) | null = null;

	$effect(() => () => stopDrag?.());

	function startDrag(event: PointerEvent): void {
		if (event.button !== 0) {
			return;
		}
		event.preventDefault();
		dragging = true;
		const origin = event.clientX;
		const startWidth = width;
		const sign = side === 'left' ? 1 : -1;
		const move = (event: PointerEvent) => {
			onDrag(startWidth + sign * (event.clientX - origin));
		};
		const stop = () => {
			dragging = false;
			stopDrag = null;
			window.removeEventListener('pointermove', move);
			window.removeEventListener('pointerup', stop);
			window.removeEventListener('pointercancel', stop);
		};
		stopDrag = stop;
		window.addEventListener('pointermove', move);
		window.addEventListener('pointerup', stop);
		window.addEventListener('pointercancel', stop);
	}
</script>

<button
	type="button"
	aria-label={label}
	data-divider={name}
	data-divider-dragging={dragging ? '' : undefined}
	class="relative flex-none self-stretch border-0 p-0 hover:bg-[var(--canvas-border)] data-[divider-dragging]:bg-[var(--canvas-border)]"
	style:width="{DIVIDER_WIDTH_PX}px"
	style:cursor="col-resize"
	onpointerdown={startDrag}
	ondblclick={onReset}
></button>
