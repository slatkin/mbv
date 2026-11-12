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
	// The armed prefix and the live toast are each one fact, and each
	// paints once a frame (design D10): while the Library panel shows, its
	// Library Status bar replaces its hints with them, so this line paints
	// neither and the band under the Library column owns them. While the
	// Library panel does not show, this line paints them as the TUI's
	// status row does.
	//
	// The band paints the canvas's type and border (mono 12 px, a 1 px top
	// border in the queue-local `--canvas-border` token) on the frame's
	// `queueFooterBand` fill, with the secondary-title token
	// (`--canvas-type-secondary`, the queue source is a secondary title) and
	// owner-marker sand (`--canvas-owner`) in place of the pills' theme roles. The slice's `GuiStatusBar` carries no volume,
	// mute, or visual-mode content, so the line paints no interactive status
	// pill and reports no region at all — a point on the line resolves to
	// nothing. It still reports through the shared layout reporter so the
	// merged report accounts for the mounted surface.
	import { onMount } from 'svelte';
	import { SaveCheck, SaveOff } from '@lucide/svelte';
	import { observeLayout, reportLayout } from '$lib/bridge/layout';
	import { footer as footerSection, status as statusSection } from '$lib/bridge/sections';
	import { toastColor } from '$lib/bridge/status';
	import { roleVar } from '$lib/bridge/theme';
	import type { LayoutRegion } from '$lib/bridge/types';

	interface Props {
		/** Whether the Library panel shows (design D10): the Library Status
		 * bar owns the armed prefix and the live toast then, so this line
		 * paints neither. */
		libraryVisible: boolean;
	}
	const { libraryVisible }: Props = $props();

	/** The footer section's pills. */
	const footer = $derived($footerSection);
	/** The Status bar section's facts. */
	const status = $derived($statusSection);

	/** The toast this line paints: the section's live toast, or `null` while
	 * the Library panel shows and its Status bar paints it (design D10). */
	const toast = $derived(libraryVisible ? null : status.toast);

	/** Whether this line paints the armed prefix badge: prefix mode is armed,
	 * no toast is live (the line's pill-drop rule), and the Library panel does
	 * not show (design D10). */
	const prefix = $derived(!libraryVisible && status.prefixArmed && !toast);

	const reportKey = Symbol('queue-footer');
	let lineEl: HTMLElement | undefined = $state();

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

	onMount(() => observeLayout(reportKey, lineEl, report));
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
		{#if prefix}
			<span class="font-bold" style:color="var(--mbv-accent)">PREFIX</span>
		{/if}
		{#if footer.autosave}
			<SaveCheck size={14} color={roleVar(footer.autosave.role)} class="shrink-0" />
		{/if}
		{#if footer.playlist.text}
			<!-- The queue's source is a secondary title, so it paints the secondary
				token (design D14); the front end paints no role here. -->
			<span style:color="var(--canvas-type-secondary)" data-footer-source
				>{footer.playlist.text}</span
			>
		{/if}
	</div>
	<!-- The toast, inline in the line's centre in its severity colour (the
		footer-line toast form the spec permits); empty while none is live. -->
	<span
		class="min-w-0 flex-1 truncate px-4 text-center"
		style:color={toast ? toastColor(toast.severity) : undefined}
	>
		{#if toast}{toast.text}{/if}
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
		<!-- The owner marker, label then dot: the canvas paints the label in
			its sand token while the dot keeps the liveness mark's own colour. -->
		<span class="flex items-center" style:gap="6px" style:color="var(--canvas-owner)">
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
