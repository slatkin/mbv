<script lang="ts">
	// The queue row's menu (add-tauri-gui-client row 6.5, design D15): the
	// frame's `rowMenu` overlay entries, each with the key hint that
	// selects it directly, and the core's cursor row highlighted. The
	// window holds no open, cursor, or hover state; a press on an entry
	// forwards like every other pointer event and the core resolves it
	// against the hit map, Esc and the hint chords resolve in the core's
	// overlay key rules.
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
	import { roleProperty } from '$lib/bridge/theme';
	import type { GuiRowMenuEntry } from '$lib/bridge/types';

	interface Props {
		/** The menu cursor the core holds. */
		cursor: number;
		/** The overlay's entries, in the frame's order. */
		entries: GuiRowMenuEntry[];
		/** The menu's top edge as a CSS length, measured from the containing
		 * block's top edge; the parent places it under the anchor row. */
		top: string;
	}
	const { cursor, entries, top }: Props = $props();

	/** The `var(--mbv-<role>)` value a part paints in. */
	function roleVar(role: string): string {
		return `var(${roleProperty(role)})`;
	}
</script>

<div
	class="absolute z-20 flex min-w-52 flex-col rounded-md border py-1 shadow-md"
	style:top
	style:right="var(--mbv-cell-col)"
	style:background="var(--mbv-popover)"
	style:border-color="var(--mbv-border)"
	data-row-menu
>
	{#each entries as entry, index (index)}
		<button
			type="button"
			class="flex items-center text-left"
			style:padding="calc(var(--mbv-cell-row) / 4) calc(var(--mbv-cell-col) * 1)"
			style:gap="calc(var(--mbv-cell-col) * 2)"
			style:color={index === cursor ? roleVar('textFocusAccent') : roleVar('textPrimary')}
			data-menu-entry={index}
		>
			<span class="flex-1 truncate">{entry.label}</span>
			{#if entry.keyHint}
				<span
					class="shrink-0 font-mono text-xs tabular-nums"
					style:color={roleVar('textMuted')}>{entry.keyHint}</span
				>
			{/if}
		</button>
	{/each}
</div>
