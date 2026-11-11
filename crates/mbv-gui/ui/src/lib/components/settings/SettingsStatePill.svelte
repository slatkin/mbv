<script lang="ts">
	// A Service's state (add-gui-settings-screen design D13, the canvas state
	// pill): a dot and the state's label in a 24 px pill. Ready paints the
	// success fill and the accent tone; Not configured, Connecting, Needs
	// authentication, and Unavailable paint the muted, source, partial, and
	// destructive tones as text, so one label grammar serves every state.
	interface Props {
		/** The Service's state label, as the frame carries it. */
		state: string;
	}
	const { state }: Props = $props();

	/** Each state's tone: the dot and the label share it. */
	const TONES: Record<string, string> = {
		ready: 'var(--canvas-accent)',
		'not configured': 'var(--canvas-muted)',
		connecting: 'var(--canvas-source)',
		'needs authentication': 'var(--canvas-partial)',
		unavailable: 'var(--canvas-destructive)',
	};

	const ready = $derived(state.toLowerCase() === 'ready');
	const tone = $derived(TONES[state.toLowerCase()] ?? 'var(--canvas-muted)');
</script>

<span
	class="flex flex-none items-center"
	style:height="24px"
	style:padding="0 10px"
	style:gap="6px"
	style:border-radius="12px"
	style:background={ready ? 'var(--canvas-success-fill)' : 'none'}
	style:color={tone}
	style:font-size="12px"
	style:font-weight="500"
	data-settings-state-pill={state}
>
	<span
		aria-hidden="true"
		style:width="7px"
		style:height="7px"
		style:border-radius="4px"
		style:background={tone}
	></span>
	{state}
</span>
