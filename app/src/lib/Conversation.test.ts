import { render, screen } from '@testing-library/svelte';
import { expect, test } from 'vitest';
import { createMockBridge } from './bridge-mock';
import Conversation from './Conversation.svelte';
import { device } from './fixtures';
import { createStore } from './store.svelte';

test('empty conversation invites drop or paste, and lights up while dragging', async () => {
  const store = createStore(createMockBridge().bridge);
  const { rerender } = render(Conversation, {
    props: { device: device(), tickets: [], me: null, store, dragging: false },
  });
  const zone = screen.getByText(/Drop here or/);
  expect(zone).toBeInTheDocument();
  expect(zone).not.toHaveClass('hot');
  await rerender({ device: device(), tickets: [], me: null, store, dragging: true });
  expect(screen.getByText(/Drop here or/)).toHaveClass('hot');
});

import { file } from './fixtures';
import type { Ticket } from './state';

test('requests on top, then Now, then Earlier', () => {
  const store = createStore(createMockBridge().bridge);
  const t = (over: Partial<Ticket>): Ticket => ({
    key: Math.random().toString(), peer: 'P', direction: 'outgoing', status: 'done', files: [file('x.pdf', 1)], total: 1,
    bytes: 1, paths: [], sources: [], at: 1, ...over,
  });
  const tickets = [
    t({ transferId: 'old', files: [file('old.pdf', 1)] }),
    t({ transferId: 'live', status: 'live', files: [file('live.mp4', 10)], total: 10, bytes: 0 }),
    t({ requestId: 'r', status: 'pending', direction: 'incoming', files: [file('req.jpg', 1)] }),
  ];
  render(Conversation, { props: { device: device(), tickets, me: null, store } });
  const text = document.body.textContent ?? '';
  expect(text.indexOf('wants to send')).toBeLessThan(text.indexOf('Now'));
  expect(text.indexOf('Now')).toBeLessThan(text.indexOf('live.mp4'));
  expect(text.indexOf('live.mp4')).toBeLessThan(text.indexOf('Earlier'));
  expect(text.indexOf('Earlier')).toBeLessThan(text.indexOf('old.pdf'));
});
