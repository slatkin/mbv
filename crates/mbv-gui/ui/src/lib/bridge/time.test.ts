// align-gui-with-design-canvas U1.5: the GUI's remaining-time glyph is the
// canvas's U+2212, never the ASCII hyphen-minus (the scrubber's right span and
// the collapsed rail's remaining time paint through the helper).

import { expect, test } from 'vitest';
import { minus } from './time';

test('a remaining time carries the canvas minus sign', () => {
	expect(minus('22:14')).toBe('\u221222:14');
	expect(minus('22m')).not.toContain('-');
});
