import { describe, expect, test } from 'vitest';
import { device, file } from './fixtures';
import {
  apply, initial, markAccepting, markDeclined, markExpired, markPending, nearby, seenBefore, ticketsFor, type Ticket,
} from './state';
import type { CoreEvent } from './types';

const PIXEL = device();
const request = (id = 'r1', from = PIXEL): CoreEvent => ({
  type: 'incomingRequest',
  request: { requestId: id, from, files: [file('a.jpg', 10)], totalSize: 10, fitsOnDisk: true, senderVerified: false },
});
const started = (
  dir: 'incoming' | 'outgoing', id = 't1', sources: string[] = [], requestId: string | null = null,
): CoreEvent => ({
  type: 'transferStarted', transferId: id, requestId, direction: dir, peer: PIXEL.fingerprint, files: [file('a.jpg', 10)], total: 10, sources,
});

describe('apply', () => {
  test('deviceUpdated adds and replaces devices', () => {
    let s = apply(initial(), { type: 'deviceUpdated', device: PIXEL }, 1);
    s = apply(s, { type: 'deviceUpdated', device: { ...PIXEL, alias: 'Pixel 9' } }, 2);
    expect(nearby(s).map((d) => d.alias)).toEqual(['Pixel 9']);
  });

  test('incoming request from an unknown sender creates device and pending ticket', () => {
    const s = apply(initial(), request(), 5);
    expect(s.devices[PIXEL.fingerprint]).toEqual(PIXEL);
    const [t] = ticketsFor(s, PIXEL.fingerprint);
    expect(t).toMatchObject({ status: 'pending', requestId: 'r1', direction: 'incoming', total: 10, at: 5, senderVerified: false });
  });

  test('accepted request becomes live when its transfer starts', () => {
    let s = apply(initial(), request(), 1);
    s = markAccepting(s, 'r1');
    expect(ticketsFor(s, PIXEL.fingerprint)[0].status).toBe('accepting');
    s = apply(s, started('incoming', 't1', [], 'r1'), 2);
    const tickets = ticketsFor(s, PIXEL.fingerprint);
    expect(tickets).toHaveLength(1);
    expect(tickets[0]).toMatchObject({ status: 'live', transferId: 't1', at: 2 });
  });

  test('transfers match their own request even with two from the same peer', () => {
    let s = apply(initial(), request('r1'), 1);
    s = apply(s, request('r2'), 1);
    s = markAccepting(s, 'r1');
    s = markAccepting(s, 'r2');
    s = apply(s, started('incoming', 't2', [], 'r2'), 2);
    const [a, b] = ticketsFor(s, PIXEL.fingerprint);
    expect(a).toMatchObject({ requestId: 'r1', status: 'accepting' });
    expect(b).toMatchObject({ requestId: 'r2', status: 'live', transferId: 't2' });
  });

  test('requestExpired expires an unanswered request but leaves started ones alone', () => {
    let s = apply(initial(), request('r1'), 1);
    s = apply(s, request('r2'), 1);
    s = markAccepting(s, 'r2');
    s = apply(s, started('incoming', 't2', [], 'r2'), 2);
    s = apply(s, { type: 'requestExpired', requestId: 'r1' }, 3);
    s = apply(s, { type: 'requestExpired', requestId: 'r2' }, 3);
    expect(ticketsFor(s, PIXEL.fingerprint).map((t) => t.status)).toEqual(['expired', 'live']);
  });

  test('auto-accepted incoming transfer without a request still gets a ticket', () => {
    const s = apply(initial(), started('incoming'), 1);
    expect(ticketsFor(s, PIXEL.fingerprint)[0]).toMatchObject({ status: 'live', direction: 'incoming' });
  });

  test('outgoing transfer keeps its sources for retry', () => {
    const s = apply(initial(), started('outgoing', 't9', ['C:/a.jpg']), 1);
    expect(ticketsFor(s, PIXEL.fingerprint)[0]).toMatchObject({ status: 'live', sources: ['C:/a.jpg'] });
  });

  test('progress, done and failure update the right ticket', () => {
    let s = apply(initial(), started('outgoing', 't1'), 1);
    s = apply(s, started('outgoing', 't2'), 1);
    s = apply(s, { type: 'progress', transferId: 't1', direction: 'outgoing', bytes: 4, total: 10 }, 2);
    expect(ticketsFor(s, PIXEL.fingerprint).map((t) => t.bytes)).toEqual([4, 0]);
    s = apply(s, { type: 'transferDone', transferId: 't1', direction: 'outgoing', paths: ['C:/a.jpg'] }, 3);
    s = apply(s, { type: 'transferFailed', transferId: 't2', direction: 'outgoing', reason: { kind: 'peerGone' } }, 3);
    const [t1, t2] = ticketsFor(s, PIXEL.fingerprint);
    expect(t1).toMatchObject({ status: 'done', bytes: 10, paths: ['C:/a.jpg'] });
    expect(t2).toMatchObject({ status: 'failed', reason: { kind: 'peerGone' } });
  });

  test('declined and cancelled failures get their own status', () => {
    let s = apply(initial(), started('outgoing', 't1'), 1);
    s = apply(s, started('outgoing', 't2'), 1);
    s = apply(s, { type: 'transferFailed', transferId: 't1', direction: 'outgoing', reason: { kind: 'declined' } }, 2);
    s = apply(s, { type: 'transferFailed', transferId: 't2', direction: 'outgoing', reason: { kind: 'cancelled' } }, 2);
    expect(ticketsFor(s, PIXEL.fingerprint).map((t) => t.status)).toEqual(['declined', 'cancelled']);
  });

  test('identity change records the old key until the device is trusted again', () => {
    const suspect = { ...PIXEL, fingerprint: 'DEAD'.padEnd(64, '3'), trust: 'suspect' as const };
    let s = apply(initial(), { type: 'identityChanged', alias: 'Pixel 8', oldFingerprint: PIXEL.fingerprint, device: suspect }, 1);
    expect(s.identityChanges[suspect.fingerprint]).toBe(PIXEL.fingerprint);
    s = apply(s, { type: 'deviceUpdated', device: { ...suspect, trust: 'known' } }, 2);
    expect(s.identityChanges[suspect.fingerprint]).toBeUndefined();
  });

  test('events for unknown transfers are ignored', () => {
    const s = initial();
    expect(apply(s, { type: 'progress', transferId: 'nope', direction: 'incoming', bytes: 1, total: 2 }, 1)).toEqual(s);
  });
});

