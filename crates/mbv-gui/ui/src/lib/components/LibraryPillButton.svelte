<script lang="ts">
	// The one pill button every Library selector paints (add-gui-library-panel
	// design D6, D15): the top pill rows and the Shows Hero's season pills. A
	// destination supplies the pill's name, its selected and focused state, its
	// own attributes, and the press; it never restates the pill's look, so the
	// two selectors cannot drift.
	import { roleVar } from '$lib/bridge/theme';

	interface Props {
		name: string;
		/** Whether the core carries the pill as the selected one. */
		selected: boolean;
		/** Whether the keys' focus holds the pill, which paints the accent
		 * outline (design D6). */
		focused?: boolean;
		/** The destination's own hook attributes for this pill. */
		attrs?: Record<string, string | undefined>;
		onpress: () => void;
	}
	const { name, selected, focused = false, attrs = {}, onpress }: Props = $props();
</script>

<button
	type="button"
	class="flex flex-none items-center whitespace-nowrap"
	style:height="32px"
	style:gap="5px"
	style:padding="0 12px"
	style:border-radius="16px"
	style:font-size="14px"
	style:font-weight={selected ? 600 : 400}
	style:border={selected ? '1px solid var(--canvas-source)' : '1px solid var(--canvas-border)'}
	style:background={selected ? 'var(--canvas-source)' : 'none'}
	style:color={selected ? 'var(--canvas-ink)' : 'var(--canvas-text)'}
	style:outline={focused ? `2px solid ${roleVar('accentActive')}` : 'none'}
	style:outline-offset="1px"
	aria-pressed={selected}
	{...attrs}
	onclick={onpress}
>
	{name}
</button>
