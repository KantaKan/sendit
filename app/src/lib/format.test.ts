import { expect, test } from 'vitest';
import { extLabel, formatBytes, lastTwo, percent, shortFp } from './format';

test('formatBytes', () => {
  expect(formatBytes(0)).toBe('0 B');
  expect(formatBytes(999)).toBe('999 B');
  expect(formatBytes(4_200_000)).toBe('4.2 MB');
  expect(formatBytes(318_000_000)).toBe('318 MB');
  expect(formatBytes(1_500_000_000)).toBe('1.5 GB');
});

test('percent never divides by zero and never exceeds 100', () => {
  expect(percent(0, 0)).toBe(100);
  expect(percent(64, 100)).toBe(64);
  expect(percent(150, 100)).toBe(100);
});

test('extLabel', () => {
  expect(extLabel('IMG_2031.jpg')).toBe('JPG');
  expect(extLabel('archive.tar.gz')).toBe('GZ');
  expect(extLabel('README')).toBe('FILE');
});

test('shortFp and lastTwo', () => {
  expect(shortFp('A91F3C07E2B4FFFF')).toBe('a91f 3c07 e2b4');
  expect(lastTwo('C:\\Users\\me\\Downloads\\sendit')).toBe('Downloads/sendit');
  expect(lastTwo('/home/me/Downloads/sendit/')).toBe('Downloads/sendit');
});

import { primaryIp } from './format';

test('primaryIp prefers home-network addresses over virtual and link-local ones', () => {
  expect(primaryIp(['172.28.0.1', '169.254.52.253', '192.168.1.101'])).toBe('192.168.1.101');
  expect(primaryIp(['172.20.1.1', '10.0.0.5'])).toBe('10.0.0.5');
  expect(primaryIp(['172.20.1.1'])).toBe('172.20.1.1');
  expect(primaryIp([])).toBeNull();
});
