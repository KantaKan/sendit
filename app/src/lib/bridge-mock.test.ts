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
