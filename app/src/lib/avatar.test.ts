import { expect, test } from 'vitest';
import { avatarUri } from './avatar';

test('avatar is deterministic, case-insensitive and key-dependent', () => {
  const a = 'a91f3c07e2b4'.padEnd(64, '1');
  expect(avatarUri(a)).toMatch(/^data:image\/svg\+xml/);
  expect(avatarUri(a)).toBe(avatarUri(a.toUpperCase()));
  expect(avatarUri(a)).not.toBe(avatarUri('7d10b6e904fa'.padEnd(64, '1')));
});
