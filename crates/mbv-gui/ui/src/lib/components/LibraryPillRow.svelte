<script lang="ts">
	// The Library pill row (add-gui-library-panel design D6, D12): the canvas's
	// single-select pill form, which every destination that has a selector row
	// paints — the Music collections, the Shows modes, the Emby library's
	// folders, the Podcasts views, and the Feeds views. The row is one base
	// component so a destination cannot invent a second pill look: each
	// destination passes its carried pills, the painted choice comes from the
	// core, and a press reports the pill's stable key to that destination's
	// command.
	//
	// `pillAttr` names the destination's own attribute (`'shows'` paints
	// `data-library-shows-pill`), so a destination's tests keep a per-tab hook
	// beside the shared `data-library-pill`.
	import { roleVar } from '$lib/bridge/theme';

	/** One painted pill: its stable key (the press names it), its name, and
	 * whether the core carries it as the selected one. */
	export interface LibraryPill {
		key: string;
		name: string;
		selected: boolean;
	}

	interface Props {
		/** The row's accessible name, naming what the pills choose between. */
		ariaLabel: string;
		/** The pills in painted order. */
		pills: readonly LibraryPill[];
		/** The pill the keys' focus holds, which paints the accent outline
		 * (design D6), or `null` while no pill has it. */
		focused: string | null;
		/** The destination's own pill attribute suffix, or `undefined` where
		 * the shared `data-library-pill` is enough. */
		pillAttr?: string;
		/** Press a pill: the destination sends its command. */
		onpress: (key: string) => void;
	}
	const { ariaLabel, pills, focused, pillAttr, onpress }: Props = $props();

	/** The destination's own pill attribute, when it has one. */
	function tabAttr(pill: LibraryPill): Record<string, string> {
		return pillAttr === undefined ? {} : { [`data-library-${pillAttr}-pill`]: pill.key };
	}
</script>

<div
	class="flex flex-none items-center overflow-hidden"
	role="group"
	aria-label={ariaLabel}
	data-library-region="pills"
	style:height="52px"
	style:gap="6px"
	style:padding="0 16px"
	style:border-bottom="1px solid var(--canvas-border)"
>
	{#each pills as pill (pill.key)}
		<button
			type="button"
			class="flex flex-none items-center whitespace-nowrap"
			style:height="32px"
			style:gap="5px"
			style:padding="0 12px"
			style:border-radius="16px"
			style:font-size="14px"
			style:font-weight={pill.selected ? 600 : 400}
			style:border={pill.selected
				? '1px solid var(--canvas-source)'
				: '1px solid var(--canvas-border)'}
			style:background={pill.selected ? 'var(--canvas-source)' : 'none'}
			style:color={pill.selected ? 'var(--canvas-ink)' : 'var(--canvas-text)'}
			style:outline={pill.key === focused ? `2px solid ${roleVar('accentActive')}` : 'none'}
			style:outline-offset="1px"
			aria-pressed={pill.selected}
			{...tabAttr(pill)}
			data-library-pill={pill.key}
			data-library-pill-focused={pill.key === focused ? '' : undefined}
			onclick={() => onpress(pill.key)}
		>
			{pill.name}
		</button>
	{/each}
</div>
