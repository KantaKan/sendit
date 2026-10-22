import { flushSync } from 'svelte';
import { expect, test, vi } from 'vitest';
import { createMockBridge } from './bridge-mock';
import { device, file } from './fixtures';
import { ticketsFor } from './state';
import { createStore } from './store.svelte';

const PIXEL = device();
const tick = () => new Promise((r) => setTimeout(r, 0));
const req = { requestId: 'r1', from: PIXEL, files: [file('a.jpg', 10)], totalSize: 10, fitsOnDisk: true, senderVerified: true };

async function setup(seed = {}) {
  const m = createMockBridge({ devices: [PIXEL], ...seed });
  const store = createStore(m.bridge);
  store.start();
  await tick();
  flushSync();
  return { m, store };
}

test('start loads info and devices', async () => {
  const { store } = await setup();
  expect(store.me?.alias).toBe('my-pc');
  expect(Object.keys(store.state.devices)).toEqual([PIXEL.fingerprint]);
});

test('drop with no device selected shows a message and sends nothing', async () => {
  const { m, store } = await setup();
  m.drag({ kind: 'drop', paths: ['C:/a.txt'] });
  expect(store.error).toBe('Pick a device first, then drop the files on it.');
  expect(m.calls.filter((c) => c.name === 'sendFiles')).toEqual([]);
});

test('drop with a device selected sends to it', async () => {
  const { m, store } = await setup();
  store.select(PIXEL.fingerprint);
  m.drag({ kind: 'drop', paths: ['C:/a.txt'] });
  await tick();
  expect(m.calls).toContainEqual({ name: 'sendFiles', args: [PIXEL.fingerprint, ['C:/a.txt']] });
});

test('incoming request selects the sender', async () => {
  const { m, store } = await setup();
  m.emit({ type: 'incomingRequest', request: req });
  expect(store.selected).toBe(PIXEL.fingerprint);
});

test('accept after core timeout marks the ticket expired', async () => {
  const { m, store } = await setup({ respondError: 'no pending request r1' });
  m.emit({ type: 'incomingRequest', request: req });
  store.accept('r1');
  await tick();
  expect(ticketsFor(store.state, PIXEL.fingerprint)[0].status).toBe('expired');
});

test('accept into a full disk shows the error and returns to pending', async () => {
  const { m, store } = await setup({ respondError: 'not enough disk space' });
  m.emit({ type: 'incomingRequest', request: req });
  store.accept('r1');
  await tick();
  expect(ticketsFor(store.state, PIXEL.fingerprint)[0].status).toBe('pending');
  expect(store.error).toBe('not enough disk space');
});

test('identity change moves the open conversation to the suspect key', async () => {
  const { m, store } = await setup();
  store.select(PIXEL.fingerprint);
  const suspect = { ...PIXEL, fingerprint: 'DEAD'.padEnd(64, '3'), trust: 'suspect' as const };
  m.emit({ type: 'identityChanged', alias: 'Pixel 8', oldFingerprint: PIXEL.fingerprint, device: suspect });
  expect(store.selected).toBe(suspect.fingerprint);
});

test('incoming request notifies only when the window is in the background', async () => {
  const { m } = await setup();
  const focus = vi.spyOn(document, 'hasFocus').mockReturnValue(false);
  m.emit({ type: 'incomingRequest', request: req });
  focus.mockReturnValue(true);
  m.emit({ type: 'incomingRequest', request: { ...req, requestId: 'r2' } });
  expect(m.calls.filter((c) => c.name === 'notify')).toEqual([
    { name: 'notify', args: ['Pixel 8 wants to send files', '1 file(s)'] },
  ]);
  focus.mockRestore();
});
