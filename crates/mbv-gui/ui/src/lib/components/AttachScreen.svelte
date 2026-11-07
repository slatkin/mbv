<script lang="ts">
	// The Owner attach screen (add-tauri-gui-client task 3.5, design D9): the
	// state the window shows while the core thread's attach campaign has no
	// slice to paint — one variant per `GuiAttachState`, with the text the
	// campaign carries (the same text `startup::run_local_instance` gives the
	// TUI Client). A `null` attach state is the wait before the first frame:
	// the core emits a frame only after the first layout report, so this
	// screen also paints while the subscription is still listening.
	//
	// The mismatch state paints the Restart and Quit buttons (key R and key
	// Q, as the TUI prompt's `[R] Restart  [Q] Quit` line reads) and reports
	// their regions through the shared layout reporter — before attach the
	// core thread resolves them itself against the hit map. Every other
	// state answers nothing and paints no regions.
	//
	// The component interprets no key and no pointer event: it paints and
	// reports, and the core resolves a press or chord.
	import { onMount } from 'svelte';
	import { LoaderCircle, RotateCcw, X } from '@lucide/svelte';
	import { reportLayout, retractLayout } from '$lib/bridge/layout';
	import { roleProperty } from '$lib/bridge/theme';
	import type { AttachTargetId, GuiAttachState, LayoutRegion } from '$lib/bridge/types';

	interface Props {
		/** The attach state the frame carries, or `null` before the first frame. */
		attach: GuiAttachState | null;
	}
	const { attach }: Props = $props();

	const reportKey = Symbol('attach-screen');
	let screenEl: HTMLElement | undefined = $state();
	let restartEl: HTMLElement | undefined = $state();
	let quitEl: HTMLElement | undefined = $state();

	/** The `var(--mbv-<role>)` value a part paints in. */
	function roleVar(role: string): string {
		return `var(${roleProperty(role)})`;
	}

	/** One button's painted region, `null` while the button is absent. */
	function region(
		el: HTMLElement | undefined,
		id: AttachTargetId,
	): LayoutRegion | null {
		if (!el) {
			return null;
		}
		const rect = el.getBoundingClientRect();
		return {
			rect: { x: rect.x, y: rect.y, width: rect.width, height: rect.height },
			target: { surface: 'attach', id },
		};
	}

	function report(): void {
		reportLayout(reportKey, {
			lists: [],
			regions: [region(restartEl, 'restart'), region(quitEl, 'quit')].filter(
				(entry) => entry !== null,
			),
		});
	}

	// Each paint that moved a region reports (design D4): this effect runs
	// after the DOM update, and the ResizeObserver reports layout-driven
	// moves. The mount report also asks the core to emit its frame — the
	// campaign answers a report by re-showing the state with its theme.
	$effect(() => {
		void attach;
		report();
	});

	onMount(() => {
		const observer =
			typeof ResizeObserver === 'undefined'
				? undefined
				: new ResizeObserver(report);
		if (screenEl) {
			observer?.observe(screenEl);
		}
		return () => {
			observer?.disconnect();
			retractLayout(reportKey);
		};
	});
</script>

<!-- The canvas draws none of these states (design D15), so they paint in
	the canvas's type, colour, and spacing: `text-sm` body type with the
	canvas's relaxed leading, the cell unit for the column's gap and gutter,
	and the mismatch buttons as the canvas's pill controls — `secondary`
	fill, `border` outline, small medium-weight labels, and the key hints in
	the mono face's muted colour. -->
<div
	class="flex h-full flex-col items-center justify-center"
	style:gap="var(--mbv-cell-col)"
	style:padding-inline="calc(var(--mbv-cell-col) * 2)"
	bind:this={screenEl}
>
	{#if attach === null || attach.kind === 'waiting'}
		<!-- The wait before the first frame and the wait for a shutting-down
			Owner read the same: the core is still working toward the attach. -->
		<LoaderCircle
			size={20}
			class="animate-spin"
			color={roleVar('textMuted')}
		/>
		<span class="text-sm font-medium" style:color={roleVar('textSecondary')}
			>Attaching to the Owner process…</span
		>
	{:else}
		<span
			class="max-w-prose text-center text-sm leading-relaxed whitespace-pre-line"
			style:color={roleVar(
				attach.kind === 'refusal' || attach.kind === 'exclusiveRefusal'
					? 'statusError'
					: 'textPrimary',
			)}>{attach.detail}</span
		>
		{#if attach.kind === 'mismatch'}
			<div class="flex items-center" style:gap="calc(var(--mbv-cell-col) / 2)">
				<button
					type="button"
					class="flex items-center gap-1.5 rounded-full border px-3 py-1.5 text-xs font-medium"
					style:background={roleVar('secondary')}
					style:border-color="var(--mbv-border)"
					style:color={roleVar('textPrimary')}
					bind:this={restartEl}
					data-target="restart"
				>
					<RotateCcw size={14} color={roleVar('accent')} />
					Restart
					<span
						class="font-mono text-xs tabular-nums"
						style:color={roleVar('textMuted')}>[R]</span
					>
				</button>
				<button
					type="button"
					class="flex items-center gap-1.5 rounded-full border px-3 py-1.5 text-xs font-medium"
					style:background={roleVar('secondary')}
					style:border-color="var(--mbv-border)"
					style:color={roleVar('textPrimary')}
					bind:this={quitEl}
					data-target="quit"
				>
					<X size={14} color={roleVar('statusError')} />
					Quit
					<span
						class="font-mono text-xs tabular-nums"
						style:color={roleVar('textMuted')}>[Q]</span
					>
				</button>
			</div>
		{/if}
	{/if}
</div>
