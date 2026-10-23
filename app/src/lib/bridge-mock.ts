import type { Bridge, DragState } from './bridge';
import type { Ticket } from './state';
import type { AppInfo, CoreEvent, Device, KnownDevice, Pasted, Settings } from './types';

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
  /** scan() resolves after this many ms (default 1500) */
  scanDelayMs?: number;
  /** paste() resolves with this */
  pasted?: Pasted;
  settings?: Partial<Settings>;
  /** updateSettings() rejects with this */
  settingsError?: string;
  /** setAutostart() rejects with this */
  autostartError?: string;
  /** what loadHistory() returns when nothing has been saved */
  history?: Ticket[];
  /** loadHistory() resolves after this many ms */
  historyDelayMs?: number;
  /** saveHistory() rejects with this */
  saveHistoryError?: string;
  /** keep saved history in localStorage, so it survives a reload (e2e) */
  persist?: boolean;
}

export interface Call {
  name: string;
  args: unknown[];
}

export function createMockBridge(seed: Seed = {}) {
  const eventSubs = new Set<(e: CoreEvent) => void>();
  const dragSubs = new Set<(d: DragState) => void>();
  const errorSubs = new Set<(m: string) => void>();
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
    addresses: ['192.168.1.101'],
    ...seed.info,
  };
  let settings: Settings = {
    alias: info.alias, downloadDir: info.downloadDir, pin: null, closeToTray: true, ...seed.settings,
  };
  let autostart = false;
  const KEY = 'sendit-mock-history';
  let savedHistory: Ticket[] | null = null;
  const readSaved = (): Ticket[] | null => {
    if (!seed.persist) return savedHistory;
    try {
      const raw = localStorage.getItem(KEY);
      return raw ? JSON.parse(raw) : null;
    } catch {
      return null;
    }
  };
  const bridge: Bridge = {
    info: async () => info,
    devices: async () => seed.devices ?? [],
    knownDevices: async () => seed.known ?? [],
    sendFiles: async (fp, paths) => rec('sendFiles', fp, paths),
    sendText: async (fp, text) => rec('sendText', fp, text),
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
      // A real subnet scan takes seconds; keep the busy state visible in dev:mock.
      await new Promise((r) => setTimeout(r, seed.scanDelayMs ?? 1500));
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
    // A real (tiny) image so thumbnails and previews render in dev:mock and e2e.
    fileUrl: (p) =>
      `data:image/svg+xml,${encodeURIComponent(`<svg xmlns='http://www.w3.org/2000/svg' width='440' height='300'><rect width='100%' height='100%' fill='#3d6b5a'/><text x='20' y='40' fill='white'>${p.split(/[\/]/).pop()}</text></svg>`)}`,
    notify: (t, b) => rec('notify', t, b),
    paste: async (fp) => {
      rec('paste', fp);
      return seed.pasted ?? { kind: 'empty' };
    },
settings: async () => settings,
    updateSettings: async (s) => {
      rec('updateSettings', s);
      if (seed.settingsError) throw seed.settingsError;
      settings = { ...s, alias: s.alias.trim() };
      return settings;
    },
    setAutoAccept: async (fp, on) => rec('setAutoAccept', fp, on),
    autostart: async () => autostart,
    setAutostart: async (on) => {
      rec('setAutostart', on);
      if (seed.autostartError) throw seed.autostartError;
      autostart = on;
    },
    loadHistory: async () => {
      if (seed.historyDelayMs) await new Promise((r) => setTimeout(r, seed.historyDelayMs));
      return readSaved() ?? seed.history ?? [];
    },
    saveHistory: async (items) => {
      rec('saveHistory', items);
      if (seed.saveHistoryError) throw seed.saveHistoryError;
      savedHistory = items;
      if (seed.persist) {
        try {
          localStorage.setItem(KEY, JSON.stringify(items));
        } catch {
          /* private mode: in-memory only */
        }
      }
    },
    onAppError: (cb) => {
      errorSubs.add(cb);
      return () => errorSubs.delete(cb);
    },
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
    appError: (m: string) => errorSubs.forEach((cb) => cb(m)),
  };
}

/** For `npm run dev:mock` and Playwright: seed via window.__senditSeed, drive via window.__sendit. */
export function installMockBridge(): Bridge {
  const w = window as unknown as { __senditSeed?: Seed; __sendit?: ReturnType<typeof createMockBridge> };
  const m = createMockBridge(w.__senditSeed ?? {});
  w.__sendit = m;
  return m.bridge;
}
