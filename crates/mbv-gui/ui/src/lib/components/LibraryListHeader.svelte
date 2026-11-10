<script lang="ts">
	// The Library list's header (add-gui-library-panel design D7, D14): the
	// canvas's list title beside the carried selection's label, and the `/`
	// filter field under both. Every destination paints this one header, so the
	// column cannot drift between tabs: a destination passes its title, the
	// label its core projection derived, and its own filter state.
	//
	// `titleAttr` and `labelAttr` name a destination's own attributes, so a
	// destination's tests keep a per-tab hook beside the shared ones.
	import LibraryFilterField from './LibraryFilterField.svelte';

	interface Props {
		/** The list's title, as the canvas names the column. */
		title: string;
		/** The destination's own title attribute, when it has one. */
		titleAttr?: string;
		/** The carried selection's label, painted on the title's line. */
		label: string;
		/** The destination's own label attribute, when it has one. */
		labelAttr?: string;
		/** The filter text the field paints. */
		filter: string;
		/** The field's placeholder, which names the selection it narrows. */
		placeholder: string;
		/** The field's accessible name. */
		ariaLabel: string;
		/** A keystroke in the field. */
		oninput: (value: string) => void;
		/** The field's accept, as Enter does. */
		onaccept: () => void;
	}
	const {
		title,
		titleAttr,
		label,
		labelAttr,
		filter,
		placeholder,
		ariaLabel,
		oninput,
		onaccept,
	}: Props = $props();

	/** The destination's own title attribute, when it has one. */
	const titleAttrs = $derived(titleAttr === undefined ? {} : { [titleAttr]: '' });

	/** The destination's own label attribute, when it has one. */
	const labelAttrs = $derived(labelAttr === undefined ? {} : { [labelAttr]: '' });
</script>

<div class="flex flex-none flex-col" style:padding="14px 16px 10px" style:gap="10px">
	<div class="flex items-baseline justify-between" style:gap="12px">
		<span
			style:font-size="13px"
			style:font-weight="600"
			style:color="var(--canvas-heading)"
			data-library-list-title=""
			{...titleAttrs}>{title}</span
		>
		<span
			class="font-mono"
			style:font-size="11px"
			style:color="var(--canvas-muted)"
			data-library-selection-label=""
			{...labelAttrs}>{label}</span
		>
	</div>
	<LibraryFilterField
		value={filter}
		{placeholder}
		{ariaLabel}
		{oninput}
		{onaccept}
	/>
</div>
