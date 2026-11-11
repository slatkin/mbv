<script lang="ts">
	// The Library list's letter rail (add-gui-library-panel design D7): A to Z
	// beside the list, one 22 px letter a row. A letter no painted row carries
	// is disabled, so it cannot move the cursor; a press scrolls to the first
	// row under the letter and moves the cursor there. The rail marks the
	// letter of the row under the cursor, as the canvas marks B. The canvas
	// gives the rail no key (design D7): its keyboard path is the `/` filter,
	// and the rail's letters are pointer targets only.
	import type { RailLetter } from '$lib/library/list';
	import { jumpToLetter } from '$lib/library/state';
	import { canvasVar } from '$lib/bridge/theme';

	interface Props {
		/** The A–Z letters and the first row under each, from the painted
		 * rows (design D7). */
		letters: RailLetter[];
		/** The letter the cursor row belongs to, or `null` while no row holds
		 * the cursor. */
		marked: string | null;
		/** What a press on an enabled letter does: the Music list's jump by
		 * default, and the Shows list's own while it paints the rail
		 * (design D12). */
		jump?: (letter: string) => void;
	}
	const { letters, marked, jump = jumpToLetter }: Props = $props();


	/** A letter's text colour: the marked letter is the canvas's green, an
	 * enabled one the muted text, and a disabled one the canvas's dimmest. */
	function letterColor(letter: RailLetter): string {
		if (letter.letter === marked) {
			return canvasVar('accent');
		}
		return letter.rowId === null ? '#4f5b58' : canvasVar('muted');
	}
</script>

<nav
	class="flex flex-none flex-col items-center"
	aria-label="Jump to letter"
	style:width="28px"
	style:padding-top="4px"
	data-library-region="rail"
>
	{#each letters as letter (letter.letter)}
		<button
			type="button"
			class="flex items-center justify-center font-mono"
			style:width="24px"
			style:height="22px"
			style:padding="0"
			style:border="0"
			style:border-radius="3px"
			style:font-size="11px"
			style:cursor={letter.rowId === null ? 'default' : 'pointer'}
			style:background={letter.letter === marked ? canvasVar('selected') : 'transparent'}
			style:color={letterColor(letter)}
			disabled={letter.rowId === null}
			data-library-rail-letter={letter.letter}
			aria-current={letter.letter === marked ? 'true' : undefined}
			onclick={() => jump(letter.letter)}
		>
			{letter.letter}
		</button>
	{/each}
</nav>
