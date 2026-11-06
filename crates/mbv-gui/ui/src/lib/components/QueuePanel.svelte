<script lang="ts">
	// The Queue panel (add-tauri-gui-client task 3.3, design D5): a fixed-row
	// column of the frame's viewport rows with no native DOM scroll — the
	// core owns scroll and sends the rows to paint. The component measures
	// the list and reports the whole rows it fits plus the painted row and
	// playlist-pill regions through the shared layout reporter.
	//
	// The paint mirrors the TUI's media-list row and queue footer
	// (crates/mbv-render/src/components/media_list/row.rs and queue.rs),
	// resolved through the frame's roles: the cursor row and the marked rows
	// paint the selection bar, the active slot leads its row with a Lucide
	// play marker in the accent role, and a played row mutes its title.
	//
	// Two fills alias role values because the frame carries roles and the
	// eight chrome fills, not surface fills (design D7): the selection bar
	// resolves `Surface::SelectedRow`, which the surface table fixes at the
	// Green slot — the value `--mbv-duration` carries; and the footer band
	// and its pills share `Surface::StatusBarPill`, the BgDim slot that
	// `--mbv-pillSelectedFg` carries.
	import { onMount } from 'svelte';
	import { Play } from '@lucide/svelte';
	import { reportLayout, retractLayout } from '$lib/bridge/layout';
	import { roleProperty } from '$lib/bridge/theme';
	import type {
		GuiFooterPill,
		GuiQueue,
		GuiQueueFooter,
		GuiRole,
	} from '$lib/bridge/types';
	import type { LayoutRegion } from '$lib/bridge/types';

	interface Props {
		queue: GuiQueue;
		footer: GuiQueueFooter;
		/** Whether the queue surface holds focus (`GuiFocus::surface`). */
		focused: boolean;
	}
	const { queue, footer, focused }: Props = $props();

	/** The fixed row height in logical pixels. Capacity is whole rows. */
	const ROW_HEIGHT = 28;

	/** The empty-queue text, as the TUI's local queue shows it. The frame
	 * carries no queue link, so the remote-queue text cannot be selected;
	 * the first slice attaches only to a local Owner (design D9). */
	const EMPTY_QUEUE_TEXT = '¯\\_(ツ)_/¯';

	const reportKey = Symbol('queue-panel');
	let panelEl: HTMLElement | undefined = $state();
	let listEl: HTMLElement | undefined = $state();
	let pillEl: HTMLElement | undefined = $state();

	/** Whether one item row paints the selection bar: a marked row bars
	 * always; the cursor row bars only while the panel holds focus, as the
	 * TUI's `focused || multi_selected` rule paints. */
	function bars(slot: number): boolean {
		return queue.marks.includes(slot) || (focused && queue.cursor === slot);
	}

	/** The `var(--mbv-<role>)` value a pill or row part paints in. */
	function roleVar(role: GuiRole): string {
		return `var(${roleProperty(role)})`;
	}

	/** The painted regions this frame's DOM reports (design D4): one per
	 * item row, resolved to its stable slot, plus the playlist pill. */
	function currentRegions(): LayoutRegion[] {
		const regions: LayoutRegion[] = [];
		if (listEl) {
			for (const el of listEl.querySelectorAll<HTMLElement>('[data-slot]')) {
				const rect = el.getBoundingClientRect();
				regions.push({
					rect: {
						x: rect.x,
						y: rect.y,
						width: rect.width,
						height: rect.height,
					},
					target: { surface: 'queue', id: { slot: Number(el.dataset.slot) } },
				});
			}
		}
		if (pillEl) {
			const rect = pillEl.getBoundingClientRect();
			regions.push({
				rect: { x: rect.x, y: rect.y, width: rect.width, height: rect.height },
				target: { surface: 'queue', id: 'playlistPill' },
			});
		}
		return regions;
	}

	function report(): void {
		const wholeRows = Math.floor((listEl?.clientHeight ?? 0) / ROW_HEIGHT);
		reportLayout(reportKey, {
			lists: [{ list: 'queue', wholeRows }],
			regions: currentRegions(),
		});
	}

	// Each paint that moved a region reports (design D4): this effect runs
	// after the DOM update, and the ResizeObserver reports layout-driven
	// moves. The reporter dedupes an unchanged contribution, so a frame
	// that moves only the cursor sends nothing.
	$effect(() => {
		void queue;
		void footer;
		report();
	});

	onMount(() => {
		const observer =
			typeof ResizeObserver === 'undefined'
				? undefined
				: new ResizeObserver(report);
		if (panelEl) {
			observer?.observe(panelEl);
		}
		return () => {
			observer?.disconnect();
			retractLayout(reportKey);
		};
	});
