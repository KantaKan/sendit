import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { disable, enable, isEnabled } from '@tauri-apps/plugin-autostart';
import { open } from '@tauri-apps/plugin-dialog';
import { isPermissionGranted, requestPermission, sendNotification } from '@tauri-apps/plugin-notification';
import { revealItemInDir } from '@tauri-apps/plugin-opener';
import type { Bridge, DragState } from './bridge';
import type { Ticket } from './state';
import type { AppInfo, CoreEvent, Device, KnownDevice, Pasted, Settings } from './types';

/** Tauri's listeners are async to attach; give the UI a sync unsubscribe. */
function subscribe<T>(attach: (cb: T) => Promise<() => void>, cb: T): () => void {
  let off: (() => void) | undefined;
  let dead = false;
  void attach(cb).then((u) => (dead ? u() : (off = u)));
  return () => {
    dead = true;
    off?.();
  };
}

export const tauriBridge: Bridge = {
  info: () => invoke<AppInfo>('info'),
  devices: () => invoke<Device[]>('devices'),
  knownDevices: () => invoke<KnownDevice[]>('known_devices'),
  sendFiles: (fingerprint, paths) => invoke('send_files', { fingerprint, paths }),
  sendText: (fingerprint, text) => invoke('send_text', { fingerprint, text }),
  respond: (requestId, accept, dest) => invoke('respond', { requestId, accept, dest: dest ?? null }),
  cancel: (transferId) => invoke<boolean>('cancel', { transferId }),
  announce: () => invoke('announce'),
  scan: () => invoke<Device[]>('scan'),
  addPeer: (addr) => invoke<Device>('add_peer', { addr }),
  trustNewKey: (fingerprint) => invoke('trust_new_key', { fingerprint }),
  block: (fingerprint) => invoke('block', { fingerprint }),
  setVerified: (fingerprint, verified) => invoke('set_verified', { fingerprint, verified }),
  pickFiles: async () => {
    const r = await open({ multiple: true, directory: false });
    return r === null ? [] : Array.isArray(r) ? r : [r];
  },
  pickFolder: async () => {
    const r = await open({ multiple: false, directory: true });
    return typeof r === 'string' ? r : null;
  },
  reveal: (path) => revealItemInDir(path),
  fileUrl: (path) => convertFileSrc(path),
  notify: (title, body) => {
    void (async () => {
      let ok = await isPermissionGranted();
      if (!ok) ok = (await requestPermission()) === 'granted';
      if (ok) sendNotification({ title, body });
    })();
  },
settings: () => invoke<Settings>('get_settings'),
  updateSettings: (settings) => invoke<Settings>('update_settings', { settings }),
  setAutoAccept: (fingerprint, on) => invoke('set_auto_accept', { fingerprint, on }),
  autostart: () => isEnabled(),
  setAutostart: (on) => (on ? enable() : disable()),
  loadHistory: () => invoke<Ticket[]>('load_history'),
  saveHistory: (items) => invoke('save_history', { items }),
  paste: (fingerprint) => invoke<Pasted>('paste', { fingerprint }),
  onAppError: (cb) => subscribe((f: typeof cb) => listen<string>('app-error', (e) => f(e.payload)), cb),
  onEvent: (cb) => subscribe((f: typeof cb) => listen<CoreEvent>('core-event', (e) => f(e.payload)), cb),
  onDrag: (cb) =>
    subscribe(
      (f: (d: DragState) => void) =>
        getCurrentWebview().onDragDropEvent((e) => {
          const p = e.payload;
          if (p.type === 'enter') f({ kind: 'enter' });
          else if (p.type === 'leave') f({ kind: 'leave' });
          else if (p.type === 'drop') f({ kind: 'drop', paths: p.paths });
        }),
      cb,
    ),
};
