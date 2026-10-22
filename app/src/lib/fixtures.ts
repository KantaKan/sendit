import type { Device, FileMeta } from './types';

export function device(over: Partial<Device> = {}): Device {
  return {
    fingerprint: 'A91F'.padEnd(64, '1'),
    alias: 'Pixel 8',
    deviceModel: 'Samsung',
    deviceType: 'mobile',
    addr: '192.168.1.24:53317',
    trust: 'known',
    isSendit: false,
    ...over,
  };
}

export function file(fileName: string, size: number, id = fileName): FileMeta {
  return { id, fileName, size, fileType: 'application/octet-stream' };
}
