<script lang="ts">
	// One labelled text field of a Settings form (add-gui-settings-screen design
	// D13): the canvas's small muted label above a mono input. The Services
	// setup forms and the Feeds form paint every field through it.
	interface Props {
		/** The label above the input. */
		label: string;
		/** The input's current draft value. */
		value: string;
		/** The `data-settings-field` name the tests and the key router read. */
		field: string;
		/** The input's accessible name. */
		ariaLabel: string;
		placeholder?: string;
		/** `password` for a secret the draft clears on submit. */
		type?: 'text' | 'password';
		/** The input's own event: the new value. */
		oninput: (value: string) => void;
	}
	const { label, value, field, ariaLabel, placeholder = label, type = 'text', oninput }: Props =
		$props();
</script>

<label class="flex flex-col" style:gap="6px" style:font-size="12px" style:color="var(--canvas-muted)">
	{label}
	<input
		{type}
		{placeholder}
		aria-label={ariaLabel}
		{value}
		data-settings-field={field}
		oninput={(event) => oninput(event.currentTarget.value)}
	/>
</label>
