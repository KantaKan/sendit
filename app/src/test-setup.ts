import '@testing-library/jest-dom/vitest';
import '@testing-library/svelte/vitest';

// jsdom has no scrolling
Element.prototype.scrollTo = () => {};
