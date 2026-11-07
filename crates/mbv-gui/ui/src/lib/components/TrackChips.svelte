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
	// disagree — anchored below the chip it belongs to and right-aligned
	// (design DR8), the chosen choice filled and checked and the core's
	// cursor row highlighted. The dialog paints the Menus artboard's chrome
	// (B.4). The window holds no open, cursor, or hover state; Esc and a
	// press elsewhere close the popover through the core's key and pointer
	// paths. The subtitle popover's Off row comes first in the frame's
	// choice order; the front end paints the order it is given. There is
	// no subtitle-offset control and no video popover (the D4a rulings).
	//
	// The audio chip reports `audioChip` and the subtitle chip
	// `subtitleChip`; the video chip is label-only (the D4a ruling) and
	// reports no region. The open popover's rows report their
	// `trackPopoverChoice` indices under a second report key, first
	// inserted when the popover opens: a child component's mount effect
	// runs before the panel's, so the chips' key lands before the
	// transport's regions in the merged report, and only a key inserted
	// after them puts the rows last — the hit map's last push wins where
	// a row paints over the transport band beneath it. A press on a chip
	// or a row forwards `pointer` like every other pointer event; the
	// core resolves it against the hit map.
	import { onMount } from 'svelte';
	import { AudioLines, Captions, Check, MonitorPlay } from '@lucide/svelte';
	import { reportLayout, retractLayout } from '$lib/bridge/layout';
	import { roleProperty } from '$lib/bridge/theme';
	import type {
		GuiOverlayFrame,
		GuiTrackChip,
		GuiTrackChips,
		LayoutRegion,
		QueuePlaybackTargetId,
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
	const popoverKey = Symbol('track-popover');
	let rowEl: HTMLElement | undefined = $state();
	let audioChipEl: HTMLElement | undefined = $state();
	let subtitleChipEl: HTMLElement | undefined = $state();

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

	/** The `var(--canvas-<name>)` value a part paints in: the queue's canvas
	 * hexes that name no theme slot (design DR9). */
	function canvasVar(name: string): string {
		return `var(--canvas-${name})`;
	}

	/** One element's painted region, `null` while the element is absent. */
	function region(
		el: HTMLElement | undefined,
		id: QueuePlaybackTargetId,
	): LayoutRegion | null {
		if (!el) {
			return null;
		}
		const rect = el.getBoundingClientRect();
		return {
			rect: { x: rect.x, y: rect.y, width: rect.width, height: rect.height },
			target: { surface: 'queuePlayback', id },
		};
	}

	/** The open popover's painted rows, each resolved to its choice index in
	 * the frame's `choices` list. */
	function popoverRegions(): LayoutRegion[] {
		if (!rowEl) {
			return [];
		}
		return [...rowEl.querySelectorAll<HTMLElement>('[data-popover-row]')]
			.map((el) =>
				region(el, { trackPopoverChoice: Number(el.dataset.popoverRow) }),
			)
			.filter((entry) => entry !== null);
	}

	/** The painted regions this frame's DOM reports (design D4): the audio
	 * and subtitle chips — the video chip is label-only and reports none —
	 * and the open popover's rows under the popover's own key (the header
	 * comment names why they report last). */
	function currentRegions(): LayoutRegion[] {
		return [region(audioChipEl, 'audioChip'), region(subtitleChipEl, 'subtitleChip')].filter(
			(entry) => entry !== null,
		);
	}

	function report(): void {
		reportLayout(reportKey, { lists: [], regions: currentRegions() });
		if (popover) {
			reportLayout(popoverKey, { lists: [], regions: popoverRegions() });
		}
	}

	// Each paint that moved a region reports (design D4): this effect runs
	// after the DOM update, and the ResizeObserver reports layout-driven
	// moves. Reading the overlay registers the popover's rows, which appear
	// and disappear with that frame section; the popover's key retracts
	// while none is open. The reporter dedupes an unchanged contribution.
	$effect(() => {
		void chips;
		void overlay;
		report();
		if (!popover) {
			retractLayout(popoverKey);
		}
	});

	// The row element's resizes report (design D4): the chips row appears
	// when the first frame carries `trackChips`, so the observer binds
	// through an effect — a new one per row bind, disconnected on cleanup.
	$effect(() => {
		const el = rowEl;
		if (!el || typeof ResizeObserver === 'undefined') {
			return;
		}
		const observer = new ResizeObserver(report);
		observer.observe(el);
		return () => observer.disconnect();
	});

	onMount(
		() => () => {
			retractLayout(reportKey);
			retractLayout(popoverKey);
		},
	);
</script>

{#snippet popoverList(pop: TrackPopover)}
	<!-- The track popover (the Menus artboard): the core's live choice list
		in the canvas dialog below the chip, right-aligned, the chosen choice
		filled and checked at the row's trailing edge, and the cursor row's
		text highlighted. -->
	<div
		class="absolute top-full right-0 z-20 mt-1 flex flex-col"
		role="dialog"
		aria-label={pop.chip === 'audio' ? 'Audio' : 'Subtitles'}
		style:width="260px"
		style:padding="6px"
		style:gap="2px"
		style:background={canvasVar('popover')}
		style:border="1px solid var(--canvas-popover-border)"
		style:border-radius="8px"
		style:box-shadow={canvasVar('popover-shadow')}
	>
		{#each pop.choices as choice, index (index)}
			<button
				type="button"
				class="flex items-center justify-between text-left"
				style:height="36px"
				style:padding="0 10px"
				style:border-radius="4px"
				style:font-size="13px"
				style:background={choice.chosen ? canvasVar('selected') : 'none'}
				style:color={choice.chosen
					? canvasVar('heading')
					: index === pop.cursor
						? roleVar('textFocusAccent')
						: canvasVar('text')}
				data-popover-row={index}
			>
				<span class="flex min-w-0 flex-1 items-center">
					<span class="truncate">{choice.label}</span>
					{#if choice.forced}
						<span
							class="shrink-0 font-mono"
							style:margin-left="6px"
							style:font-size="11px"
							style:color={canvasVar('muted')}>forced</span
						>
					{/if}
				</span>
				{#if choice.chosen}
					<Check
						size={14}
						strokeWidth={2.5}
						color={canvasVar('accent')}
					/>
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
		bind:this={rowEl}
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
				style:border-color={popover?.chip === 'audio'
					? canvasVar('open-border')
					: 'var(--mbv-border)'}
				style:background={popover?.chip === 'audio'
					? canvasVar('action')
					: 'var(--mbv-secondary)'}
				style:color={roleVar('textPrimary')}
				aria-expanded={popover?.chip === 'audio'}
				data-chip="audio"
				bind:this={audioChipEl}
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
				style:border-color={popover?.chip === 'subtitles'
					? canvasVar('open-border')
					: 'var(--mbv-border)'}
				style:background={popover?.chip === 'subtitles'
					? canvasVar('action')
					: 'var(--mbv-secondary)'}
				style:color={roleVar('textPrimary')}
				aria-expanded={popover?.chip === 'subtitles'}
				data-chip="subtitles"
				bind:this={subtitleChipEl}
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