</script>

{#snippet footerPill(pill: GuiFooterPill)}
	<span
		class="px-1"
		class:font-bold={pill.bold}
		style:color={roleVar(pill.role)}>{pill.text}</span
	>
{/snippet}

<div class="flex h-full flex-col bg-card" bind:this={panelEl}>
	<div class="min-h-0 flex-1 overflow-hidden" bind:this={listEl}>
		{#if queue.rows.length === 0}
			<div
				class="flex h-full items-center justify-center font-bold"
				style:color={roleVar('emptyQueueFg')}
			>
				{EMPTY_QUEUE_TEXT}
			</div>
		{:else}
			{#each queue.rows as row, i (row.row === 'item' ? row.slot : `structural-${i}`)}
				{#if row.row === 'item'}
					{@const barred = bars(row.slot)}
					<div
						class="flex items-center gap-1 overflow-hidden px-2 whitespace-nowrap"
						style:height="{ROW_HEIGHT}px"
						style:background={barred ? 'var(--mbv-duration)' : undefined}
						data-slot={row.slot}
					>
						{#if queue.active === row.slot}
							<Play
								size={14}
								fill="currentColor"
								color={barred ? 'var(--mbv-selectedRowFg)' : 'var(--mbv-accent)'}
								class="shrink-0"
							/>
						{/if}
						<span
							class="truncate"
							style:color={barred
								? 'var(--mbv-selectedRowFg)'
								: row.secondary
									? 'var(--mbv-splitRowContextFg)'
									: row.state.state === 'played'
										? 'var(--mbv-textMuted)'
										: 'var(--mbv-textEmphasis)'}>{row.primary}</span
						>
						{#if row.secondary}
							<span
								class="truncate"
								style:color={barred
									? 'var(--mbv-selectedRowFg)'
									: row.state.state === 'played'
										? 'var(--mbv-textMuted)'
										: 'var(--mbv-splitRowTitleFg)'}>{row.secondary}</span
							>
						{/if}
						{#if row.state.state === 'active' || row.state.state === 'nowPlaying'}
							{#if row.state.progress !== null}
								<span
									style:color={barred
										? 'var(--mbv-selectedRowProgressFg)'
										: 'var(--mbv-progressPercent)'}
									>{row.state.progress}%</span
								>
							{/if}
						{/if}
						<span class="flex-1"></span>
						{#if row.trailing}
							<span
								style:color={barred
									? 'var(--mbv-selectedRowFg)'
									: 'var(--mbv-statusAvailable)'}
								>{row.trailing.gutter}</span
							>
						{/if}
						{#if row.kind === 'media' && row.duration}
							<span
								style:color={barred
									? 'var(--mbv-selectedRowFg)'
									: 'var(--mbv-duration)'}>{row.duration}</span
							>
						{/if}
					</div>
				{:else if row.row === 'heading'}
					<div
						class="flex items-center overflow-hidden px-2 uppercase"
						style:height="{ROW_HEIGHT}px"
						style:color={roleVar('groupHeadingFg')}>{row.text}</div
					>
				{:else}
					<div style:height="{ROW_HEIGHT}px"></div>
				{/if}
			{/each}
		{/if}
	</div>
	<div
		class="flex items-center justify-between px-2"
		style:height="{ROW_HEIGHT}px"
		style:background="var(--mbv-pillSelectedFg)"
	>
		<div class="flex items-center">
			{#if footer.autosave}
				{@render footerPill(footer.autosave)}
			{/if}
			<span bind:this={pillEl} class="px-1" class:font-bold={footer.playlist.bold} style:color={roleVar(footer.playlist.role)}>{footer.playlist.text}</span>
		</div>
		<div class="flex items-center">
			{#if footer.dirty}
				{@render footerPill(footer.dirty)}
			{/if}
			{@render footerPill(footer.owner)}
		</div>
	</div>
</div>
