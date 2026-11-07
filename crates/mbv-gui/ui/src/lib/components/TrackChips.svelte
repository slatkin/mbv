<script lang="ts">
	// The track chips row under the transport (add-tauri-gui-client row
	// 6.4, design D4a): the video stream's resolved label as a label-only
	// chip (the D4a ruling gives the video chip no popover, so it is not a
	// button), and the audio and subtitle chips, each a button whose press
	// opens the core's track popover. The chips carry no data while the
	// local player is inactive — the chips' source is the local player's
	// stream — so the row paints nothing then, as the status-indicator
	// cluster does.
	//
	// The popover paints from the frame's `trackPopover` overlay — the
	// same projection the chips read, so its rows and the chips cannot
	// disagree — anchored above the chip it belongs to, the chosen choice
	// marked and the core's cursor row highlighted. The window holds no
	// open, cursor, or hover state; Esc and a press elsewhere close the
	// popover through the core's key and pointer paths. The subtitle
	// popover's Off row comes first in the frame's choice order; the front
	// end paints the order it is given. There is no subtitle-offset
	// control and no video popover (the D4a rulings).
	//
	// Region gap (dispatch 3): the wire's `QueuePlaybackTargetId` has no
	// chip or popover-row variant and the leaf has no arm resolving one
	// yet, so the chips and the popover rows report no region — a press
	// still forwards `pointer`, which resolves to nothing in the core (the
	// documented no-op the playlist pill's press already is). Reporting an
	// id the wire does not know would fail the whole `LayoutReport`, so
	// none is sent. Once the Rust ids land this row reports them through
	// the same region path the panel's other controls use.
	import { onMount } from 'svelte';
	import { AudioLines, Captions, Check, MonitorPlay } from '@lucide/svelte';
	import { reportLayout, retractLayout } from '$lib/bridge/layout';
	import { roleProperty } from '$lib/bridge/theme';
	import type {
		GuiOverlayFrame,
		GuiTrackChip,
		GuiTrackChips,
	} from '$lib/bridge/types';

	type TrackPopover = Extract<GuiOverlayFrame, { kind: 'trackPopover' }>;

	interface Props {
		chips: GuiTrackChips | null;
		/** The open menu or popover (design D4): the track popover paints
		 * above its chip while it is the open overlay. `null` while none is
		 * open. */
		overlay: GuiOverlayFrame | null;
	}
	const { chips, overlay }: Props = $props();

	const reportKey = Symbol('track-chips');

	/** The open track popover, or `null` while another overlay (or none) is
	 * up. Its `chip` fact picks the chip it anchors to. */
	const popover = $derived(overlay?.kind === 'trackPopover' ? overlay : null);

	/** The label a chip paints: the chosen choice's label; `Off` while the
	 * subtitle chip has no chosen choice (the popover's Off row is then the
	 * chosen one), the kind's name while the audio chip's choice is
	 * unresolved. */
	function chipLabel(chip: GuiTrackChip, kind: 'audio' | 'subtitles'): string {
		const chosen = chip.choices.find((choice) => choice.id === chip.chosen);
		return chosen?.label ?? (kind === 'subtitles' ? 'Off' : 'Audio');
	}

	/** The `var(--mbv-<role>)` value a part paints in. */
	function roleVar(role: string): string {
		return `var(${roleProperty(role)})`;
	}

	// The row reports no regions of its own (the gap the header comment
	// records): the mount-time report still registers the mounted surface's
	// empty contribution so the merged report accounts for it.
	$effect(() => {
		void chips;
		void overlay;
		reportLayout(reportKey, { lists: [], regions: [] });
	});

	onMount(() => () => retractLayout(reportKey));
</script>

{#snippet popoverList(pop: TrackPopover)}
	<!-- The track popover: the core's live choice list, the chosen choice
		marked with a check and the cursor row highlighted — the same row
		presentation the header's target chooser paints. -->
	<div
		class="absolute bottom-full left-0 z-20 mb-1 flex min-w-48 flex-col rounded-md border py-1 shadow-md"
		style:background="var(--mbv-popover)"
		style:border-color="var(--mbv-border)"
	>
		{#each pop.choices as choice, index (index)}
			<button
				type="button"
				class="flex items-center text-left"
				style:padding="calc(var(--mbv-cell-row) / 4) calc(var(--mbv-cell-col) * 1)"
				style:gap="var(--mbv-cell-col)"
				style:color={index === pop.cursor
					? roleVar('textFocusAccent')
					: roleVar('textPrimary')}
				data-popover-row={index}
			>
				<span class="flex w-4 shrink-0 justify-center">
					{#if choice.chosen}
						<Check size={14} color={roleVar('accent')} />
					{/if}
				</span>
				<span class="truncate">{choice.label}</span>
				{#if choice.forced}
					<span class="shrink-0 text-xs" style:color={roleVar('textMuted')}
						>forced</span
					>
				{/if}
			</button>
		{/each}
	</div>
{/snippet}

{#if chips}
	<!-- The chips row, centred under the transport as the canvas draws it:
		video label, audio chip, subtitle chip — one chip-height row in the
		chips' mono face with tabular figures (the spec's chips type rule). -->
	<div
		class="flex items-center justify-center font-mono text-xs tabular-nums"
		style:gap="calc(var(--mbv-cell-col) * 2)"
	>
		{#if chips.video}
			<!-- The video chip: the resolved stream label only — the D4a
				ruling gives it no popover, so it is a label, not a button. -->
			<span
				class="flex items-center rounded-full border px-2 py-0.5"
				style:gap="calc(var(--mbv-cell-col) / 2)"
				style:border-color="var(--mbv-border)"
				style:background="var(--mbv-secondary)"
				style:color={roleVar('textPrimary')}
				data-chip="video"
			>
				<MonitorPlay size={12} color={roleVar('indicatorResolutionFg')} />
				<span>{chips.video}</span>
			</span>
		{/if}
		<div class="relative">
			<button
				type="button"
				class="flex items-center rounded-full border px-2 py-0.5"
				style:gap="calc(var(--mbv-cell-col) / 2)"
				style:border-color="var(--mbv-border)"
				style:background="var(--mbv-secondary)"
				style:color={roleVar('textPrimary')}
				data-chip="audio"
			>
				<AudioLines size={12} color={roleVar('indicatorAudioFg')} />
				<span>{chipLabel(chips.audio, 'audio')}</span>
			</button>
			{#if popover?.chip === 'audio'}
				{@render popoverList(popover)}
			{/if}
		</div>
		<div class="relative">
			<button
				type="button"
				class="flex items-center rounded-full border px-2 py-0.5"
				style:gap="calc(var(--mbv-cell-col) / 2)"
				style:border-color="var(--mbv-border)"
				style:background="var(--mbv-secondary)"
				style:color={roleVar('textPrimary')}
				data-chip="subtitles"
			>
				<Captions size={12} color={roleVar('textMuted')} />
				<span>{chipLabel(chips.subtitles, 'subtitles')}</span>
			</button>
			{#if popover?.chip === 'subtitles'}
				{@render popoverList(popover)}
			{/if}
		</div>
	</div>
{/if}
