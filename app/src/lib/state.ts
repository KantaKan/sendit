import type { CoreEvent, Device, Direction, FailReason, FileMeta, KnownDevice } from './types';

export type TicketStatus =
  | 'pending' | 'accepting' | 'live' | 'done' | 'failed' | 'declined' | 'cancelled' | 'expired' | 'message' | 'sending';

export interface Ticket {
  key: string;
  transferId?: string;
  requestId?: string;
  peer: string;
  direction: Direction;
  files: FileMeta[];
  total: number;
  bytes: number;
  status: TicketStatus;
  reason?: FailReason;
  paths: string[];
  sources: string[];
  senderVerified?: boolean;
  fitsOnDisk?: boolean;
  /** text of a LocalSend text message */
  text?: string;
  /** id of an outgoing message (MessageSending/Sent/Failed) */
  messageId?: string;
  /** recent transfer speeds in bytes/s, newest last */
  speeds?: number[];
  /** time of the last progress event */
  lastAt?: number;
  /** when it finished (done, failed, declined…) */
  doneAt?: number;
  /** ms timestamp the ticket appeared */
  at: number;
}

export interface State {
  devices: Record<string, Device>;
  known: KnownDevice[];
  tickets: Ticket[];
  /** suspect fingerprint -> fingerprint it replaced */
  identityChanges: Record<string, string>;
}

export const initial = (): State => ({ devices: {}, known: [], tickets: [], identityChanges: {} });

function patch(s: State, pick: (t: Ticket) => boolean, change: (t: Ticket) => Partial<Ticket>): State {
  const i = s.tickets.findIndex(pick);
  if (i < 0) return s;
  const tickets = s.tickets.slice();
  tickets[i] = { ...tickets[i], ...change(tickets[i]) };
  return { ...s, tickets };
}

const byTransfer = (id: string) => (t: Ticket) => t.transferId === id;
const byRequest = (id: string) => (t: Ticket) => t.requestId === id;

let seq = 0;

export function apply(s: State, e: CoreEvent, now: number): State {
  switch (e.type) {
    case 'deviceUpdated': {
      const identityChanges = { ...s.identityChanges };
      if (e.device.trust !== 'suspect') delete identityChanges[e.device.fingerprint];
      return { ...s, devices: { ...s.devices, [e.device.fingerprint]: e.device }, identityChanges };
    }
    case 'identityChanged':
      return {
        ...s,
        devices: { ...s.devices, [e.device.fingerprint]: e.device },
        identityChanges: { ...s.identityChanges, [e.device.fingerprint]: e.oldFingerprint },
      };
    case 'incomingRequest': {
      const r = e.request;
      const devices = s.devices[r.from.fingerprint] ? s.devices : { ...s.devices, [r.from.fingerprint]: r.from };
      const ticket: Ticket = {
        key: `req:${r.requestId}`,
        requestId: r.requestId,
        peer: r.from.fingerprint,
        direction: 'incoming',
        files: r.files,
        total: r.totalSize,
        bytes: 0,
        status: 'pending',
        paths: [],
        sources: [],
        senderVerified: r.senderVerified,
        fitsOnDisk: r.fitsOnDisk,
        at: now,
      };
      return { ...s, devices, tickets: [...s.tickets, ticket] };
    }
    case 'transferStarted': {
      if (e.requestId && s.tickets.some(byRequest(e.requestId))) {
        return patch(s, byRequest(e.requestId), () => ({
          transferId: e.transferId, status: 'live', files: e.files, total: e.total, at: now,
        }));
      }
      const ticket: Ticket = {
        key: e.transferId,
        transferId: e.transferId,
        peer: e.peer,
        direction: e.direction,
        files: e.files,
        total: e.total,
        bytes: 0,
        status: 'live',
        paths: [],
        sources: e.sources,
        at: now,
      };
      return { ...s, tickets: [...s.tickets, ticket] };
    }
    case 'messageReceived': {
      const devices = s.devices[e.from.fingerprint] ? s.devices : { ...s.devices, [e.from.fingerprint]: e.from };
      const ticket: Ticket = {
        key: `msg:${now}:${seq++}`,
        peer: e.from.fingerprint,
        direction: 'incoming',
        files: [],
        total: 0,
        bytes: 0,
        status: 'message',
        paths: [],
        sources: [],
        senderVerified: e.senderVerified,
        text: e.text,
        at: now,
      };
      return { ...s, devices, tickets: [...s.tickets, ticket] };
    }
    case 'messageSending': {
      const ticket: Ticket = {
        key: `msg:${e.id}`,
        messageId: e.id,
        peer: e.peer,
        direction: 'outgoing',
        files: [],
        total: 0,
        bytes: 0,
        status: 'sending',
        paths: [],
        sources: [],
        text: e.text,
        at: now,
      };
      return { ...s, tickets: [...s.tickets, ticket] };
    }
    case 'messageSent':
      return patch(s, (t) => t.messageId === e.id, () => ({ status: 'message', at: now, doneAt: now }));
    case 'messageFailed':
      return patch(s, (t) => t.messageId === e.id, () => ({ status: 'failed', reason: e.reason, doneAt: now }));
    case 'requestExpired':
      return patch(
        s,
        (t) => t.requestId === e.requestId && (t.status === 'pending' || t.status === 'accepting'),
        () => ({ status: 'expired' }),
      );
    case 'progress':
      return patch(s, byTransfer(e.transferId), (t) => {
        const dt = (now - (t.lastAt ?? t.at)) / 1000;
        const speeds = dt > 0 ? [...(t.speeds ?? []), (e.bytes - t.bytes) / dt].slice(-30) : t.speeds;
        return { bytes: e.bytes, speeds, lastAt: now };
      });
    case 'transferDone':
      return patch(s, byTransfer(e.transferId), (t) => ({ status: 'done', bytes: t.total, paths: e.paths, doneAt: now }));
    case 'transferFailed': {
      const status: TicketStatus =
        e.reason.kind === 'cancelled' ? 'cancelled' : e.reason.kind === 'declined' ? 'declined' : 'failed';
      return patch(s, byTransfer(e.transferId), () => ({ status, reason: e.reason, doneAt: now }));
    }
  }
}

