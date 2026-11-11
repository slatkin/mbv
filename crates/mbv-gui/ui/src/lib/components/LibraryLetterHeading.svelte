<script lang="ts">
	// The letter heading of an alphabetical Library list (add-gui-library-panel
	// design D15): the one heading the Emby and Audiobooks lists paint between
	// letter groups. `attr` names the destination's own hook.
	interface Props {
		attr: string;
		letter: string;
		height: number;
	}
	const { attr, letter, height }: Props = $props();
</script>

<div
	class="flex items-end font-mono"
	style:height="{height}px"
	style:padding="0 8px 2px"
	style:font-size="12px"
	style:color="var(--canvas-source)"
	{...{ [`data-library-${attr}-letter`]: letter }}
>
	{letter}
</div>
