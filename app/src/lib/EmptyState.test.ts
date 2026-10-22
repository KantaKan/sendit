import { fireEvent, render, screen } from '@testing-library/svelte';
import { expect, test, vi } from 'vitest';
import EmptyState from './EmptyState.svelte';

const props = (over = {}) => ({
  hasDevices: false,
  searching: false,
  onrefresh: vi.fn(),
  onscan: vi.fn(),
  onaddpeer: vi.fn(async () => {}),
  ...over,
});

test('explains discovery and offers refresh and scan', async () => {
  const p = props();
  render(EmptyState, { props: p });
  expect(screen.getByText(/Devices on the same Wi-Fi show up here/)).toBeInTheDocument();
  await fireEvent.click(screen.getByRole('button', { name: 'Refresh' }));
  await fireEvent.click(screen.getByRole('button', { name: 'Scan network' }));
  expect(p.onrefresh).toHaveBeenCalled();
  expect(p.onscan).toHaveBeenCalled();
});

test('add by IP shows the error under the field', async () => {
  const p = props({ onaddpeer: vi.fn(async () => Promise.reject('could not reach 10.0.0.9')) });
  render(EmptyState, { props: p });
  await fireEvent.input(screen.getByLabelText('Add by IP address'), { target: { value: '10.0.0.9' } });
  await fireEvent.click(screen.getByRole('button', { name: 'Add' }));
  expect(await screen.findByText('could not reach 10.0.0.9')).toBeInTheDocument();
  expect(p.onaddpeer).toHaveBeenCalledWith('10.0.0.9');
});

test('with devices but none selected, asks to pick one', () => {
  render(EmptyState, { props: props({ hasDevices: true }) });
  expect(screen.getByText('Pick a device on the left to start.')).toBeInTheDocument();
});
