import { expect, test } from 'vitest';
import { reasonText } from './reasons';

test('every reason has human text without dashes', () => {
  const kinds = ['declined', 'cancelled', 'peerGone', 'busy', 'pinRequired', 'blocked', 'checksum', 'diskFull', 'identityMismatch'] as const;
  for (const kind of kinds) {
    const t = reasonText({ kind });
    expect(t.length).toBeGreaterThan(3);
    expect(t).not.toMatch(/[\u2013\u2014]/);
  }
  expect(reasonText({ kind: 'protocol', detail: 'boom' })).toContain('boom');
  expect(reasonText({ kind: 'peerGone' })).toBe('Connection lost');
});
