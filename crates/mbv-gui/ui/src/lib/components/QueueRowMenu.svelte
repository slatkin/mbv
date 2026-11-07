<script lang="ts">
	// The queue row's menu (add-tauri-gui-client row 6.5, design D15): the
	// frame's `rowMenu` overlay entries, each with the key hint that
	// selects it directly, and the core's cursor row highlighted. The
	// window holds no open, cursor, or hover state; a press on an entry
	// forwards like every other pointer event and the core resolves it
	// against the hit map, Esc and the hint chords resolve in the core's
	// overlay key rules.
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
	import type { GuiRowMenuEntry } from '$lib/bridge/types';

	interface Props {
		/** The menu cursor the core holds. */
		cursor: number;
		/** The overlay's entries, in the frame's order. */
		entries: GuiRowMenuEntry[];
		/** The menu's top edge as a CSS length, measured from the containing
		 * block's top edge; the parent places it over the anchor row's
		 * bottom edge, as the canvas draws it. */
		top: string;
	}
	const { cursor, entries, top }: Props = $props();

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
	style:top
	style:right="var(--mbv-cell-col)"
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
			data-menu-entry={index}
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
