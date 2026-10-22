import { createAvatar } from '@dicebear/core';
import { thumbs } from '@dicebear/collection';

const cache = new Map<string, string>();

/** Same key, same face. The seed is part of the fingerprint, so a changed key changes the face. */
export function avatarUri(fingerprint: string): string {
  const seed = fingerprint.toUpperCase().slice(0, 16);
  let uri = cache.get(seed);
  if (!uri) {
    uri = createAvatar(thumbs, { seed, radius: 22 }).toDataUri();
    cache.set(seed, uri);
  }
  return uri;
}
