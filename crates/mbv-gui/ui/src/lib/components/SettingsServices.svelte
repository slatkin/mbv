<script lang="ts">
	// The Services section (add-gui-settings-screen design D9, D13's canvas
	// Services grid): one card for each Service in the canvas's two-column
	// grid. The card's header carries the Service's name and, on the right, its
	// state pill — or, for Feeds, the subscription count the frame projects.
	// The actions the TUI offers for the Service's state paint as the canvas's
	// buttons: Remove destructive, setup primary, Cancel ghost, the rest
	// secondary. The open setup form is a recessed well inside its own
	// Service's card, with labelled mono fields — the Emby three-field form or
	// the Audiobookshelf two-field one — with its progress and its error. The
	// password and API-key fields are password inputs the draft clears on
	// submit, so a secret the user typed leaves the window in the command and
	// never in a frame or a draft (the spec's *Secrets never return*).
	import type { GuiServiceActionIntent, GuiSettingsContent } from '$lib/bridge/types';
	import { sendSettingsCommand } from '$lib/bridge/settings';
	import { entryId } from '$lib/settings/entries';
	import type { SettingsEntry } from '$lib/settings/entries';
	import {
		blurSettingsField,
		focusSettingsField,
		pressSettingsEntry,
		setAudiobookshelfSetupDraft,
		setEmbySetupDraft,
		settingsUi,
		submitSettingsForm,
	} from '$lib/settings/state';
	import SettingsButton from './settings/SettingsButton.svelte';
	import SettingsCard from './settings/SettingsCard.svelte';
	import SettingsStatePill from './settings/SettingsStatePill.svelte';

	interface Props {
		/** The Services section's content: the Service cards and the open setup
		 * form's facts. */
		content: Extract<GuiSettingsContent, { kind: 'services' }>;
		/** The section's full entry list. */
		entries: SettingsEntry[];
		/** The entry id under the content cursor; `null` while the sidebar
		 * holds the keyboard. */
		cursorId: string | null;
	}
	const { content, entries, cursorId }: Props = $props();

	const onpress = (id: string) => pressSettingsEntry(entries, id);

	/** Each Service action's button kind (design D13): Remove is destructive,
	 * setup is primary, and every other action is secondary. */
	const ACTION_VARIANTS: Partial<Record<GuiServiceActionIntent, 'primary' | 'destructive'>> = {
		setupEmby: 'primary',
		setupAudiobookshelf: 'primary',
		removeEmby: 'destructive',
		removeAudiobookshelf: 'destructive',
	};

	/** The form's keyboard hold (design D11, step 3): a field's focus sets
	 * the router's fact, and a `focusout` that leaves the form clears it. */
	function fieldFocusOut(event: FocusEvent, form: 'embySetup' | 'audiobookshelfSetup'): void {
		const host = event.currentTarget;
		const next = event.relatedTarget;
		if (host instanceof HTMLElement && (next === null || !host.contains(next as Node))) {
			blurSettingsField(form);
		}
	}

	/** The field the form hands the keyboard to when it opens. */
	function focusFirstField(formEl: HTMLElement | undefined): void {
		formEl?.querySelector('input')?.focus();
	}

	let embyFormEl: HTMLElement | undefined = $state();
	let absFormEl: HTMLElement | undefined = $state();
	let embyWasOpen = false;
	let absWasOpen = false;

	// A form opening hands its first field the keyboard, so the user who
	// picked the setup action by key types without reaching for the pointer
	// (the spec's *keyboard reachability*).
	$effect(() => {
		const open = content.setup?.service === 'emby';
		if (open && !embyWasOpen) {
			focusFirstField(embyFormEl);
		}
		embyWasOpen = open;
	});
	$effect(() => {
		const open = content.setup?.service === 'audiobookshelf';
		if (open && !absWasOpen) {
			focusFirstField(absFormEl);
		}
		absWasOpen = open;
	});
</script>

