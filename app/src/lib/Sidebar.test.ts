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
      seenBefore: [{ fingerprint: 'B'.repeat(64), alias: 'Lab PC', trust: 'known', autoAccept: false }],
      selected: pixel.fingerprint,
      searching: false,
      waiting: { ['D'.repeat(64)]: 2 },
      onselect,
      onrefresh: vi.fn(),
      settingsOpen: false,
      onsettings: vi.fn(),
    },
  });
  expect(screen.getByRole('button', { name: /Pixel 8/ })).toHaveAttribute('aria-current', 'true');
  expect(screen.getByText('key changed')).toBeInTheDocument();
  expect(screen.getByText('Lab PC')).toBeInTheDocument();
  expect(screen.getByLabelText('2 waiting')).toHaveTextContent('2');
  await fireEvent.click(screen.getByRole('button', { name: /iPhone/ }));
  expect(onselect).toHaveBeenCalledWith('D'.repeat(64));
});

test('the Settings button opens settings and Refresh stays reachable', async () => {
  const onsettings = vi.fn();
  const onrefresh = vi.fn();
  render(Sidebar, {
    props: {
      me: null, nearby: [device()], seenBefore: [], selected: null, searching: false, waiting: {},
      onselect: vi.fn(), onrefresh, onsettings, settingsOpen: true,
    },
  });
  const btn = screen.getByRole('button', { name: 'Settings' });
  expect(btn).toHaveAttribute('aria-pressed', 'true');
  await fireEvent.click(btn);
  expect(onsettings).toHaveBeenCalled();
  await fireEvent.click(screen.getByRole('button', { name: 'Refresh' }));
  expect(onrefresh).toHaveBeenCalled();
});