export const markAccepting = (s: State, requestId: string) => patch(s, byRequest(requestId), () => ({ status: 'accepting' }));
export const markPending = (s: State, requestId: string) => patch(s, byRequest(requestId), () => ({ status: 'pending' }));
export const markDeclined = (s: State, requestId: string) => patch(s, byRequest(requestId), () => ({ status: 'declined' }));
export const markExpired = (s: State, requestId: string) => patch(s, byRequest(requestId), () => ({ status: 'expired' }));

export const ticketsFor = (s: State, fp: string) => s.tickets.filter((t) => t.peer === fp);

const byAlias = (a: { alias: string }, b: { alias: string }) =>
  a.alias.localeCompare(b.alias, undefined, { sensitivity: 'base' });

export const nearby = (s: State) => Object.values(s.devices).sort(byAlias);

export const seenBefore = (s: State) => s.known.filter((k) => !s.devices[k.fingerprint]).sort(byAlias);

/** Requests waiting for an answer, what is moving now, and finished items (newest first). */
export function sections(ts: Ticket[]) {
  const requests = ts.filter((t) => t.status === 'pending' || t.status === 'accepting');
  const now = ts.filter((t) => t.status === 'live' || t.status === 'sending');
  const history = ts
    .filter((t) => !requests.includes(t) && !now.includes(t))
    .sort((a, b) => (b.doneAt ?? b.at) - (a.doneAt ?? a.at));
  return { requests, now, history };
}

/** Bytes of transfers that finished on the same calendar day as `now`. */
export function bytesToday(ts: Ticket[], now: number): number {
  const day = new Date(now).toDateString();
  return ts
    .filter((t) => t.status === 'done' && t.doneAt !== undefined && new Date(t.doneAt).toDateString() === day)
    .reduce((sum, t) => sum + t.total, 0);
}

/** Forget one item (the file itself is untouched). */
export const removeTicket = (s: State, key: string): State => ({ ...s, tickets: s.tickets.filter((t) => t.key !== key) });