{#snippet formActions(service: 'emby' | 'audiobookshelf')}
	<div class="flex justify-end" style:gap="8px">
		<SettingsButton
			variant="ghost"
			onpress={() => void sendSettingsCommand({ command: 'cancelSetup', service })}>Cancel</SettingsButton
		>
		<SettingsButton
			variant="primary"
			disabled={content.setup?.busy ?? false}
			onpress={() => submitSettingsForm(content)}
			>{content.setup?.busy === true ? 'Checking…' : 'Set up'}</SettingsButton
		>
	</div>
{/snippet}

<div
	class="grid"
	style:grid-template-columns="repeat(2, minmax(0, 1fr))"
	style:gap="20px"
	style:align-items="start"
	data-settings-services=""
>
	{#each content.services as service (service.entry)}
		<SettingsCard title={service.name} label={service.name}>
			{#snippet header()}
				{#if service.entry === 'feeds' && service.context !== ''}
					<span
						style:font-size="12px"
						style:color="var(--canvas-muted)"
						data-settings-service-context="">{service.context}</span
					>
				{:else}
					<SettingsStatePill state={service.state} />
				{/if}
			{/snippet}
			<div class="flex flex-wrap" style:gap="8px">
				{#each service.actions as action (action.intent)}
					{@const id = entryId.serviceAction(service.entry, action.intent)}
					<SettingsButton
						variant={ACTION_VARIANTS[action.intent] ?? 'secondary'}
						entry={id}
						cursor={id === cursorId}
						onpress={() => onpress(id)}>{action.label}</SettingsButton
					>
				{/each}
			</div>
			{#if service.entry === 'emby' && content.setup?.service === 'emby' && $settingsUi.embySetup !== null}
				{@const setup = content.setup}
				{@const draft = $settingsUi.embySetup}
				<div
					bind:this={embyFormEl}
					class="flex flex-col"
					style:padding="16px"
					style:gap="12px"
					style:background="var(--canvas-well)"
					style:border-radius="8px"
					data-settings-setup="emby"
					onfocusin={() => focusSettingsField('embySetup')}
					onfocusout={(event) => fieldFocusOut(event, 'embySetup')}
				>
					<label
						class="flex flex-col"
						style:gap="6px"
						style:font-size="12px"
						style:color="var(--canvas-muted)"
					>
						Server URL
						<input
							type="text"
							placeholder="http://emby:8096"
							aria-label="Emby server URL"
							value={draft.serverUrl}
							data-settings-field="serverUrl"
							oninput={(event) =>
								setEmbySetupDraft({ serverUrl: event.currentTarget.value })}
						/>
					</label>
					<label
						class="flex flex-col"
						style:gap="6px"
						style:font-size="12px"
						style:color="var(--canvas-muted)"
					>
						Username
						<input
							type="text"
							placeholder="Username"
							aria-label="Emby username"
							value={draft.username}
							data-settings-field="username"
							oninput={(event) =>
								setEmbySetupDraft({ username: event.currentTarget.value })}
						/>
					</label>
					<label
						class="flex flex-col"
						style:gap="6px"
						style:font-size="12px"
						style:color="var(--canvas-muted)"
					>
						Password
						<input
							type="password"
							placeholder="Password"
							aria-label="Emby password"
							value={draft.password}
							data-settings-field="password"
							oninput={(event) =>
								setEmbySetupDraft({ password: event.currentTarget.value })}
						/>
					</label>
					{#if setup.error !== ''}
						<div
							style:font-size="13px"
							style:color="var(--canvas-destructive)"
							data-settings-setup-error="">{setup.error}</div
						>
					{/if}
					{@render formActions('emby')}
				</div>
			{:else if service.entry === 'audiobookshelf' && content.setup?.service === 'audiobookshelf' && $settingsUi.audiobookshelfSetup !== null}
				{@const setup = content.setup}
				{@const draft = $settingsUi.audiobookshelfSetup}
				<div
					bind:this={absFormEl}
					class="flex flex-col"
					style:padding="16px"
					style:gap="12px"
					style:background="var(--canvas-well)"
					style:border-radius="8px"
					data-settings-setup="audiobookshelf"
					onfocusin={() => focusSettingsField('audiobookshelfSetup')}
					onfocusout={(event) => fieldFocusOut(event, 'audiobookshelfSetup')}
				>
					<label
						class="flex flex-col"
						style:gap="6px"
						style:font-size="12px"
						style:color="var(--canvas-muted)"
					>
						Server URL
						<input
							type="text"
							placeholder="http://audiobookshelf:13378"
							aria-label="Audiobookshelf server URL"
							value={draft.serverUrl}
							data-settings-field="serverUrl"
							oninput={(event) =>
								setAudiobookshelfSetupDraft({ serverUrl: event.currentTarget.value })}
						/>
					</label>
					<label
						class="flex flex-col"
						style:gap="6px"
						style:font-size="12px"
						style:color="var(--canvas-muted)"
					>
						API key
						<input
							type="password"
							placeholder="API key"
							aria-label="Audiobookshelf API key"
							value={draft.apiKey}
							data-settings-field="apiKey"
							oninput={(event) =>
								setAudiobookshelfSetupDraft({ apiKey: event.currentTarget.value })}
						/>
					</label>
					{#if setup.error !== ''}
						<div
							style:font-size="13px"
							style:color="var(--canvas-destructive)"
							data-settings-setup-error="">{setup.error}</div
						>
					{/if}
					{@render formActions('audiobookshelf')}
				</div>
			{/if}
		</SettingsCard>
	{/each}
</div>
