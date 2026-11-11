// The Status bar's shared paint facts (design D10): the frame's `GuiStatusBar`
// is carried to both surfaces that paint it — the Queue footer's line and the
// Library Status bar — so its toast takes one severity colour from here rather
// than one per painter.

import type { GuiToastSeverity } from './types';

/** The toast text's severity foreground (`toast_spans` in the TUI's status bar
 * panel). */
export function toastColor(severity: GuiToastSeverity): string {
	switch (severity) {
		case 'neutral':
			return 'var(--mbv-textPrimary)';
		case 'success':
			return 'var(--mbv-statusAvailable)';
		case 'warning':
			return 'var(--mbv-textFocusAccent)';
		case 'error':
			return 'var(--mbv-statusError)';
	}
}
