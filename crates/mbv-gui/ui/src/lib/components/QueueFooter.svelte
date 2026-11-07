<script lang="ts">
	// The queue's canvas footer line (align-gui-with-design-canvas U4.3):
	// one 40 px band at the queue column's bottom, with the canvas's top
	// border and side padding, that merges what the TUI paints as the queue
	// footer's status pills and the Status bar's prefix and toast rows — the
	// queue source, the session marker, the prefix-armed state, and the
	// toasts.
	//
	// The left group is the queue source: the autosave indicator (while
	// save-on-consume is on for a saved playlist), then the playlist text.
	// The canvas paints the source as plain text with no leading icon, and
	// the text reports no hit region — the GUI presents no playlists
	// surface, so a press on it would have nowhere to go, and a reported
	// target that can never fire is a lie in the layout report. (The TUI's
	// pill opens its Playlists overlay.) The right group is the dirty
	// indicator (while the queue differs from its saved playlist), then the
	// session marker: the footer owner pill's label — the Player-owner label
	// (`woims` on the canvas) — then its liveness dot, painted only while
	// the shown Owner's link is up (`ownerLive`, design D4a: the dot marks
	// the live Owner). The canvas's owner marker carries no leading icon
	// either.
	//
	// The prefix-armed state paints the same `PREFIX` badge the TUI's
	// status row paints while prefix mode is armed, and it drops while a
	// toast is live — the TUI's pill-drop rule carried into the line. The
	// toast paints inline in the line, centred in its severity colour:
	// the spec permits the footer to show toasts in the line or in a
	// conventional GUI form. A live toast moves no layout: the line is
	// always painted and the toast only fills its centre span. The centre
	// is empty while the projection holds none of those states, as the
	// canvas draws it.
	//
	// The band paints the canvas's type and border (mono 12 px, a 1 px top
	// border in the queue-local `--canvas-border` token) on the frame's
	// `queueFooterBand` fill. The slice's `GuiStatusBar` carries no volume,
	// mute, or visual-mode content, so the line paints no interactive status
	// pill and reports no region at all — a point on the line resolves to
	// nothing. It still reports through the shared layout reporter so the
	// merged report accounts for the mounted surface.
	import { onMount } from 'svelte';
	import { SaveCheck, SaveOff } from '@lucide/svelte';
	import { reportLayout, retractLayout } from '$lib/bridge/layout';
	import { roleProperty } from '$lib/bridge/theme';
	import type {
		GuiQueueFooter,
		GuiRole,
		GuiStatusBar,
		GuiToastSeverity,
		LayoutRegion,
	} from '$lib/bridge/types';

	interface Props {
		footer: GuiQueueFooter;
		status: GuiStatusBar;
	}
	const { footer, status }: Props = $props();

	const reportKey = Symbol('queue-footer');
	let lineEl: HTMLElement | undefined = $state();

	/** The `var(--mbv-<role>)` value a part paints in. */
	function roleVar(role: GuiRole): string {
		return `var(${roleProperty(role)})`;
	}

	/** The toast text's severity foreground (`toast_spans` in the TUI's
	 * status bar panel). */
	function toastColor(severity: GuiToastSeverity): string {
		switch (severity) {
			case 'neutral':
				return 'var(--mbv-textPrimary)';
			case 'success':
				return 'var(--mbv-statusAvailable)';
			case 'warning':
				return 'var(--mbv-textFocusAccent)';
			case 'error':
				return 'var(--mbv-statusError)';
		}
	}

	/** The painted regions this frame's DOM reports (design D4): none. The
	 * line's pills are informational, so a point on it resolves to no
	 * target. */
	function currentRegions(): LayoutRegion[] {
		return [];
	}

	function report(): void {
		reportLayout(reportKey, { lists: [], regions: currentRegions() });
	}

	// Each paint that moved a region reports (design D4): this effect runs
	// after the DOM update, and the ResizeObserver reports layout-driven
	// moves. The reporter dedupes an unchanged contribution.
	$effect(() => {
		void footer;
		void status;
		report();
	});

	onMount(() => {
		const observer =
			typeof ResizeObserver === 'undefined'
				? undefined
				: new ResizeObserver(report);
		if (lineEl) {
			observer?.observe(lineEl);
		}
		return () => {
			observer?.disconnect();
			retractLayout(reportKey);
		};
	});
</script>

<!-- The canvas footer band: 40 px, 16 px side padding, the canvas's 1 px
	top border, and the canvas's mono type. The left line is the queue's
	source text, the right line the owner marker (label then dot), and the
	centre carries the toast, the prefix badge, and the autosave and dirty
	states while they are active. -->
<div
	class="flex items-center overflow-hidden font-mono whitespace-nowrap"
	style:height="40px"
	style:padding-inline="16px"
	style:font-size="12px"
	style:border-top="1px solid var(--canvas-border)"
	style:background="var(--mbv-queueFooterBand)"
	data-footer-line
	bind:this={lineEl}
>
	<!-- The left group: the prefix badge while armed (dropped while a toast
		is live, the TUI's pill-drop rule), the autosave indicator, then the
		queue's source text — no leading icon, as the canvas paints it. -->
	<div class="flex items-center" style:gap="16px">
		{#if status.prefixArmed && !status.toast}
			<span class="font-bold" style:color="var(--mbv-accent)">PREFIX</span>
		{/if}
		{#if footer.autosave}
			<SaveCheck size={14} color={roleVar(footer.autosave.role)} class="shrink-0" />
		{/if}
		{#if footer.playlist.text}
			<span style:color={roleVar(footer.playlist.role)} data-footer-source
				>{footer.playlist.text}</span
			>
		{/if}
	</div>
	<!-- The toast, inline in the line's centre in its severity colour (the
		footer-line toast form the spec permits); empty while none is live. -->
	<span
		class="min-w-0 flex-1 truncate px-4 text-center"
		style:color={status.toast ? toastColor(status.toast.severity) : undefined}
	>
		{#if status.toast}{status.toast.text}{/if}
	</span>
	<!-- The right group: the dirty state while present, then the session
		marker — the owner pill's label then its liveness dot while the shown
		Owner's link is up — flush at the row's right edge. -->
	<div class="flex items-center" style:gap="16px">
		{#if footer.dirty}
			<span class="flex items-center" style:gap="8px">
				<SaveOff size={14} color={roleVar(footer.dirty.role)} class="shrink-0" />
				<span class:font-bold={footer.dirty.bold} style:color={roleVar(footer.dirty.role)}
					>{footer.dirty.text}</span
				>
			</span>
		{/if}
		<span class="flex items-center" style:gap="6px" style:color={roleVar(footer.owner.role)}>
			<span data-owner-label>{footer.owner.text}</span>
			{#if footer.ownerLive}
				<!-- The live-Owner dot (design D4a): the shown queue's Player
					owner answers on a live link. -->
				<span
					style:color="var(--mbv-statusAvailable)"
					data-owner-live
					title="Owner live"
					aria-label="Owner live">●</span
				>
			{/if}
		</span>
	</div>
</div>
