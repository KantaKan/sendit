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
