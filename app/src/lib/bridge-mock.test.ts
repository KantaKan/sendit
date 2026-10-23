import { expect, test, vi } from 'vitest';
import { createMockBridge } from './bridge-mock';
import { device } from './fixtures';

test('mock delivers events and drags and records calls', async () => {
  const m = createMockBridge({ devices: [device()] });
  const onEvent = vi.fn();
  const onDrag = vi.fn();
  const off = m.bridge.onEvent(onEvent);
  m.bridge.onDrag(onDrag);
  m.emit({ type: 'deviceUpdated', device: device() });
  m.drag({ kind: 'drop', paths: ['a'] });
  off();
  m.emit({ type: 'deviceUpdated', device: device() });
  expect(onEvent).toHaveBeenCalledTimes(1);
  expect(onDrag).toHaveBeenCalledWith({ kind: 'drop', paths: ['a'] });
  await m.bridge.sendFiles('FP', ['a']);
  expect(m.calls).toEqual([{ name: 'sendFiles', args: ['FP', ['a']] }]);
  expect(await m.bridge.devices()).toHaveLength(1);
});

test('mock settings round-trip and history persists only when asked', async () => {
  const m = createMockBridge({ info: { alias: 'desk' } });
  expect(await m.bridge.settings()).toEqual({
    alias: 'desk', downloadDir: 'C:\\Users\\me\\Downloads\\sendit', pin: null, closeToTray: true,
  });
  const s = await m.bridge.updateSettings({ ...(await m.bridge.settings()), alias: ' studio ' });
  expect(s.alias).toBe('studio');
  expect(await m.bridge.loadHistory()).toEqual([]);
  await m.bridge.saveHistory([{ key: 'k' } as never]);
  expect(await m.bridge.loadHistory()).toEqual([{ key: 'k' }]);
  expect(localStorage.getItem('sendit-mock-history')).toBeNull();
  await expect(createMockBridge({ settingsError: 'no' }).bridge.updateSettings(s)).rejects.toBe('no');
});
