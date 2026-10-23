import { fireEvent, render, screen } from '@testing-library/svelte';
import { expect, test, vi } from 'vitest';
import { file } from './fixtures';
import RequestCard from './RequestCard.svelte';
import type { Ticket } from './state';

const base: Ticket = {
  key: 'k', peer: 'P', direction: 'incoming', requestId: 'r1', status: 'pending', bytes: 0, paths: [], sources: [], at: 0,
  files: [file('a.jpg', 40_000_000), file('b.png', 40_000_000), file('c.mp4', 16_000_000)], total: 96_000_000,
  senderVerified: true, fitsOnDisk: true,
};
const h = () => ({ onaccept: vi.fn(), ondecline: vi.fn(), onelsewhere: vi.fn() });

test('summarizes what is coming and answers', async () => {
  const x = h();
  render(RequestCard, { props: { ticket: base, peerAlias: 'iPhone', destLabel: 'Downloads/sendit', ...x } });
  expect(screen.getByText('iPhone wants to send 3 items')).toBeInTheDocument();
  expect(screen.getByText('2 photos, 1 video · 96 MB')).toBeInTheDocument();
  await fireEvent.click(screen.getByRole('button', { name: 'Accept' }));
  await fireEvent.click(screen.getByRole('button', { name: 'Decline' }));
  await fireEvent.click(screen.getByRole('button', { name: 'to Downloads/sendit' }));
  expect(x.onaccept).toHaveBeenCalled();
  expect(x.ondecline).toHaveBeenCalled();
  expect(x.onelsewhere).toHaveBeenCalled();
});

test('single file uses its name; warnings show', () => {
  render(RequestCard, {
    props: { ticket: { ...base, files: [file('report.pdf', 5)], total: 5, senderVerified: false, fitsOnDisk: false }, peerAlias: 'iPhone', destLabel: 'D', ...h() },
  });
  expect(screen.getByText('iPhone wants to send report.pdf')).toBeInTheDocument();
  expect(screen.getByText('Unverified sender')).toBeInTheDocument();
  expect(screen.getByText('Not enough free space in D')).toBeInTheDocument();
});

test('huge batch stays one line', () => {
  const files = Array.from({ length: 500 }, (_, i) => file(`f${i}.jpg`, 1, String(i)));
  render(RequestCard, { props: { ticket: { ...base, files, total: 500 }, peerAlias: 'iPhone', destLabel: 'D', ...h() } });
  expect(screen.getByText('iPhone wants to send 500 items')).toBeInTheDocument();
  expect(screen.getByText('500 photos · 500 B')).toBeInTheDocument();
});

test('accepting disables the buttons', () => {
  render(RequestCard, { props: { ticket: { ...base, status: 'accepting' }, peerAlias: 'iPhone', destLabel: 'D', ...h() } });
  expect(screen.getByRole('button', { name: 'Starting…' })).toBeDisabled();
});
