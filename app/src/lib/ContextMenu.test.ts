import { fireEvent, render, screen } from '@testing-library/svelte';
import { expect, test, vi } from 'vitest';
import ContextMenu from './ContextMenu.svelte';

const setup = () => {
  const show = vi.fn();
  const remove = vi.fn();
  const onclose = vi.fn();
  render(ContextMenu, {
    props: {
      x: 10, y: 10, onclose,
      items: [{ label: 'Show in folder', run: show }, { label: 'Remove from history', run: remove, danger: true }],
    },
  });
  return { show, remove, onclose };
};

test('lists the actions as a menu and runs the clicked one', async () => {
  const { remove, onclose } = setup();
  expect(screen.getByRole('menu')).toBeInTheDocument();
  expect(screen.getAllByRole('menuitem').map((m) => m.textContent)).toEqual(['Show in folder', 'Remove from history']);
  await fireEvent.click(screen.getByRole('menuitem', { name: 'Remove from history' }));
  expect(remove).toHaveBeenCalled();
  expect(onclose).toHaveBeenCalled();
});

test('keyboard: first item focused, arrows move, Enter runs, Escape closes', async () => {
  const { show, onclose } = setup();
  const items = screen.getAllByRole('menuitem');
  expect(items[0]).toHaveFocus();
  await fireEvent.keyDown(screen.getByRole('menu'), { key: 'ArrowDown' });
  expect(items[1]).toHaveFocus();
  await fireEvent.keyDown(screen.getByRole('menu'), { key: 'ArrowDown' });
  expect(items[0]).toHaveFocus();
  await fireEvent.keyDown(items[0], { key: 'Enter' });
  expect(show).toHaveBeenCalled();
  await fireEvent.keyDown(screen.getByRole('menu'), { key: 'Escape' });
  expect(onclose).toHaveBeenCalled();
});
