<script lang="ts">
	// One Settings toggle (add-gui-settings-screen design D13, the canvas
	// switch): a 44 by 24 px track with an 18 px knob, `role="switch"` and
	// `aria-checked` for assistive technology, and the accent fill while it is
	// on. A press runs the same store function the keyboard's Space runs, so
	// the two paths share one effect (design D11's pointer contract).
	interface Props {
		/** The toggle's accessible name. */
		label: string;
		/** Whether the toggle is on. */
		checked: boolean;
		/** Whether the toggle cannot be operated. */
		disabled?: boolean;
		/** A press on the track. */
		onpress: () => void;
	}
	const { label, checked, disabled = false, onpress }: Props = $props();
</script>

<button
	type="button"
	role="switch"
	aria-checked={checked}
	aria-label={label}
	class="relative flex-none"
	style:width="44px"
	style:height="24px"
	style:padding="0"
	style:border="0"
	style:border-radius="12px"
	style:background={checked ? 'var(--canvas-accent)' : 'var(--canvas-segment-active)'}
	style:transition="background 120ms"
	{disabled}
	data-settings-switch=""
	onclick={onpress}
>
	<span
		aria-hidden="true"
		class="absolute"
		style:top="3px"
		style:left={checked ? '23px' : '3px'}
		style:width="18px"
		style:height="18px"
		style:border-radius="9px"
		style:background={checked ? 'var(--canvas-ink)' : 'var(--canvas-text)'}
		style:transition="left 120ms"
	></span>
</button>
