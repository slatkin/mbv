<script lang="ts">
	// The list's `/` filter field (add-gui-library-panel design D3, step 2,
	// D7): the panel's open text field, in the one look every Library list
	// paints it — the `/` glyph and the input in the canvas's bordered field.
	// The key router types into it without checking focus, so the field owns
	// its focus: it takes the keyboard while the filter is open, closes the
	// filter when it loses focus keeping the text as Enter does, and reports
	// the focus and text through its props, because each destination's list
	// holds its own filter text.
	import { libraryUi, openLibraryFilter } from '$lib/library/state';

	interface Props {
		/** The field's current text: the destination's own filter text. */
		value: string;
		/** The placeholder, naming what the list holds. */
		placeholder: string;
		/** The input's accessible name. */
		ariaLabel: string;
		/** A text change: the destination's set narrows its rows and follows the
		 * cursor. */
		oninput: (value: string) => void;
		/** The close that keeps the text: the blur's accept, as Enter's. */
		onaccept: () => void;
	}
	const { value, placeholder, ariaLabel, oninput, onaccept }: Props = $props();

	let filterEl: HTMLInputElement | undefined = $state();

	// The field owns its focus (design D3, step 2): it takes the keyboard
	// while the filter is open and gives it back when the filter closes.
	$effect(() => {
		const open = $libraryUi.filterOpen;
		const el = filterEl;
		if (el === undefined) {
			return;
		}
		if (open && document.activeElement !== el) {
			el.focus();
		}
		if (!open && document.activeElement === el) {
			el.blur();
		}
	});
</script>

<label
	class="flex flex-none items-center"
	style:height="32px"
	style:padding="0 10px"
	style:gap="8px"
	style:background="var(--canvas-library-column)"
	style:border="1px solid var(--canvas-border)"
	style:border-radius="6px"
	style:color="var(--canvas-muted)"
>
	<span class="font-mono" style:font-size="13px" style:color="var(--canvas-source)">/</span>
	<input
		bind:this={filterEl}
		type="text"
		class="min-w-0 flex-1"
		style:background="none"
		style:border="0"
		style:outline="none"
		style:color="var(--canvas-heading)"
		style:font-size="14px"
		{placeholder}
		aria-label={ariaLabel}
		{value}
		data-library-filter=""
		onfocus={() => openLibraryFilter()}
		onblur={onaccept}
		oninput={(event) => oninput(event.currentTarget.value)}
	/>
</label>
