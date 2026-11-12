<script lang="ts">
	// The queue row's menu (add-tauri-gui-client row 6.5, design D15): the
	// frame's `rowMenu` overlay entries, each with the key hint that selects it
	// directly, and the core's cursor row highlighted. The window holds no open,
	// cursor, or hover state; a press on an entry forwards like every other
	// pointer event and the core resolves it against the hit map, Esc and the
	// hint chords resolve in the core's overlay key rules. The Library Hero's
	// item menu paints the same face (add-gui-library-panel design D8): its
	// entries are front-end state, so it passes `onSelect` and a press sends the
	// menu-entry command instead of reaching the core's hit map.
	//
	// The menu paints the Menus artboard's chrome (align-gui-with-design-
	// canvas B.2): a 230 px card over its anchor row's bottom edge, the
	// canvas's fill, border, radius, shadow, 6 px padding and 2 px gap, 34 px
	// entries at 13 px, and the muted mono hints. The destructive entry (the
	// projected flag, design DR6) paints in the canvas's tone under a
	// divider. The hint glyph is a paint-time map (design DR7): the core
	// keeps the raw `Enter` chord that `direct_select` matches.
	//
	// The component is presentation-only: `QueuePanel` measures the
	// `[data-menu-entry]` rows and reports their `rowMenuEntry` regions
	// under its own second report key, inserted when the menu opens — the
	// same technique `TrackChips` uses for its popover rows — so the
	// entries land after the queue's regions in the merged report and win
	// the hit map where the menu paints over a queue row beneath it (the
	// hit map's last push wins). Reporting from the parent, not from this
	// child's mount effect, keeps that order even when the first painted
	// frame already carries the open menu: a child's mount effect runs
	// before the parent's, so a child-held key would land first and the
	// entries would lose to the rows beneath them.

	interface Props {
		/** The menu cursor the core holds; `-1` while no entry carries it. */
		cursor: number;
		/** The overlay's entries, in the frame's order: the queue's entries
		 * carry the destructive flag, the Library's carry none. An entry's own
		 * `index` is the core's shared-builder order, carried so a front end
		 * that paints only some entries (the Hero's More actions) still sends
		 * the index of the entry it painted. */
		entries: {
			label: string;
			keyHint: string | null;
			destructive?: boolean;
			index?: number;
		}[];
		/** The menu's top edge as a CSS length, measured from the containing
		 * block's top edge; the parent places it over the anchor row's
		 * bottom edge, as the canvas draws it. */
		top: string;
		/** The menu's left edge from the containing block's left edge, for a
		 * menu the front end anchors at a pointer position (the Library's
		 * right-click item menu). When it is set, `right` stays unset. */
		left?: string;
		/** The menu's right edge from the containing block's right edge: the
		 * queue anchors it to its value column, the Library's to its actions
		 * row. */
		right?: string;
		/** The press on an entry, for a menu the front end owns: the Library's
		 * item menu sends its own command. The queue's menu passes none, so its
		 * presses reach the core's hit map as every other pointer event does.
		 * The index a press reports is the entry's own `index` when it carries
		 * one (the core's shared-builder order), else this enumeration — so a
		 * front end that paints only some entries still sends the index of the
		 * entry it painted. */
		onSelect?: (index: number) => void;
		/** The menu's accessible name; a front-end menu names its own list (the
		 * Settings pick's values). */
		label?: string;
	}
	const {
		cursor,
		entries,
		top,
		left,
		right = 'var(--mbv-cell-col)',
		onSelect,
		label = 'Row actions',
	}: Props = $props();

	/** The painted hint: the canvas draws `⏎` where the core's hint chord is
	 * the `Enter` key name. The map is paint-only, so the core's
	 * `direct_select` keeps matching the raw `Enter` string and menu key
	 * routing follows the configured bindings unchanged (design DR7). */
	function paintHint(hint: string): string {
		return hint === 'Enter' ? '⏎' : hint;
	}
</script>

<div
	class="absolute z-20 flex flex-col"
	role="menu"
	aria-label={label}
	style:top
	style:left
	style:right
	style:width="230px"
	style:padding="6px"
	style:gap="2px"
	style:background="var(--canvas-menu)"
	style:border="1px solid var(--canvas-menu-border)"
	style:border-radius="8px"
	style:box-shadow="var(--canvas-menu-shadow)"
	data-row-menu
>
	{#each entries as entry, index (index)}
		{#if entry.destructive}
			<!-- The canvas's divider above the destructive entry. -->
			<div
				style:height="1px"
				style:margin="4px 0"
				style:background="var(--canvas-menu-border)"
				data-menu-divider
			></div>
		{/if}
		<button
			type="button"
			class="flex items-center justify-between text-left"
			role="menuitem"
			style:height="34px"
			style:padding="0 10px"
			style:border-radius="4px"
			style:font-size="13px"
			style:background={index === cursor ? 'var(--canvas-selected)' : 'none'}
			style:color={entry.destructive
				? 'var(--canvas-destructive)'
				: index === cursor
					? 'var(--canvas-heading)'
					: 'var(--canvas-text)'}
			data-menu-entry={entry.index ?? index}
			onclick={() => onSelect?.(entry.index ?? index)}
		>
			<span class="flex-1 truncate">{entry.label}</span>
			{#if entry.keyHint}
				<span
					class="shrink-0 font-mono tabular-nums"
					style:font-size="11px"
					style:color="var(--canvas-muted)">{paintHint(entry.keyHint)}</span
				>
			{/if}
		</button>
	{/each}
</div>
