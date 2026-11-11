<script lang="ts">
	// The daemon-lost recovery dialog the Client core raised
	// (`App::raise_daemon_lost_modal`, `daemon-disconnect-handling`: an
	// unannounced connection loss raises a recovery dialog): the diagnostics
	// and the recovery choices the frame's `daemonLost` overlay carries,
	// painted window-modal over every panel in the confirmation modal's own
	// look.
	//
	// The dialog blocks every other input until one of its choices runs: the
	// core's overlay key rules bind its three chords (and nothing else, Esc
	// included), and a press on a button forwards like every other pointer
	// event — the core resolves it against the hit map and answers with the
	// choice the index names.
	//
	// The buttons report their `daemonLost` regions under this component's
	// own report key, so the core's hit map names them while the dialog
	// paints; the contribution retracts on unmount.
	import { onMount } from 'svelte';
	import { reportLayout, retractLayout } from '$lib/bridge/layout';
	import type { GuiOverlayFrame, LayoutRegion } from '$lib/bridge/types';

	interface Props {
		daemonLost: Extract<GuiOverlayFrame, { kind: 'daemonLost' }>;
	}
	const { daemonLost }: Props = $props();

	const reportKey = Symbol('daemonLostOverlay');

	function region(el: HTMLElement): LayoutRegion {
		const rect = el.getBoundingClientRect();
		return {
			rect: { x: rect.x, y: rect.y, width: rect.width, height: rect.height },
			target: { surface: 'daemonLost', id: { button: Number(el.dataset.daemonLostButton) } },
		};
	}

	function buttonRegions(): LayoutRegion[] {
		return [...document.querySelectorAll<HTMLElement>('[data-daemon-lost-button]')].map(region);
	}

	$effect(() => {
		void daemonLost;
		reportLayout(reportKey, { lists: [], regions: buttonRegions() });
	});

	onMount(() => {
		return () => {
			retractLayout(reportKey);
		};
	});
</script>

<div
	class="absolute inset-0 z-30 flex items-center justify-center"
	style:background="var(--canvas-modal-scrim)"
	data-daemon-lost-overlay
	role="dialog"
	aria-modal="true"
	aria-label="The local daemon connection was lost"
>
	<div
		class="flex flex-col"
		style:background="var(--canvas-menu)"
		style:border="1px solid var(--canvas-menu-border)"
		style:border-radius="8px"
		style:box-shadow="var(--canvas-menu-shadow)"
		style:padding="16px"
		style:gap="12px"
		style:max-width="420px"
	>
		<p style:font-size="13px" style:color="var(--canvas-heading)">
			The local daemon connection was lost unexpectedly.
		</p>
		{#if daemonLost.lastPlayingTitle !== null}
			<p style:font-size="12px" style:color="var(--canvas-text-secondary)">
				Was playing: {daemonLost.lastPlayingTitle}
			</p>
		{/if}
		<p style:font-size="12px" style:color="var(--canvas-text-secondary)">
			Daemon log: {daemonLost.daemonLogPath}
		</p>
		{#if daemonLost.restartError !== null}
			<p style:font-size="12px" style:color="var(--canvas-destructive)">
				Restart failed: {daemonLost.restartError}
			</p>
		{/if}
		<div class="flex justify-end" style:gap="8px">
			{#each daemonLost.buttons as button, index (index)}
				<button
					type="button"
					data-daemon-lost-button={index}
					class="flex items-center"
					style:gap="6px"
					style:padding="4px 10px"
					style:border-radius="4px"
					style:font-size="12px"
					style:background="var(--canvas-well)"
					style:color={button.tone === 'affirmative'
						? 'var(--canvas-accent)'
						: 'var(--canvas-destructive)'}
				>
					<span>{button.label}</span>
					<span class="font-mono tabular-nums" style:font-size="11px"
						>{button.keys}</span
					>
				</button>
			{/each}
		</div>
	</div>
</div>
