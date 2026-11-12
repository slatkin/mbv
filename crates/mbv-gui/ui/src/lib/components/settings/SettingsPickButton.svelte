<script lang="ts">
	// A Settings pick row's control (add-gui-settings-screen design D13, the
	// canvas pick button): 34 px high, at least 140 px wide, the value and a
	// `ChevronDown`, opening the row's values in the row menu's chrome. The
	// press runs the row's activation (`pressSettingsEntry`), the same one
	// Enter runs, so the keyboard and the pointer open one menu. The menu is
	// window content: the open state lives in the Settings pick store
	// (`$lib/settings/pick-menu`), so the Settings key router takes its Up,
	// Down, Enter, and Esc while it is open, and one press outside closes it
	// with no send. A press on an entry sends `setValue` for the row's setting
	// and that value, the same command the keyboard's Enter sends.
	import { ChevronDown } from '@lucide/svelte';
	import QueueRowMenu from '$lib/components/QueueRowMenu.svelte';
	import {
		closePickMenu,
		pickMenu,
		pickValueLabel,
		selectPickValue,
	} from '$lib/settings/pick-menu';

	interface Props {
		/** The row's setting name, which `setValue` carries back. */
		setting: string;
		/** The row's accessible name. */
		label: string;
		/** The value the row shows now; the empty value paints as Any. */
		value: string;
		/** Whether the pick cannot be operated. */
		disabled?: boolean;
		/** A press on the button: the row's activation. */
		onpress: () => void;
	}
	const { setting, label, value, disabled = false, onpress }: Props = $props();

	/** The open menu, while it is this row's. */
	const menu = $derived($pickMenu?.setting === setting ? $pickMenu : null);

	let buttonEl: HTMLButtonElement | undefined = $state();
	let menuEl: HTMLElement | undefined = $state();

	// A press outside the menu and its button closes the menu with no send,
	// as the queue's row menu closes on Esc (design D13).
	$effect(() => {
		if (menu === null) {
			return;
		}
		function outside(event: PointerEvent): void {
			const target = event.target;
			if (!(target instanceof Node)) {
				return;
			}
			if (menuEl?.contains(target) || buttonEl?.contains(target)) {
				return;
			}
			closePickMenu();
		}
		window.addEventListener('pointerdown', outside);
		return () => window.removeEventListener('pointerdown', outside);
	});
</script>

<div class="relative flex-none" data-settings-pick={setting}>
	<button
		bind:this={buttonEl}
		type="button"
		class="flex items-center justify-between"
		style:height="34px"
		style:min-width="140px"
		style:gap="10px"
		style:padding="0 10px 0 12px"
		style:background="var(--canvas-library-raise)"
		style:border="1px solid var(--canvas-library-control)"
		style:border-radius="6px"
		style:color="var(--canvas-text)"
		style:font-size="13px"
		aria-label={`${label}: ${pickValueLabel(value)}`}
		aria-haspopup="menu"
		aria-expanded={menu !== null}
		{disabled}
		onclick={onpress}
	>
		<span class="min-w-0 truncate">{pickValueLabel(value)}</span>
		<ChevronDown size={12} strokeWidth={2.2} aria-hidden="true" />
	</button>
	{#if menu !== null}
		<div bind:this={menuEl}>
			<QueueRowMenu
				label={`${label} values`}
				cursor={menu.cursor}
				entries={menu.values.map((candidate) => ({
					label: pickValueLabel(candidate),
					keyHint: null,
				}))}
				top="calc(100% + 4px)"
				right="0"
				onSelect={(index) => selectPickValue(index)}
			/>
		</div>
	{/if}
</div>
