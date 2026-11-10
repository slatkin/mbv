<script lang="ts">
	// A Library list's one-line notice (add-gui-library-panel design D5, D7,
	// D14): the placeholder a list paints above its rows while it loads, when
	// its fetch failed, and when it holds nothing. Every destination paints it
	// in the same look, so the three states cannot drift between tabs.
	//
	// `attr` names the destination's own attribute (`data-library-shows-empty`),
	// so a destination's tests keep their hook.
	interface Props {
		/** The line's text. */
		text: string;
		/** The destination's own attribute name, which its tests read. */
		attr: string;
	}
	const { text, attr }: Props = $props();
</script>

<div
	class="flex flex-none items-center"
	style:height="36px"
	style:padding="0 16px"
	style:font-size="14px"
	style:color="var(--canvas-muted)"
	{...{ [attr]: '' }}
>
	{text}
</div>
