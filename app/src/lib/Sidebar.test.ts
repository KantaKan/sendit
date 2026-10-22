import { fireEvent, render, screen } from '@testing-library/svelte';
import { expect, test, vi } from 'vitest';
import { device } from './fixtures';
import Sidebar from './Sidebar.svelte';

test('lists nearby and seen-before devices and selects on click', async () => {
  const onselect = vi.fn();
  const pixel = device();
  const suspect = device({ fingerprint: 'D'.repeat(64), alias: 'iPhone', trust: 'suspect' });
  render(Sidebar, {
    props: {
      me: null,
      nearby: [pixel, suspect],
      seenBefore: [{ fingerprint: 'B'.repeat(64), alias: 'Lab PC', trust: 'known' }],
      selected: pixel.fingerprint,
      searching: false,
      waiting: { ['D'.repeat(64)]: 2 },
      onselect,
      onrefresh: vi.fn(),
    },
  });
  expect(screen.getByRole('button', { name: /Pixel 8/ })).toHaveAttribute('aria-current', 'true');
  expect(screen.getByText('key changed')).toBeInTheDocument();
  expect(screen.getByText('Lab PC')).toBeInTheDocument();
  expect(screen.getByLabelText('2 waiting')).toHaveTextContent('2');
  await fireEvent.click(screen.getByRole('button', { name: /iPhone/ }));
  expect(onselect).toHaveBeenCalledWith('D'.repeat(64));
});
