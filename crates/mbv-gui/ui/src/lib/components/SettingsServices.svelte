<script lang="ts">
	// The Services section (add-gui-settings-screen design D9): the three
	// Services with each one's state label and the actions the TUI offers for
	// that state, plus the open setup form — the Emby three-field form or the
	// Audiobookshelf two-field one — with its progress and its error. The
	// password and API-key fields are password inputs the draft clears on
	// submit, so a secret the user typed leaves the window in the command
	// and never in a frame or a draft (the spec's *Secrets never return*).
	import type { GuiSettingsContent } from '$lib/bridge/types';
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
	import SettingsEntryRow from './SettingsEntryRow.svelte';

	interface Props {
		/** The Services section's content: the Service rows and the open setup
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

	/** The canvas-bordered text field every Settings form paints. */
	const fieldStyle =
		'background:var(--canvas-library-column);border:1px solid var(--canvas-border);border-radius:6px;color:var(--canvas-heading);font-size:14px;height:32px;padding:0 10px;outline:none;';

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

<div data-settings-services="">
	{#each content.services as service (service.entry)}
		<div
			class="flex items-center"
			style:height="36px"
			style:padding="0 16px"
			style:gap="8px"
			style:font-size="14px"
			style:color="var(--canvas-heading)"
			style:border-top="1px solid var(--canvas-border)"
			data-settings-service={service.entry}
		>
			<span class="min-w-0 flex-1 truncate">{service.name}</span>
			<span class="flex-none truncate" style:color="var(--canvas-muted)"
				>{service.state}{service.context === '' ? '' : ` · ${service.context}`}</span
			>
		</div>
		{#each service.actions as action (action.intent)}
			{@const id = entryId.serviceAction(service.entry, action.intent)}
			<SettingsEntryRow {id} cursor={id === cursorId} {onpress} label={action.label} indent={true} />
		{/each}
		{#if service.entry === 'emby' && content.setup?.service === 'emby' && $settingsUi.embySetup !== null}
			{@const setup = content.setup}
			{@const draft = $settingsUi.embySetup}
			<div
				bind:this={embyFormEl}
				class="flex flex-col"
				style:padding="8px 16px"
				style:gap="8px"
				style:border-top="1px solid var(--canvas-border)"
				data-settings-setup="emby"
				onfocusin={() => focusSettingsField('embySetup')}
				onfocusout={(event) => fieldFocusOut(event, 'embySetup')}
			>
				<input
					type="text"
					placeholder="Server URL"
					aria-label="Emby server URL"
					style={fieldStyle}
					value={draft.serverUrl}
					data-settings-field="serverUrl"
					oninput={(event) => setEmbySetupDraft({ serverUrl: event.currentTarget.value })}
				/>
				<input
					type="text"
					placeholder="Username"
					aria-label="Emby username"
					style={fieldStyle}
					value={draft.username}
					data-settings-field="username"
					oninput={(event) => setEmbySetupDraft({ username: event.currentTarget.value })}
				/>
				<input
					type="password"
					placeholder="Password"
					aria-label="Emby password"
					style={fieldStyle}
					value={draft.password}
					data-settings-field="password"
					oninput={(event) => setEmbySetupDraft({ password: event.currentTarget.value })}
				/>
				{#if setup.error !== ''}
					<div style:font-size="13px" style:color="var(--canvas-destructive)" data-settings-setup-error="">
						{setup.error}
					</div>
				{/if}
				<div class="flex" style:gap="8px">
					<button
						type="button"
						disabled={setup.busy}
						style:font-size="13px"
						style:padding="4px 10px"
						style:border-radius="4px"
						style:border="0"
						style:background="var(--canvas-well)"
						style:color="var(--canvas-accent)"
						data-settings-setup-submit=""
						onclick={() => submitSettingsForm(content)}
						>{setup.busy ? 'Checking…' : 'Set up'}</button
					>
					<button
						type="button"
						style:font-size="13px"
						style:padding="4px 10px"
						style:border-radius="4px"
						style:border="0"
						style:background="var(--canvas-well)"
						style:color="var(--canvas-destructive)"
						data-settings-setup-cancel=""
						onclick={() =>
							void sendSettingsCommand({ command: 'cancelSetup', service: 'emby' })}
						>Cancel</button
					>
				</div>
			</div>
		{:else if service.entry === 'audiobookshelf' && content.setup?.service === 'audiobookshelf' && $settingsUi.audiobookshelfSetup !== null}
			{@const setup = content.setup}
			{@const draft = $settingsUi.audiobookshelfSetup}
			<div
				bind:this={absFormEl}
				class="flex flex-col"
				style:padding="8px 16px"
				style:gap="8px"
				style:border-top="1px solid var(--canvas-border)"
				data-settings-setup="audiobookshelf"
				onfocusin={() => focusSettingsField('audiobookshelfSetup')}
				onfocusout={(event) => fieldFocusOut(event, 'audiobookshelfSetup')}
			>
				<input
					type="text"
					placeholder="Server URL"
					aria-label="Audiobookshelf server URL"
					style={fieldStyle}
					value={draft.serverUrl}
					data-settings-field="serverUrl"
					oninput={(event) =>
						setAudiobookshelfSetupDraft({ serverUrl: event.currentTarget.value })}
				/>
				<input
					type="password"
					placeholder="API key"
					aria-label="Audiobookshelf API key"
					style={fieldStyle}
					value={draft.apiKey}
					data-settings-field="apiKey"
					oninput={(event) =>
						setAudiobookshelfSetupDraft({ apiKey: event.currentTarget.value })}
				/>
				{#if setup.error !== ''}
					<div style:font-size="13px" style:color="var(--canvas-destructive)" data-settings-setup-error="">
						{setup.error}
					</div>
				{/if}
				<div class="flex" style:gap="8px">
					<button
						type="button"
						disabled={setup.busy}
						style:font-size="13px"
						style:padding="4px 10px"
						style:border-radius="4px"
						style:border="0"
						style:background="var(--canvas-well)"
						style:color="var(--canvas-accent)"
						data-settings-setup-submit=""
						onclick={() => submitSettingsForm(content)}
						>{setup.busy ? 'Checking…' : 'Set up'}</button
					>
					<button
						type="button"
						style:font-size="13px"
						style:padding="4px 10px"
						style:border-radius="4px"
						style:border="0"
						style:background="var(--canvas-well)"
						style:color="var(--canvas-destructive)"
						data-settings-setup-cancel=""
						onclick={() =>
							void sendSettingsCommand({ command: 'cancelSetup', service: 'audiobookshelf' })}
						>Cancel</button
					>
				</div>
			</div>
		{/if}
	{/each}
</div>