describe('local marks', () => {
  test('declined, expired and back-to-pending', () => {
    let s = apply(initial(), request('r1'), 1);
    s = apply(s, request('r2'), 1);
    s = markDeclined(s, 'r1');
    s = markAccepting(s, 'r2');
    s = markPending(s, 'r2');
    expect(ticketsFor(s, PIXEL.fingerprint).map((t) => t.status)).toEqual(['declined', 'pending']);
    s = markExpired(s, 'r2');
    expect(ticketsFor(s, PIXEL.fingerprint)[1].status).toBe('expired');
  });
});

describe('selectors', () => {
  test('seenBefore lists known devices that are not nearby', () => {
    let s = apply(initial(), { type: 'deviceUpdated', device: PIXEL }, 1);
    s = { ...s, known: [{ fingerprint: PIXEL.fingerprint, alias: 'Pixel 8', trust: 'known', autoAccept: false }, { fingerprint: 'B'.repeat(64), alias: 'Lab PC', trust: 'known', autoAccept: false }] };
    expect(seenBefore(s).map((k) => k.alias)).toEqual(['Lab PC']);
  });

  test('nearby sorts by alias, case-insensitive', () => {
    let s = apply(initial(), { type: 'deviceUpdated', device: device({ fingerprint: '1', alias: 'zed' }) }, 1);
    s = apply(s, { type: 'deviceUpdated', device: device({ fingerprint: '2', alias: 'Amy' }) }, 1);
    expect(nearby(s).map((d) => d.alias)).toEqual(['Amy', 'zed']);
  });
});

describe('messages', () => {
  test('a text message becomes a message ticket and adds an unknown sender', () => {
    const s = apply(initial(), { type: 'messageReceived', from: PIXEL, text: 'hi there', senderVerified: false }, 7);
    expect(s.devices[PIXEL.fingerprint]).toEqual(PIXEL);
    expect(ticketsFor(s, PIXEL.fingerprint)[0]).toMatchObject({
      status: 'message', text: 'hi there', direction: 'incoming', senderVerified: false, at: 7,
    });
  });
});

