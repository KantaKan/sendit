import { fireEvent, render, screen } from '@testing-library/svelte';
import { expect, test, vi } from 'vitest';
import { file } from './fixtures';
import HistoryRow from './HistoryRow.svelte';
import type { Ticket } from './state';

const at = new Date(2026, 9, 23, 14, 2).getTime();
const done: Ticket = {
  key: 'a', transferId: 'a', peer: 'P', direction: 'incoming', status: 'done', files: [file('IMG_2031.jpg', 4_200_000)],
  total: 4_200_000, bytes: 4_200_000, paths: ['C:/d/IMG_2031.jpg'], sources: [], at, doneAt: at,
};
const h = () => ({ onretry: vi.fn(), onreveal: vi.fn() });

test('received photo: thumbnail, direction, size, time, and Show', async () => {
  const x = h();
  render(HistoryRow, { props: { ticket: done, thumb: 'asset://IMG_2031.jpg', ...x } });
  expect(screen.getByText('IMG_2031.jpg')).toBeInTheDocument();
  expect(screen.getByLabelText('received')).toBeInTheDocument();
  expect(screen.getByText('4.2 MB')).toBeInTheDocument();
  expect(screen.getByText('14:02')).toBeInTheDocument();
  expect(document.querySelector('img')).toHaveAttribute('src', 'asset://IMG_2031.jpg');
  await fireEvent.click(screen.getByRole('button', { name: 'Show' }));
  expect(x.onreveal).toHaveBeenCalledWith('C:/d/IMG_2031.jpg');
});

test('sent file shows the sent arrow and the type tile', () => {
  const t: Ticket = { ...done, direction: 'outgoing', files: [file('thesis.pdf', 2_100_000)], total: 2_100_000, sources: ['C:/t.pdf'] };
  render(HistoryRow, { props: { ticket: t, thumb: null, ...h() } });
  expect(screen.getByLabelText('sent')).toBeInTheDocument();
  expect(screen.getByText('PDF')).toBeInTheDocument();
});

test('just finished rows flash a check; old ones do not', () => {
  const { unmount } = render(HistoryRow, { props: { ticket: { ...done, doneAt: Date.now() }, thumb: null, ...h() } });
  expect(screen.getByText('✓')).toBeInTheDocument();
  unmount();
  render(HistoryRow, { props: { ticket: done, thumb: null, ...h() } });
  expect(screen.queryByText('✓')).not.toBeInTheDocument();
});

test('failed send explains and retries', async () => {
  const x = h();
  const t: Ticket = { ...done, direction: 'outgoing', status: 'failed', reason: { kind: 'peerGone' }, sources: ['C:/a'] };
  render(HistoryRow, { props: { ticket: t, thumb: null, ...x } });
  expect(screen.getByText('Connection lost')).toBeInTheDocument();
  await fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
  expect(x.onretry).toHaveBeenCalled();
});

test('declined shows as such', () => {
  render(HistoryRow, { props: { ticket: { ...done, direction: 'outgoing', status: 'declined' }, thumb: null, ...h() } });
  expect(screen.getByText('Declined')).toBeInTheDocument();
});

test('a message row shows the text and copies it', async () => {
  const writeText = vi.fn(async () => {});
  Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
  const t: Ticket = { ...done, status: 'message', text: 'ได้แล้ว ขอบคุณ', files: [], total: 0, senderVerified: false };
  render(HistoryRow, { props: { ticket: t, thumb: null, ...h() } });
  expect(screen.getByText('ได้แล้ว ขอบคุณ')).toBeInTheDocument();
  expect(screen.getByText(/unverified/)).toBeInTheDocument();
  await fireEvent.click(screen.getByRole('button', { name: 'Copy' }));
  expect(writeText).toHaveBeenCalledWith('ได้แล้ว ขอบคุณ');
});

test('a thumbnail that fails to load falls back to the type tile', async () => {
  render(HistoryRow, { props: { ticket: done, thumb: 'asset://gone.jpg', ...h() } });
  await fireEvent.error(document.querySelector('img')!);
  expect(screen.getByText('JPG')).toBeInTheDocument();
});

test('right-click asks for the menu with the row and the pointer position', async () => {
  const onmenu = vi.fn();
  const { container } = render(HistoryRow, { props: { ticket: done, thumb: null, ...h(), onmenu } });
  await fireEvent.contextMenu(container.querySelector('.row')!, { clientX: 50, clientY: 60 });
  expect(onmenu).toHaveBeenCalledWith(done, 50, 60);
});
