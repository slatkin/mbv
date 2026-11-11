/** A Svelte action: hand the keyboard to a form's first text field when the
 * form mounts, so a user who picked the action by key types without reaching
 * for the pointer (the spec's *keyboard reachability*). On an input it focuses
 * the input itself. A form mounts exactly when it opens. */
export function focusOnMount(node: HTMLElement): void {
	const input = node instanceof HTMLInputElement ? node : node.querySelector('input');
	input?.focus();
}
