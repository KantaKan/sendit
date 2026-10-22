import { render, screen } from '@testing-library/svelte';
import { expect, test } from 'vitest';
import PortBanner from './PortBanner.svelte';

test('explains the port fallback', () => {
  render(PortBanner, { props: { port: 61234 } });
  expect(screen.getByRole('status')).toHaveTextContent('Port 53317 is busy');
  expect(screen.getByRole('status')).toHaveTextContent('61234');
});
