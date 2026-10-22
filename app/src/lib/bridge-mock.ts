import type { Bridge, DragState } from './bridge';
import type { AppInfo, CoreEvent, Device, KnownDevice } from './types';

export interface Seed {
  info?: Partial<AppInfo>;
  devices?: Device[];
  known?: KnownDevice[];
  pickedFiles?: string[];
  pickedFolder?: string | null;
  /** respond() rejects with this message */
  respondError?: string;
  /** addPeer() resolves with this device, otherwise rejects */
  addPeer?: Device;
}

export interface Call {
  name: string;
  args: unknown[];
}

export function createMockBridge(seed: Seed = {}) {
  const eventSubs = new Set<(e: CoreEvent) => void>();
  const dragSubs = new Set<(d: DragState) => void>();
  const calls: Call[] = [];
  const rec = (name: string, ...args: unknown[]) => {
    calls.push({ name, args });
  };
  const info: AppInfo = {
    alias: 'my-pc',
    fingerprint: 'C0FFEE'.padEnd(64, '0'),
    port: 53317,
    portFallback: false,
    downloadDir: 'C:\\Users\\me\\Downloads\\sendit',
    ...seed.info,
  };
  const bridge: Bridge = {
    info: async () => info,
    devices: async () => seed.devices ?? [],
    knownDevices: async () => seed.known ?? [],
    sendFiles: async (fp, paths) => rec('sendFiles', fp, paths),
    respond: async (id, accept, dest) => {
      rec('respond', id, accept, dest ?? null);
      if (seed.respondError) throw seed.respondError;
    },
    cancel: async (id) => {
      rec('cancel', id);
      return true;
    },
    announce: async () => rec('announce'),
    scan: async () => {
      rec('scan');
      return [];
    },
    addPeer: async (addr) => {
      rec('addPeer', addr);
      if (seed.addPeer) return seed.addPeer;
      throw `could not reach ${addr}`;
    },
    trustNewKey: async (fp) => rec('trustNewKey', fp),
    block: async (fp) => rec('block', fp),
    setVerified: async (fp, v) => rec('setVerified', fp, v),
    pickFiles: async () => seed.pickedFiles ?? [],
    pickFolder: async () => seed.pickedFolder ?? null,
    reveal: async (p) => rec('reveal', p),
    notify: (t, b) => rec('notify', t, b),
    onEvent: (cb) => {
      eventSubs.add(cb);
      return () => eventSubs.delete(cb);
    },
    onDrag: (cb) => {
      dragSubs.add(cb);
      return () => dragSubs.delete(cb);
    },
  };
  return {
    bridge,
    calls,
    emit: (e: CoreEvent) => eventSubs.forEach((cb) => cb(e)),
    drag: (d: DragState) => dragSubs.forEach((cb) => cb(d)),
  };
}

/** For `npm run dev:mock` and Playwright: seed via window.__senditSeed, drive via window.__sendit. */
export function installMockBridge(): Bridge {
  const w = window as unknown as { __senditSeed?: Seed; __sendit?: ReturnType<typeof createMockBridge> };
  const m = createMockBridge(w.__senditSeed ?? {});
  w.__sendit = m;
  return m.bridge;
}
