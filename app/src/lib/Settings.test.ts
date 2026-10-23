import { fireEvent, render, screen } from '@testing-library/svelte';
import { flushSync } from 'svelte';
import { expect, test } from 'vitest';
import { createMockBridge, type Seed } from './bridge-mock';
import { device } from './fixtures';
import Settings from './Settings.svelte';
import { createStore } from './store.svelte';

const tick = () => new Promise((r) => setTimeout(r, 0));
const MAC = { fingerprint: '5B1E'.padEnd(64, '2'), alias: "Nont's MacBook", trust: 'verified' as const, autoAccept: false };
const PHONE = { fingerprint: 'E7D4'.padEnd(64, '8'), alias: 'iPhone', trust: 'known' as const, autoAccept: false };

async function setup(seed: Seed = {}) {
  const m = createMockBridge({ devices: [device()], known: [MAC, PHONE], ...seed });
  const store = createStore(m.bridge);
  store.start();
  await tick();
  flushSync();
  render(Settings, { props: { store } });
  await tick();
  return { m, store };
}

test('shows the current settings', async () => {
  await setup();
  expect(screen.getByLabelText('Device name')).toHaveValue('my-pc');
  expect(screen.getByText('C:\\Users\\me\\Downloads\\sendit')).toBeInTheDocument();
  expect(screen.getByRole('switch', { name: 'Require a PIN' })).toHaveAttribute('aria-checked', 'false');
  expect(screen.getByRole('button', { name: 'Keep running in tray' })).toHaveAttribute('aria-pressed', 'true');
  expect(screen.getByRole('switch', { name: 'Start sendit when I sign in' })).toHaveAttribute('aria-checked', 'false');
});

test('renaming saves the new name', async () => {
  const { m, store } = await setup();
  const name = screen.getByLabelText('Device name');
  await fireEvent.input(name, { target: { value: 'Studio PC' } });
  await fireEvent.blur(name);
  await tick();
  expect(m.calls.find((c) => c.name === 'updateSettings')!.args[0]).toMatchObject({ alias: 'Studio PC' });
  expect(store.me?.alias).toBe('Studio PC');
});

test('a rejected name shows the error under the field and keeps the saved one', async () => {
  const { store } = await setup({ settingsError: "Device name can't be empty" });
  const name = screen.getByLabelText('Device name');
  await fireEvent.input(name, { target: { value: ' ' } });
  await fireEvent.blur(name);
  await tick();
  expect(screen.getByText("Device name can't be empty")).toBeInTheDocument();
  expect(store.settings?.alias).toBe('my-pc');
});

test('choosing a folder saves it', async () => {
  const { m } = await setup({ pickedFolder: 'D:\\inbox' });
  await fireEvent.click(screen.getByRole('button', { name: 'Change…' }));
  await tick();
  expect(m.calls.find((c) => c.name === 'updateSettings')!.args[0]).toMatchObject({ downloadDir: 'D:\\inbox' });
  expect(await screen.findByText('D:\\inbox')).toBeInTheDocument();
});

test('a PIN is saved once typed and removed when switched off', async () => {
  const { m } = await setup();
  await fireEvent.click(screen.getByRole('switch', { name: 'Require a PIN' }));
  const pin = screen.getByLabelText('PIN');
  await fireEvent.input(pin, { target: { value: '4321' } });
  await fireEvent.blur(pin);
  await tick();
  await fireEvent.click(screen.getByRole('switch', { name: 'Require a PIN' }));
  await tick();
  const pins = m.calls.filter((c) => c.name === 'updateSettings').map((c) => (c.args[0] as { pin: string | null }).pin);
  expect(pins).toEqual(['4321', null]);
});

test('auto-accept can only be turned on for verified devices', async () => {
  const { m } = await setup();
  const phone = screen.getByRole('switch', { name: 'Auto-accept iPhone' });
  expect(phone).toBeDisabled();
  expect(screen.getByText('verify it first to allow')).toBeInTheDocument();
  await fireEvent.click(screen.getByRole('switch', { name: "Auto-accept Nont's MacBook" }));
  expect(m.calls).toContainEqual({ name: 'setAutoAccept', args: [MAC.fingerprint, true] });
});

test('start on sign-in snaps back when the OS refuses', async () => {
  const { store } = await setup({ autostartError: 'access denied' });
  const sw = screen.getByRole('switch', { name: 'Start sendit when I sign in' });
  await fireEvent.click(sw);
  await tick();
  expect(sw).toHaveAttribute('aria-checked', 'false');
  expect(store.error).toBe('access denied');
});

test('close button choice is saved', async () => {
  const { m } = await setup();
  await fireEvent.click(screen.getByRole('button', { name: 'Quit' }));
  await tick();
  expect(m.calls.find((c) => c.name === 'updateSettings')!.args[0]).toMatchObject({ closeToTray: false });
  expect(screen.getByRole('button', { name: 'Quit' })).toHaveAttribute('aria-pressed', 'true');
});

test('clear history asks first', async () => {
  const { store } = await setup({
    history: [{ key: 'old', peer: 'P', direction: 'incoming', files: [], total: 1, bytes: 1, status: 'done', paths: [], sources: [], at: 1 }],
  });
  await fireEvent.click(screen.getByRole('button', { name: 'Clear history' }));
  expect(store.state.tickets).toHaveLength(1);
  await fireEvent.click(screen.getByRole('button', { name: 'Yes, clear' }));
  expect(store.state.tickets).toHaveLength(0);
});