test('an outgoing message shows at once as sending, then sent', () => {
  let s = apply(initial(), { type: 'messageSending', id: 'm1', peer: PIXEL.fingerprint, text: 'yo' }, 9);
  expect(ticketsFor(s, PIXEL.fingerprint)[0]).toMatchObject({ status: 'sending', direction: 'outgoing', text: 'yo', at: 9 });
  s = apply(s, { type: 'messageSent', id: 'm1' }, 10);
  expect(ticketsFor(s, PIXEL.fingerprint)[0]).toMatchObject({ status: 'message', text: 'yo' });
});

test('a failed message keeps its text and the reason', () => {
  let s = apply(initial(), { type: 'messageSending', id: 'm1', peer: PIXEL.fingerprint, text: 'yo' }, 9);
  s = apply(s, { type: 'messageFailed', id: 'm1', reason: { kind: 'peerGone' } }, 10);
  expect(ticketsFor(s, PIXEL.fingerprint)[0]).toMatchObject({ status: 'failed', text: 'yo', reason: { kind: 'peerGone' } });
});

import { bytesToday, sections } from './state';

describe('timeline', () => {
  test('progress records speed samples (bytes/s) and done records when', () => {
    let s = apply(initial(), started('outgoing', 't1'), 0);
    s = apply(s, { type: 'progress', transferId: 't1', direction: 'outgoing', bytes: 4, total: 10 }, 1000);
    s = apply(s, { type: 'progress', transferId: 't1', direction: 'outgoing', bytes: 10, total: 10 }, 2000);
    expect(ticketsFor(s, PIXEL.fingerprint)[0].speeds).toEqual([4, 6]);
    s = apply(s, { type: 'transferDone', transferId: 't1', direction: 'outgoing', paths: [] }, 2500);
    expect(ticketsFor(s, PIXEL.fingerprint)[0].doneAt).toBe(2500);
  });

  test('sections: requests, then what is moving now, then history newest first', () => {
    let s = apply(initial(), started('outgoing', 'old'), 1);
    s = apply(s, { type: 'transferDone', transferId: 'old', direction: 'outgoing', paths: [] }, 2);
    s = apply(s, started('outgoing', 'new'), 3);
    s = apply(s, { type: 'transferDone', transferId: 'new', direction: 'outgoing', paths: [] }, 4);
    s = apply(s, started('incoming', 'live'), 5);
    s = apply(s, { type: 'messageSending', id: 'm', peer: PIXEL.fingerprint, text: 'x' }, 6);
    s = apply(s, request('r1'), 7);
    const { requests, now, history } = sections(ticketsFor(s, PIXEL.fingerprint));
    expect(requests.map((t) => t.requestId)).toEqual(['r1']);
    expect(now.map((t) => t.transferId ?? t.messageId)).toEqual(['live', 'm']);
    expect(history.map((t) => t.transferId)).toEqual(['new', 'old']);
  });

  test('bytesToday sums finished transfers from today only', () => {
    const today = new Date(2026, 9, 23, 14, 0).getTime();
    const yesterday = new Date(2026, 9, 22, 14, 0).getTime();
    let s = apply(initial(), started('outgoing', 'a'), today);
    s = apply(s, { type: 'transferDone', transferId: 'a', direction: 'outgoing', paths: [] }, today);
    s = apply(s, started('outgoing', 'b'), yesterday);
    s = apply(s, { type: 'transferDone', transferId: 'b', direction: 'outgoing', paths: [] }, yesterday);
    expect(bytesToday(s.tickets, today)).toBe(10);
  });
});

import { removeTicket } from './state';

test('removeTicket drops one item from the history', () => {
  let s = apply(initial(), started('outgoing', 'a'), 1);
  s = apply(s, started('outgoing', 'b'), 1);
  s = removeTicket(s, 'a');
  expect(ticketsFor(s, PIXEL.fingerprint).map((t) => t.transferId)).toEqual(['b']);
});

test('message keys stay unique next to restored items', () => {
  let s = { ...initial(), tickets: [] as Ticket[] };
  s = apply(s, { type: 'messageReceived', from: device(), text: 'a', senderVerified: true }, 1000);
  // a restored item from an earlier run that used the old key scheme
  s = { ...s, tickets: [{ ...s.tickets[0], key: 'msg:1' }, ...s.tickets] };
  s = apply(s, { type: 'messageReceived', from: device(), text: 'b', senderVerified: true }, 1000);
  s = removeTicket(s, s.tickets[0].key);
  s = apply(s, { type: 'messageReceived', from: device(), text: 'c', senderVerified: true }, 1000);
  const keys = s.tickets.map((x) => x.key);
  expect(new Set(keys).size).toBe(keys.length);
});
