// add-gui-settings-screen row 9.3: the shared Settings checkbox grid (design
// D13, the canvas My languages card). Each box carries `role="checkbox"` and
// `aria-checked`, a press flips the held mark, and Apply calls the apply
// handler with the marked ids — the ids the `applyChoices` command carries.

import { fireEvent, render } from '@testing-library/svelte';
import { expect, test, vi } from 'vitest';
import SettingsCheckboxGrid from './SettingsCheckboxGrid.svelte';

/** The boxes of one grid: two languages, one marked by the frame. */
const OPTIONS = [
	{ id: 'english', label: 'English' },
	{ id: 'french', label: 'French' },
];

function grid(onapply: (ids: string[]) => void) {
	return render(SettingsCheckboxGrid, {
		props: {
			title: 'My languages',
			options: OPTIONS,
			selected: ['english'],
			onapply,
		},
	});
}

test('the boxes carry role="checkbox" and the frame’s aria-checked', () => {
	const { container } = grid(() => {});

	const boxes = container.querySelectorAll('[role="checkbox"]');
	expect(boxes[0].getAttribute('aria-checked')).toBe('true');
	expect(boxes[1].getAttribute('aria-checked')).toBe('false');
});

test('a press flips one mark and Apply calls the apply handler with the marked ids', async () => {
	const onapply = vi.fn();
	const { container } = grid(onapply);

	await fireEvent.click(container.querySelector('[data-settings-checkbox="french"]') as HTMLElement);
	expect(
		container
			.querySelector('[data-settings-checkbox="french"]')
			?.getAttribute('aria-checked'),
	).toBe('true');

	await fireEvent.click(container.querySelector('[data-settings-button="primary"]') as HTMLElement);

	expect(onapply).toHaveBeenCalledWith(['english', 'french']);
});
