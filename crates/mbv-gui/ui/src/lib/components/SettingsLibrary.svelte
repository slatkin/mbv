<script lang="ts">
	// The Library section (add-gui-settings-screen design D9): the section's
	// choice rows (the hidden and the feed-view libraries) above the routes
	// list — each routable library with its assigned device, and, once a
	// `routeDevices` load lands, the selected library's candidates with the
	// automatic entry that clears the route. A device with no direct-connect
	// endpoint paints disabled: it is not currently routable, so a press on
	// it assigns nothing.
	import type { GuiSettingsContent } from '$lib/bridge/types';
	import { entryId } from '$lib/settings/entries';
	import type { SettingsEntry } from '$lib/settings/entries';
	import { pressSettingsEntry } from '$lib/settings/state';
	import SettingsEntryRow from './SettingsEntryRow.svelte';
	import SettingsRows from './SettingsRows.svelte';

	interface Props {
		/** The Library section's content: its rows, routes, the selected
		 * library, and its candidate devices. */
		content: Extract<GuiSettingsContent, { kind: 'library' }>;
		/** The section's full entry list. */
		entries: SettingsEntry[];
		/** The entry id under the content cursor; `null` while the sidebar
		 * holds the keyboard. */
		cursorId: string | null;
	}
	const { content, entries, cursorId }: Props = $props();

	const onpress = (id: string) => pressSettingsEntry(entries, id);
</script>

<div data-settings-library="">
	<SettingsRows rows={content.rows} {entries} {cursorId} />
	{#if content.routes !== null}
		<div
			class="flex items-center font-mono"
			style:height="36px"
			style:padding="0 16px"
			style:font-size="13px"
			style:color="var(--canvas-muted)"
			style:border-top="1px solid var(--canvas-border)"
			data-settings-routes-head=""
		>
			Library routes
		</div>
		{#each content.routes as route (route.library)}
			{@const id = entryId.route(route.library)}
			<SettingsEntryRow
				{id}
				cursor={id === cursorId}
				{onpress}
				label={route.name}
				trailing={route.endpoint ?? 'Automatic'}
			/>
		{/each}
		{#if content.selected !== null}
			{@const library = content.selected}
			{@const id = entryId.routeClear(library)}
			<SettingsEntryRow
				{id}
				cursor={id === cursorId}
				{onpress}
				label="Automatic"
				indent={true}
			/>
			{#each content.devices ?? [] as device (device.name)}
				{@const id = entryId.routeDevice(library, device.name)}
				<SettingsEntryRow
					{id}
					cursor={id === cursorId}
					{onpress}
					label={device.name}
					trailing={device.endpoint === null ? 'Not routable' : null}
					disabled={device.endpoint === null}
					indent={true}
				/>
			{/each}
		{/if}
	{/if}
</div>
