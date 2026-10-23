import type { AppInfo, CoreEvent, Device, KnownDevice, Pasted } from './types';

export type DragState = { kind: 'enter' } | { kind: 'leave' } | { kind: 'drop'; paths: string[] };

/** Everything the UI may ask of the backend. Two implementations: Tauri and mock. */
export interface Bridge {
  info(): Promise<AppInfo>;
  devices(): Promise<Device[]>;
  knownDevices(): Promise<KnownDevice[]>;
  sendFiles(fingerprint: string, paths: string[]): Promise<void>;
  respond(requestId: string, accept: boolean, dest?: string): Promise<void>;
  cancel(transferId: string): Promise<boolean>;
  announce(): Promise<void>;
  scan(): Promise<Device[]>;
  addPeer(addr: string): Promise<Device>;
  trustNewKey(fingerprint: string): Promise<void>;
  block(fingerprint: string): Promise<void>;
  setVerified(fingerprint: string, verified: boolean): Promise<void>;
  pickFiles(): Promise<string[]>;
  pickFolder(): Promise<string | null>;
  reveal(path: string): Promise<void>;
  notify(title: string, body: string): void;
  /** Send whatever is on the OS clipboard to this device. */
  paste(fingerprint: string): Promise<Pasted>;
  /** Errors from work that finished after its command returned. */
  onAppError(cb: (message: string) => void): () => void;
  onEvent(cb: (e: CoreEvent) => void): () => void;
  onDrag(cb: (d: DragState) => void): () => void;
}
