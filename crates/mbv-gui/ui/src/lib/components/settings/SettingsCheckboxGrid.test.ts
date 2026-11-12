// add-gui-settings-screen row 9.3: the shared Settings checkbox grid (design
// D13, the canvas My languages card). Each box carries `role="checkbox"` and
// `aria-checked` from the window's marks, and a press on a box or on Apply
// reports the entry it names: the grid holds no marks of its own, so the
// keyboard and the pointer share one store (the spec's *Choice-list settings
// are edited in place*).

import { fireEvent, render } from '@testing-library/svelte';
import { expect, test, vi } from 'vitest';
import SettingsCheckboxGrid from './SettingsCheckboxGrid.svelte';

/** The boxes of one grid: two languages, one marked by the window. */
const OPTIONS = [
	{ id: 'english', label: 'English' },
	{ id: 'french', label: 'French' },
];

function grid(onpress: (id: string) => void) {
	return render(SettingsCheckboxGrid, {
		props: {
			title: 'My languages',
			rowKey: 'myLanguages',
			options: OPTIONS,
			selected: ['english'],
			cursorId: null,
			onpress,
		},
	});
}

test('the boxes carry role="checkbox" and the window’s aria-checked', () => {
	const { container } = grid(() => {});

	const boxes = container.querySelectorAll('[role="checkbox"]');
	expect(boxes[0].getAttribute('aria-checked')).toBe('true');
	expect(boxes[1].getAttribute('aria-checked')).toBe('false');
});

test('a press on a box reports its choice entry', async () => {
	const onpress = vi.fn();
	const { container } = grid(onpress);

	await fireEvent.click(container.querySelector('[data-settings-checkbox="french"]') as HTMLElement);

	expect(onpress).toHaveBeenCalledWith('choice:myLanguages:french');
});

test('a press on Apply reports the apply entry', async () => {
	const onpress = vi.fn();
	const { container } = grid(onpress);

	await fireEvent.click(container.querySelector('[data-settings-button="primary"]') as HTMLElement);

	expect(onpress).toHaveBeenCalledWith('apply:myLanguages');
});
