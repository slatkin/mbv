// add-tauri-gui-client row 6.7 (design D8): the bundled faces are the
// window's only type source — the gui-client spec's "typography and colours
// of the canvas" requirement ("text and times show in the two bundled
// faces"). jsdom applies no CSS, so these tests pin the sources the build
// ships: `app.css`'s face declarations and `main.ts`'s bundled-face imports.
// The theme's `--mbv-*` application is `sections.test.ts`'s contract.

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test } from 'vitest';

// jsdom rewrites `import.meta.url` to an http URL, so the sources read
// relative to the package root — vitest's working directory.
const appCss = readFileSync(join(process.cwd(), 'src/app.css'), 'utf8');
const mainTs = readFileSync(join(process.cwd(), 'src/main.ts'), 'utf8');

test('the text face is IBM Plex Sans with the system-ui fallback', () => {
	expect(appCss).toMatch(/--font-sans:\s*'IBM Plex Sans',\s*system-ui/);
});

test('the mono face is JetBrains Mono with the monospace fallback', () => {
	expect(appCss).toMatch(/--font-mono:\s*'JetBrains Mono',\s*monospace/);
});

test('the bundle imports the two faces at the three used weights, latin subset', () => {
	for (const face of ['ibm-plex-sans', 'jetbrains-mono']) {
		for (const weight of [400, 500, 600]) {
			expect(mainTs).toContain(`@fontsource/${face}/latin-${weight}.css`);
		}
	}
});

test('no SUSE face remains', () => {
	expect(mainTs).not.toContain('suse');
	expect(appCss).not.toContain('SUSE');
});
