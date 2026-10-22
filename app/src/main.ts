import { mount } from 'svelte';
import App from './App.svelte';
import './app.css';

async function boot() {
  const bridge =
    import.meta.env.MODE === 'mock'
      ? (await import('./lib/bridge-mock')).installMockBridge()
      : (await import('./lib/bridge-tauri')).tauriBridge;
  mount(App, { target: document.getElementById('app')!, props: { bridge } });
}

void boot();
