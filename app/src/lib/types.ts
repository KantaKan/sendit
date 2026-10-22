// Mirrors sendit-core's serde output (see events.rs event_json_matches_frontend_contract).
export type Trust = 'new' | 'known' | 'verified' | 'suspect' | 'blocked';
export type Direction = 'incoming' | 'outgoing';

export interface Device {
  fingerprint: string;
  alias: string;
  deviceModel: string | null;
  deviceType: string | null;
  addr: string;
  trust: Trust;
  isSendit: boolean;
}

export interface KnownDevice {
  fingerprint: string;
  alias: string;
  trust: Trust;
}

export interface FileMeta {
  id: string;
  fileName: string;
  size: number;
  fileType: string;
}

export type FailReason =
  | {
      kind:
        | 'declined'
        | 'cancelled'
        | 'peerGone'
        | 'busy'
        | 'pinRequired'
        | 'blocked'
        | 'checksum'
        | 'diskFull'
        | 'identityMismatch';
    }
  | { kind: 'protocol' | 'io'; detail: string };

export interface IncomingRequest {
  requestId: string;
  from: Device;
  files: FileMeta[];
  totalSize: number;
  fitsOnDisk: boolean;
  senderVerified: boolean;
}

export interface AppInfo {
  alias: string;
  fingerprint: string;
  port: number;
  portFallback: boolean;
  downloadDir: string;
}

export type CoreEvent =
  | { type: 'deviceUpdated'; device: Device }
  | { type: 'identityChanged'; alias: string; oldFingerprint: string; device: Device }
  | { type: 'incomingRequest'; request: IncomingRequest }
  | { type: 'requestExpired'; requestId: string }
  | { type: 'messageReceived'; from: Device; text: string; senderVerified: boolean }
  | {
      type: 'transferStarted';
      transferId: string;
      /** set when this transfer was accepted from an IncomingRequest */
      requestId: string | null;
      direction: Direction;
      peer: string;
      files: FileMeta[];
      total: number;
      sources: string[];
    }
  | { type: 'progress'; transferId: string; direction: Direction; bytes: number; total: number }
  | { type: 'transferDone'; transferId: string; direction: Direction; paths: string[] }
  | { type: 'transferFailed'; transferId: string; direction: Direction; reason: FailReason };
