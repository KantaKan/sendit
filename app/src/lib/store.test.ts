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

test('paste with no device selected explains and sends nothing', async () => {
  const { m, store } = await setup();
  await store.paste();
  expect(store.error).toBe('Pick a device first, then paste.');
  expect(m.calls.filter((c) => c.name === 'paste')).toEqual([]);
});

test('paste sends to the open conversation without a toast', async () => {
  const { m, store } = await setup({ pasted: { kind: 'files', count: 2, name: 'a.pdf' } });
  store.select(PIXEL.fingerprint);
  await store.paste();
  expect(m.calls).toContainEqual({ name: 'paste', args: [PIXEL.fingerprint] });
  expect(store.notice).toBeNull();
});

test('pasting an empty clipboard leaves a short notice', async () => {
  const { store } = await setup({ pasted: { kind: 'empty' } });
  store.select(PIXEL.fingerprint);
  await store.paste();
  expect(store.notice).toBe('Nothing to paste. Copy a file, image or text first.');
});

test('retrying a failed message sends the same text again', async () => {
  const { m, store } = await setup();
  m.emit({ type: 'messageSending', id: 'm1', peer: PIXEL.fingerprint, text: 'yo' });
  m.emit({ type: 'messageFailed', id: 'm1', reason: { kind: 'peerGone' } });
  store.retry(ticketsFor(store.state, PIXEL.fingerprint)[0]);
  await tick();
  expect(m.calls).toContainEqual({ name: 'sendText', args: [PIXEL.fingerprint, 'yo'] });
});

test('background send errors show up as the error banner', async () => {
  const { m, store } = await setup();
  m.appError('device is blocked');
  expect(store.error).toBe('device is blocked');
});

const DONE = {
  key: 'old', transferId: 'old', peer: PIXEL.fingerprint, direction: 'incoming' as const, files: [file('a.jpg', 10)],
  total: 10, bytes: 10, status: 'done' as const, paths: ['C:/d/a.jpg'], sources: [], at: 1, doneAt: 1,
};
const msg = { type: 'messageReceived' as const, from: PIXEL, text: 'hi', senderVerified: true };
const saves = (m: ReturnType<typeof createMockBridge>) => m.calls.filter((c) => c.name === 'saveHistory');

test('saved history shows up in the conversation', async () => {
  const { store } = await setup({ history: [DONE] });
  expect(ticketsFor(store.state, PIXEL.fingerprint).map((t) => t.key)).toEqual(['old']);
});

test('history is not saved before the saved history has loaded', async () => {
  vi.useFakeTimers();
  try {
    const m = createMockBridge({ devices: [PIXEL], history: [DONE], historyDelayMs: 5000 });
    const store = createStore(m.bridge);
    store.start();
    m.emit(msg);
    await vi.advanceTimersByTimeAsync(2000);
    expect(saves(m)).toEqual([]);
    await vi.advanceTimersByTimeAsync(5000);
    const last = saves(m).at(-1)!.args[0] as { key: string }[];
    expect(last.map((t) => t.key)).toEqual(['old', expect.stringMatching(/^msg:/)]);
  } finally {
    vi.useRealTimers();
  }
});

test('a failed history save shows the banner once', async () => {
  vi.useFakeTimers();
  try {
    const m = createMockBridge({ devices: [PIXEL], saveHistoryError: 'disk full' });
    const store = createStore(m.bridge);
    store.start();
    await vi.advanceTimersByTimeAsync(0);
    m.emit(msg);
    await vi.advanceTimersByTimeAsync(1100);
    expect(store.error).toBe('Could not save history: disk full');
    store.dismissError();
    m.emit({ ...msg, text: 'again' });
    await vi.advanceTimersByTimeAsync(1100);
    expect(saves(m).length).toBe(2);
    expect(store.error).toBeNull();
  } finally {
    vi.useRealTimers();
  }
});

test('clear history forgets finished items and saves', async () => {
  vi.useFakeTimers();
  try {
    const m = createMockBridge({ devices: [PIXEL], history: [DONE] });
    const store = createStore(m.bridge);
    store.start();
    await vi.advanceTimersByTimeAsync(0);
    m.emit({ type: 'incomingRequest', request: req });
    store.clearHistory();
    await vi.advanceTimersByTimeAsync(1100);
    expect(store.state.tickets.map((t) => t.status)).toEqual(['pending']);
    expect(saves(m).at(-1)!.args[0]).toEqual([]);
  } finally {
    vi.useRealTimers();
  }
});

test('two quick settings changes both stick', async () => {
  const { m, store } = await setup();
  const a = store.updateSettings({ alias: 'Studio' });
  const b = store.updateSettings({ closeToTray: false });
  await Promise.all([a, b]);
  expect(store.settings).toMatchObject({ alias: 'Studio', closeToTray: false });
  expect(m.calls.filter((c) => c.name === 'updateSettings').at(-1)!.args[0]).toMatchObject({ alias: 'Studio', closeToTray: false });
});

test('a device that is not nearby opens with its history, and sending to it explains why not', async () => {
  const LAB = { fingerprint: 'B'.repeat(64), alias: 'Lab PC', trust: 'known' as const, autoAccept: false };
  const { m, store } = await setup({ known: [LAB], history: [{ ...DONE, peer: LAB.fingerprint }] });
  store.select(LAB.fingerprint);
  expect(store.selectedDevice?.alias).toBe('Lab PC');
  expect(ticketsFor(store.state, LAB.fingerprint)).toHaveLength(1);
  m.drag({ kind: 'drop', paths: ['C:/a.txt'] });
  await store.paste();
  await tick();
  expect(store.error).toBe('Lab PC is not nearby right now.');
  expect(m.calls.filter((c) => c.name === 'sendFiles' || c.name === 'paste')).toEqual([]);
});

test('the store is ready once this device and the nearby list have loaded', async () => {
  const m = createMockBridge({ devices: [PIXEL] });
  const store = createStore(m.bridge);
  expect(store.ready).toBe(false);
  store.start();
  await tick();
  expect(store.ready).toBe(true);
});

test('skipping face verification is remembered across conversations', async () => {
  const { store } = await setup();
  expect(store.verifySkipped(PIXEL.fingerprint)).toBe(false);
  store.skipVerify(PIXEL.fingerprint);
  expect(store.verifySkipped(PIXEL.fingerprint)).toBe(true);
});
