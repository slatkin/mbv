// The canvas's two bundled faces (add-tauri-gui-client D8): IBM Plex Sans
// for text, JetBrains Mono for times, key hints, chips, and counters — the
// latin subsets at the three weights the components use, so the build
// carries woff2 and the window fetches no font at runtime.
import '@fontsource/ibm-plex-sans/latin-400.css';
import '@fontsource/ibm-plex-sans/latin-500.css';
import '@fontsource/ibm-plex-sans/latin-600.css';
import '@fontsource/jetbrains-mono/latin-400.css';
import '@fontsource/jetbrains-mono/latin-500.css';
import '@fontsource/jetbrains-mono/latin-600.css';
import './app.css';
import { mount } from 'svelte';
import App from './App.svelte';
import { applyCellUnit } from './lib/bridge/cells';

const target = document.getElementById('app');
if (!target) {
  throw new Error('index.html has no #app mount point');
}

// The cell unit's CSS properties exist before the first paint, so every
// component's `calc()` over them resolves from the first frame.
applyCellUnit();

mount(App, { target });
