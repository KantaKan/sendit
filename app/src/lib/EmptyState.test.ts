import { fireEvent, render, screen } from '@testing-library/svelte';
import { expect, test, vi } from 'vitest';
import EmptyState from './EmptyState.svelte';
import type { AppInfo } from './types';

const ME: AppInfo = {
  alias: 'fia-pc', fingerprint: 'C0FFEE'.padEnd(64, '0'), port: 53317, portFallback: false,
  downloadDir: 'C:/d', addresses: ['172.28.0.1', '192.168.1.101'],
};
const props = (over = {}) => ({
  me: ME,
  hasDevices: false,
  searching: false,
  onrefresh: vi.fn(),
  onscan: vi.fn(),
  onaddpeer: vi.fn(async () => {}),
  ...over,
});

test('shows how others see us and how to get found', async () => {
  const p = props();
  render(EmptyState, { props: p });
  expect(screen.getByRole('heading', { name: /Looking on this Wi-Fi/ })).toBeInTheDocument();
  expect(screen.getByText('fia-pc')).toBeInTheDocument();
  expect(screen.getByText('192.168.1.101')).toBeInTheDocument();
  expect(screen.getByText('VISIBLE')).toBeInTheDocument();
  expect(screen.getByText(/keep LocalSend on screen/)).toBeInTheDocument();
  expect(screen.getByText(/to send anything you copied/)).toBeInTheDocument();
  await fireEvent.click(screen.getByRole('button', { name: 'Refresh' }));
  await fireEvent.click(screen.getByRole('button', { name: 'Scan network' }));
  expect(p.onrefresh).toHaveBeenCalled();
  expect(p.onscan).toHaveBeenCalled();
});

test('add by IP opens a field and shows errors under it', async () => {
  const p = props({ onaddpeer: vi.fn(async () => Promise.reject('could not reach 10.0.0.9')) });
  render(EmptyState, { props: p });
  expect(screen.queryByLabelText('Add by IP address')).not.toBeInTheDocument();
  await fireEvent.click(screen.getByRole('button', { name: 'add by IP' }));
  await fireEvent.input(screen.getByLabelText('Add by IP address'), { target: { value: '10.0.0.9' } });
  await fireEvent.click(screen.getByRole('button', { name: 'Add' }));
  expect(await screen.findByText('could not reach 10.0.0.9')).toBeInTheDocument();
  expect(p.onaddpeer).toHaveBeenCalledWith('10.0.0.9');
});

test('with devices but none selected, asks to pick one', () => {
  render(EmptyState, { props: props({ hasDevices: true }) });
  expect(screen.getByRole('heading', { name: 'Pick a device on the left' })).toBeInTheDocument();
});
