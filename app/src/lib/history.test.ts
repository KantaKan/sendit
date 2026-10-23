import { expect, test } from 'vitest';
import { clearFinished, restore, toSaved } from './history';
import { initial, type Ticket } from './state';

const t = (key: string, status: Ticket['status'], at: number, extra: Partial<Ticket> = {}): Ticket => ({
  key, peer: 'P', direction: 'incoming', files: [], total: 1, bytes: 1, status, paths: [], sources: [], at, ...extra,
});

test('toSaved keeps finished items, oldest first, without live-only fields', () => {
  const saved = toSaved([
    t('b', 'done', 1, { doneAt: 50, speeds: [1, 2], lastAt: 40 }),
    t('live', 'live', 2),
    t('req', 'pending', 3),
    t('send', 'sending', 4),
    t('a', 'failed', 5, { doneAt: 10 }),
    t('m', 'message', 30),
  ]);
  expect(saved.map((x) => x.key)).toEqual(['a', 'm', 'b']);
  expect(saved[2]).not.toHaveProperty('speeds');
  expect(saved[2]).not.toHaveProperty('lastAt');
});

test('restore puts saved items first and skips keys already present', () => {
  const s = { ...initial(), tickets: [t('new', 'live', 9)] };
  const r = restore(s, [t('old', 'done', 1), t('new', 'done', 1)]);
  expect(r.tickets.map((x) => x.key)).toEqual(['old', 'new']);
  expect(r.tickets[1].status).toBe('live');
});

test('clearFinished keeps requests and live transfers', () => {
  const s = { ...initial(), tickets: [t('a', 'done', 1), t('b', 'pending', 2), t('c', 'live', 3), t('d', 'expired', 4)] };
  expect(clearFinished(s).tickets.map((x) => x.key)).toEqual(['b', 'c']);
});
