import type { Bridge } from './bridge';
import {
  apply, initial, markAccepting, markDeclined, markExpired, markPending, type State, type Ticket,
} from './state';
import type { AppInfo, CoreEvent, Device } from './types';

export function createStore(bridge: Bridge) {
  let state = $state<State>(initial());
  let me = $state<AppInfo | null>(null);
  let selected = $state<string | null>(null);
  let searching = $state(false);
  let dragging = $state(false);
  let error = $state<string | null>(null);
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
    if (e.type === 'incomingRequest') {
      selected ??= e.request.from.fingerprint;
      if (!document.hasFocus()) {
        bridge.notify(`${e.request.from.alias} wants to send files`, `${e.request.files.length} file(s)`);
      }
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
    get selectedDevice(): Device | null { return selected ? (state.devices[selected] ?? null) : null; },

    start(): () => void {
      const offEvent = bridge.onEvent(onEvent);
      const offDrag = bridge.onDrag((d) => {
        dragging = d.kind === 'enter';
        if (d.kind === 'drop') send(d.paths);
      });
      void (async () => {
        try {
          me = await bridge.info();
          const [devices, known] = await Promise.all([bridge.devices(), bridge.knownDevices()]);
          for (const d of devices) addDevice(d);
          state = { ...state, known };
        } catch (e) {
          fail(e);
        }
      })();
      return () => {
        offEvent();
        offDrag();
      };
    },
    select(fp: string) { selected = fp; },
    send,
    async chooseFiles() { send(await bridge.pickFiles()); },
    accept,
    async acceptElsewhere(requestId: string) {
      const dir = await bridge.pickFolder();
      if (dir) accept(requestId, dir);
    },
    decline(requestId: string) {
      state = markDeclined(state, requestId);
      bridge.respond(requestId, false).catch(() => {});
    },
    cancel(transferId: string) { bridge.cancel(transferId).catch(fail); },
    retry(t: Ticket) {
      if (t.sources.length) bridge.sendFiles(t.peer, t.sources).catch(fail);
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
  };
}

export type Store = ReturnType<typeof createStore>;
