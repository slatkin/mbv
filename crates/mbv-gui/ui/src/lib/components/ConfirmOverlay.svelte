<script lang="ts">
	// The shared confirmation modal the Client core raised (`App::ask_confirm`,
	// add-gui-library-panel queue fixes): the message and the button row the
	// frame's `confirm` overlay carries, painted window-modal over every
	// panel. The core holds the action; Enter and Esc answer through the
	// core's overlay key rules, and a press on a button forwards like every
	// other pointer event — the core resolves it against the hit map and
	// executes the button's key hint.
	//
	// The buttons report their `confirm` regions under this component's own
	// report key, so the core's hit map names them while the modal paints;
	// the contribution retracts on unmount.
	import { onMount } from 'svelte';
	import { reportLayout, retractLayout } from '$lib/bridge/layout';
	import type { GuiOverlayFrame, LayoutRegion } from '$lib/bridge/types';

	interface Props {
		confirm: Extract<GuiOverlayFrame, { kind: 'confirm' }>;
	}
	const { confirm }: Props = $props();

	const reportKey = Symbol('confirmOverlay');

	function region(el: HTMLElement): LayoutRegion {
		const rect = el.getBoundingClientRect();
		return {
			rect: { x: rect.x, y: rect.y, width: rect.width, height: rect.height },
			target: { surface: 'confirm', id: { button: Number(el.dataset.confirmButton) } },
		};
	}

	function buttonRegions(): LayoutRegion[] {
		return [...document.querySelectorAll<HTMLElement>('[data-confirm-button]')].map(region);
	}

	$effect(() => {
		void confirm;
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
	data-confirm-overlay
	role="dialog"
	aria-modal="true"
	aria-label="Confirm"
>
	<div
		class="flex flex-col"
		style:background="var(--canvas-menu)"
		style:border="1px solid var(--canvas-menu-border)"
		style:border-radius="8px"
		style:box-shadow="var(--canvas-menu-shadow)"
		style:padding="16px"
		style:gap="12px"
		style:max-width="340px"
	>
		<p style:font-size="13px" style:color="var(--canvas-heading)">{confirm.message}</p>
		<div class="flex justify-end" style:gap="8px">
			{#each confirm.buttons as button, index (index)}
				<button
					type="button"
					data-confirm-button={index}
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
