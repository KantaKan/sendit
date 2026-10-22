/// <reference types="svelte" />
/// <reference types="vite/client" />

// svelte-check 4 + TypeScript 6 does not resolve .svelte imports from .ts files
// on its own here; this keeps them typed as components.
declare module '*.svelte' {
  import type { Component } from 'svelte';
  const component: Component<any>;
  export default component;
}
