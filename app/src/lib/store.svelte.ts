import type { Bridge } from './bridge';
import { clearFinished, restore, toSaved } from './history';
import {
  apply, initial, markAccepting, removeTicket, markDeclined, markExpired, markPending, type State, type Ticket,
} from './state';
import type { AppInfo, CoreEvent, Device, Settings } from './types';

export function createStore(bridge: Bridge) {
  let state = $state<State>(initial());
  let me = $state<AppInfo | null>(null);
  let selected = $state<string | null>(null);
  let searching = $state(false);
  let dragging = $state(false);
  let error = $state<string | null>(null);
  let notice = $state<string | null>(null);
  let settings = $state<Settings | null>(null);
  let page = $state<'chat' | 'settings'>('chat');
  let noticeTimer: ReturnType<typeof setTimeout> | undefined;
  const showNotice = (text: string) => {
    notice = text;
    clearTimeout(noticeTimer);
    noticeTimer = setTimeout(() => (notice = null), 2500);
  };
  // History is saved at most once a second, and only after the saved copy has
  // loaded, so a slow load can never be overwritten by a near-empty list.
  let historyLoaded = false;
  let saveTimer: ReturnType<typeof setTimeout> | undefined;
  let lastSaved = '';
  let saveFailed = false;
  function persist() {
    if (!historyLoaded || saveTimer) return;
    saveTimer = setTimeout(() => {
      saveTimer = undefined;
      const items = toSaved($state.snapshot(state.tickets) as Ticket[]);
      const json = JSON.stringify(items);
      if (json === lastSaved) return;
      lastSaved = json;
      bridge.saveHistory(items).catch((e) => {
        lastSaved = '';
        if (!saveFailed) {
          saveFailed = true;
          error = `Could not save history: ${e}`;
        }
      });
    }, 1000);
  }
  // ponytail: a quit within 1 s of the last change loses that change; flush on exit if it bites.
  const fail = (e: unknown) => {
    error = String(e);
  };
  const addDevice = (device: Device) => {
    state = apply(state, { type: 'deviceUpdated', device }, Date.now());
  };
  const refreshKnown = () => {
    void bridge.knownDevices().then((known) => {
      state = { ...state, known };
    });
  };

  function onEvent(e: CoreEvent) {
    state = apply(state, e, Date.now());
    persist();
    if (e.type === 'incomingRequest') {
      selected ??= e.request.from.fingerprint;
      if (!document.hasFocus()) {
        bridge.notify(`${e.request.from.alias} wants to send files`, `${e.request.files.length} file(s)`);
      }
    }
    if (e.type === 'messageReceived') {
      selected ??= e.from.fingerprint;
      if (!document.hasFocus()) bridge.notify(`${e.from.alias} sent a message`, e.text.slice(0, 120));
    }
    if (e.type === 'identityChanged' && selected === e.oldFingerprint) selected = e.device.fingerprint;
    if (e.type === 'deviceUpdated' || e.type === 'identityChanged') refreshKnown();
  }

  function send(paths: string[]) {
    if (!paths.length) return;
    if (!selected) {
      error = 'Pick a device first, then drop the files on it.';
      return;
    }
    bridge.sendFiles(selected, paths).catch(fail);
  }

  function accept(requestId: string, dest?: string) {
    state = markAccepting(state, requestId);
    bridge.respond(requestId, true, dest).catch((e) => {
      const msg = String(e);
      if (msg.includes('no pending request')) {
        state = markExpired(state, requestId);
      } else {
        state = markPending(state, requestId);
        error = msg;
      }
    });
  }

  return {
    get state() { return state; },
    get me() { return me; },
    get selected() { return selected; },
    get searching() { return searching; },
    get dragging() { return dragging; },
    get error() { return error; },
    /** short-lived hint shown in the conversation footer */
    get notice() { return notice; },
    get selectedDevice(): Device | null { return selected ? (state.devices[selected] ?? null) : null; },

    start(): () => void {
      const offEvent = bridge.onEvent(onEvent);
      const offError = bridge.onAppError((m) => {
        error = m;
      });
      const offDrag = bridge.onDrag((d) => {
        dragging = d.kind === 'enter';
        if (d.kind === 'drop') send(d.paths);
      });
      void (async () => {
        try {
          me = await bridge.info();
          settings = await bridge.settings();
          const [devices, known] = await Promise.all([bridge.devices(), bridge.knownDevices()]);
          for (const d of devices) addDevice(d);
          state = { ...state, known };
        } catch (e) {
          fail(e);
        }
      })();
      bridge
        .loadHistory()
        .then((saved) => {
          state = restore(state, saved);
          lastSaved = JSON.stringify(toSaved(saved));
          historyLoaded = true;
          persist();
        })
        // Never enable saving after a failed load: that would overwrite the file.
        .catch((e) => fail(`Could not load history: ${e}`));
      return () => {
        offEvent();
        offDrag();
        offError();
      };
    },
    get settings() { return settings; },
    get page() { return page; },
    openSettings() { page = 'settings'; },
    closeSettings() { page = 'chat'; },
    select(fp: string) { selected = fp; page = 'chat'; },
    /** Saves one change. Resolves to the message to show under that field, or null. */
    async updateSettings(change: Partial<Settings>): Promise<string | null> {
      if (!settings) return null;
      try {
        settings = await bridge.updateSettings({ ...settings, ...change });
        if (me) me = { ...me, alias: settings.alias, downloadDir: settings.downloadDir };
        return null;
      } catch (e) {
        return String(e);
      }
    },
    setAutoAccept(fp: string, on: boolean) {
      bridge.setAutoAccept(fp, on).then(refreshKnown).catch(fail);
    },
    autostart: () => bridge.autostart().catch(() => false),
    /** Resolves to the state the OS really has (so a refused change snaps back). */
    async setAutostart(on: boolean): Promise<boolean> {
      try {
        await bridge.setAutostart(on);
      } catch (e) {
        fail(e);
      }
      return bridge.autostart().catch(() => false);
    },
    pickFolder: () => bridge.pickFolder(),
    send,
    async paste() {
      if (!selected) {
        error = 'Pick a device first, then paste.';
        return;
      }
      try {
        // What was pasted shows up as a ticket or bubble; only "nothing" needs words.
        const pasted = await bridge.paste(selected);
        if (pasted.kind === 'empty') showNotice('Nothing to paste. Copy a file, image or text first.');
      } catch (e) {
        fail(e);
      }
    },
    async chooseFiles() { send(await bridge.pickFiles()); },
    accept,
    async acceptElsewhere(requestId: string) {
      const dir = await bridge.pickFolder();
      if (dir) accept(requestId, dir);
    },
    decline(requestId: string) {
      state = markDeclined(state, requestId);
      persist();
      bridge.respond(requestId, false).catch(() => {});
    },
    cancel(transferId: string) { bridge.cancel(transferId).catch(fail); },
    retry(t: Ticket) {
      if (t.text !== undefined) bridge.sendText(t.peer, t.text).catch(fail);
      else if (t.sources.length) bridge.sendFiles(t.peer, t.sources).catch(fail);
    },
    fileUrl: (path: string) => bridge.fileUrl(path),
    removeTicket(key: string) {
      state = removeTicket(state, key);
      persist();
    },
    reveal(path: string) { bridge.reveal(path).catch(fail); },
    refresh() {
      searching = true;
      bridge.announce().catch(fail);
      setTimeout(() => (searching = false), 3000);
    },
    async scan() {
      searching = true;
      try {
        for (const d of await bridge.scan()) addDevice(d);
      } catch (e) {
        fail(e);
      } finally {
        searching = false;
      }
    },
    async addPeer(addr: string) {
      const d = await bridge.addPeer(addr);
      addDevice(d);
      selected = d.fingerprint;
    },
    verify(fp: string) { bridge.setVerified(fp, true).catch(fail); },
    trustNewKey(fp: string) { bridge.trustNewKey(fp).catch(fail); },
    block(fp: string) { bridge.block(fp).catch(fail); },
    dismissError() { error = null; },
    clearHistory() {
      state = clearFinished(state);
      persist();
    },
  };
}

export type Store = ReturnType<typeof createStore>;

