import '@fontsource-variable/suse';
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
