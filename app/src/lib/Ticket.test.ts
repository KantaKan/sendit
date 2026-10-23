import { fireEvent, render, screen } from '@testing-library/svelte';
import { expect, test, vi } from 'vitest';
import { file } from './fixtures';
import type { Ticket as T } from './state';
import Ticket from './Ticket.svelte';

const base: T = {
  key: 'k', peer: 'P', direction: 'incoming', files: [file('IMG_2044.jpg', 3_900_000)], total: 3_900_000,
  bytes: 0, status: 'pending', paths: [], sources: [], at: 0, requestId: 'r1', senderVerified: true, fitsOnDisk: true,
};
const handlers = () => ({
  onaccept: vi.fn(), ondecline: vi.fn(), onelsewhere: vi.fn(), oncancel: vi.fn(), onretry: vi.fn(), onreveal: vi.fn(),
});

test('pending request offers accept, decline and another folder', async () => {
  const h = handlers();
  render(Ticket, { props: { ticket: base, destLabel: 'Downloads/sendit', ...h } });
  expect(screen.getByText('wants to send 1 file')).toBeInTheDocument();
  await fireEvent.click(screen.getByRole('button', { name: 'Accept' }));
  await fireEvent.click(screen.getByRole('button', { name: 'Decline' }));
  await fireEvent.click(screen.getByRole('button', { name: 'to Downloads/sendit' }));
  expect(h.onaccept).toHaveBeenCalled();
  expect(h.ondecline).toHaveBeenCalled();
  expect(h.onelsewhere).toHaveBeenCalled();
});

test('huge batch lists four files and a count', () => {
  const files = Array.from({ length: 500 }, (_, i) => file(`ไฟล์ภาพยาวมาก ${i} ${'x'.repeat(200)}.jpg`, 1, String(i)));
  render(Ticket, { props: { ticket: { ...base, files }, destLabel: 'd', ...handlers() } });
  expect(screen.getByText('wants to send 500 files')).toBeInTheDocument();
  expect(screen.getAllByRole('listitem')).toHaveLength(5);
  expect(screen.getByText('and 496 more')).toBeInTheDocument();
});

test('unverified sender and full disk are called out', () => {
  render(Ticket, { props: { ticket: { ...base, senderVerified: false, fitsOnDisk: false }, destLabel: 'Downloads/sendit', ...handlers() } });
  expect(screen.getByText('Unverified sender')).toBeInTheDocument();
  expect(screen.getByText('Not enough free space in Downloads/sendit')).toBeInTheDocument();
});

test('accepting disables the buttons', () => {
  render(Ticket, { props: { ticket: { ...base, status: 'accepting' }, destLabel: 'd', ...handlers() } });
  expect(screen.getByRole('button', { name: 'Starting…' })).toBeDisabled();
});

test('live ticket shows percent and can be cancelled', async () => {
  const h = handlers();
  render(Ticket, { props: { ticket: { ...base, status: 'live', transferId: 't1', bytes: 1_950_000 }, destLabel: 'd', ...h } });
  expect(screen.getByText('50%')).toBeInTheDocument();
  await fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
  expect(h.oncancel).toHaveBeenCalled();
});

test('done incoming ticket reveals the file', async () => {
  const h = handlers();
  render(Ticket, { props: { ticket: { ...base, status: 'done', paths: ['C:/d/IMG_2044.jpg'] }, destLabel: 'd', ...h } });
  await fireEvent.click(screen.getByRole('button', { name: 'Show in folder' }));
  expect(h.onreveal).toHaveBeenCalledWith('C:/d/IMG_2044.jpg');
});

test('failed outgoing ticket explains and retries', async () => {
  const h = handlers();
  const t: T = { ...base, direction: 'outgoing', status: 'failed', reason: { kind: 'peerGone' }, sources: ['C:/a.jpg'] };
  render(Ticket, { props: { ticket: t, destLabel: 'd', ...h } });
  expect(screen.getByText('Connection lost')).toBeInTheDocument();
  await fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
  expect(h.onretry).toHaveBeenCalled();
});

test('message shows the text, not a file, and copies it', async () => {
  const writeText = vi.fn(async () => {});
  Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
  const t: T = { ...base, status: 'message', text: 'line one\nline two', files: [], total: 0, senderVerified: true };
  render(Ticket, { props: { ticket: t, destLabel: 'd', ...handlers() } });
  expect(screen.getByText(/line one/)).toBeInTheDocument();
  expect(screen.queryByText('Show in folder')).not.toBeInTheDocument();
  await fireEvent.click(screen.getByRole('button', { name: 'Copy' }));
  expect(writeText).toHaveBeenCalledWith('line one\nline two');
  expect(await screen.findByRole('button', { name: 'Copied' })).toBeInTheDocument();
});

test('outgoing message sits on the right', () => {
  const t: T = { ...base, direction: 'outgoing', status: 'message', text: 'sent text', files: [], total: 0, senderVerified: undefined };
  const { container } = render(Ticket, { props: { ticket: t, destLabel: 'd', ...handlers() } });
  expect(screen.getByText('sent text')).toBeInTheDocument();
  expect(container.querySelector('.row.out')).not.toBeNull();
  expect(screen.queryByText(/unverified/)).not.toBeInTheDocument();
});
