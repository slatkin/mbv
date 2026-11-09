<script lang="ts">
	// The Library Status bar (add-gui-library-panel design D10): one 40 px band
	// under the Library column, in the canvas's status-bar form — the canvas's
	// mono type, 16 px side padding, its top border, and its list-column fill.
	//
	// The left side is the canvas's fixed key hints: the GUI's own keys (design
	// D3), which `[keys]` does not configure, so the text is fixed. The right
	// side is the location: the destination, the selected collection names, and
	// the picked item's artist or title (`$lib/library/status`), joined by the
	// canvas's separator.
	//
	// The frame's `GuiStatusBar` carries the armed prefix and the live toast,
	// the same facts the Queue footer's line paints. While either is active it
	// replaces the hints — the toast wins over the prefix badge, as the footer
	// line drops the badge while a toast is live — and the location stays. The
	// band paints no interactive region: a point on it resolves to no target,
	// as the canvas's bar carries none.
	import { toastColor } from '$lib/bridge/status';
	import type {
		GuiHomeFrame,
		GuiLibraryTab,
		GuiMusicFrame,
		GuiStatusBar,
	} from '$lib/bridge/types';
	import { libraryLocation } from '$lib/library/status';

	interface Props {
		/** The presented destination tabs and the active one (design D2): the
		 * location's first segment. While the Home content is present the
		 * location's segments come from it. */
		tabs: GuiLibraryTab[];
		currentTab: number | null;
		/** The carried Music content (design D2): the selected collections and
		 * the picked item's Hero. */
		music: GuiMusicFrame | null;
		/** The carried Home content (design D12): the location reads `Home` on
		 * it. */
		home?: GuiHomeFrame | null;
		/** The armed prefix and the live toast (design D10): they replace the
		 * hints while active. */
		status: GuiStatusBar;
	}
	const { tabs, currentTab, music, home = null, status }: Props = $props();

	/** The canvas's key hints, key glyph then what the key does (design D3,
	 * D10). */
	const HINTS: readonly { key: string; label: string }[] = [
		{ key: '/', label: 'filter' },
		{ key: '⏎', label: 'play' },
		{ key: 'n', label: 'next' },
		{ key: 'a', label: 'queue' },
		{ key: 'tab', label: 'pane' },
		{ key: '1–7', label: 'library' },
	];

	/** The location line, or `''` while the content fills no segment. On Home
	 * it reads the destination's one-word name, then the selected row
	 * (design D12). */
	const location = $derived(libraryLocation(tabs, currentTab, music, home));
</script>

<!-- The canvas status bar: 40 px, 16 px side padding, the canvas's 1 px top
	border and list-column fill, the hints at the left edge and the location at
	the right. -->
<div
	class="flex flex-none items-center justify-between overflow-hidden font-mono whitespace-nowrap"
	style:height="40px"
	style:padding-inline="16px"
	style:font-size="12px"
	style:border-top="1px solid var(--canvas-border)"
	style:background="var(--canvas-library-panel)"
	data-library-status=""
>
	<!-- The left side: the armed prefix or the live toast replaces the hints
		while either is active; the hints paint otherwise, each key in the
		canvas's teal and its action in the muted line colour. -->
	<span class="overflow-hidden" data-library-status-hints="">
		{#if status.toast}
			<span style:color={toastColor(status.toast.severity)} data-library-status-toast=""
				>{status.toast.text}</span
			>
		{:else if status.prefixArmed}
			<span class="font-bold" style:color="var(--mbv-accent)" data-library-status-prefix=""
				>PREFIX</span
			>
		{:else}
			<!-- Each pair is a key glyph and its action, spaced as the canvas's
				run of hints. -->
			<span class="flex items-center" style:gap="14px" style:color="var(--canvas-muted)">
				{#each HINTS as hint (hint.key)}
					<span data-library-hint=""
						><span style:color="var(--canvas-source)">{hint.key}</span> {hint.label}</span
					>
				{/each}
			</span>
		{/if}
	</span>
	<!-- The location, flush at the right edge: the destination, the selected
		collections, and the picked item, joined by the canvas's separator. -->
	<span class="overflow-hidden" style:color="var(--canvas-muted)" data-library-status-location=""
		>{location}</span
	>
</div>
