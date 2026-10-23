import { fireEvent, render, screen } from '@testing-library/svelte';
import { expect, test, vi } from 'vitest';
import { file } from './fixtures';
import NowCard from './NowCard.svelte';
import type { Ticket } from './state';

const live: Ticket = {
  key: 't', transferId: 't', peer: 'P', direction: 'outgoing', status: 'live', files: [file('screenrec.mp4', 318_000_000)],
  total: 318_000_000, bytes: 204_000_000, paths: [], sources: ['C:/s.mp4'], at: 0, speeds: [20_000_000, 22_000_000],
};

test('shows progress, speed and time left, and cancels', async () => {
  const oncancel = vi.fn();
  render(NowCard, { props: { ticket: live, peerAlias: 'iPhone', oncancel } });
  expect(screen.getByText('screenrec.mp4')).toBeInTheDocument();
  expect(screen.getByText(/204 of 318 MB/)).toBeInTheDocument();
  expect(screen.getByText(/22\.0 MB\/s/)).toBeInTheDocument();
  expect(screen.getByText(/0:05 left/)).toBeInTheDocument();
  expect(screen.getByRole('progressbar')).toHaveAttribute('aria-valuenow', '64');
  await fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
  expect(oncancel).toHaveBeenCalled();
});

test('outgoing file not yet accepted says who it waits for', () => {
  render(NowCard, { props: { ticket: { ...live, bytes: 0, speeds: [] }, peerAlias: 'iPhone', oncancel: vi.fn() } });
  expect(screen.getByText(/Waiting for iPhone to accept/)).toBeInTheDocument();
});

test('a message being sent', () => {
  const t: Ticket = { ...live, transferId: undefined, messageId: 'm', status: 'sending', text: 'on its way', files: [], total: 0, bytes: 0 };
  render(NowCard, { props: { ticket: t, peerAlias: 'iPhone', oncancel: vi.fn() } });
  expect(screen.getByText('on its way')).toBeInTheDocument();
  expect(screen.getByText(/Sending/)).toBeInTheDocument();
  expect(screen.queryByRole('button', { name: 'Cancel' })).not.toBeInTheDocument();
});
