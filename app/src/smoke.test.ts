import { render, screen } from '@testing-library/svelte';
import { expect, test } from 'vitest';
import App from './App.svelte';

test('app renders', () => {
  render(App);
  expect(screen.getByRole('heading', { name: 'sendit' })).toBeInTheDocument();
});
