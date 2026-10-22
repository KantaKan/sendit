import { expect, test, type Page } from '@playwright/test';

const PIXEL = { fingerprint: 'A91F'.padEnd(64, '1'), alias: 'Pixel 8', deviceModel: 'Samsung', deviceType: 'mobile', addr: '192.168.1.24:53317', trust: 'known', isSendit: false };
const MAC = { ...PIXEL, fingerprint: '77C2'.padEnd(64, '2'), alias: "Nont's MacBook", deviceModel: 'macos', isSendit: true };
const FILE = { id: '1', fileName: 'IMG_2044.jpg', size: 3_900_000, fileType: 'image/jpeg' };

type Call = { name: string; args: unknown[] };

async function boot(page: Page, seed: object) {
  await page.addInitScript((s) => {
    (window as unknown as { __senditSeed: object }).__senditSeed = s;
  }, seed);
  await page.goto('/');
}
const emit = (page: Page, e: object) => page.evaluate((ev) => (window as any).__sendit.emit(ev), e);
const drag = (page: Page, d: object) => page.evaluate((x) => (window as any).__sendit.drag(x), d);
const calls = (page: Page, name: string): Promise<Call[]> =>
  page.evaluate((n) => (window as any).__sendit.calls.filter((c: Call) => c.name === n), name);

test('empty state explains how devices appear', async ({ page }) => {
  await boot(page, {});
  await expect(page.getByText(/Devices on the same Wi-Fi show up here/)).toBeVisible();
});

test('flow A: accept an incoming file and reveal it', async ({ page }) => {
  await boot(page, { devices: [PIXEL] });
  await emit(page, { type: 'incomingRequest', request: { requestId: 'r1', from: PIXEL, files: [FILE], totalSize: FILE.size, fitsOnDisk: true, senderVerified: true } });
  await page.getByRole('button', { name: 'Accept' }).click();
  const [r] = await calls(page, 'respond');
  expect(r.args.slice(0, 2)).toEqual(['r1', true]);
  await emit(page, { type: 'transferStarted', transferId: 't1', requestId: 'r1', direction: 'incoming', peer: PIXEL.fingerprint, files: [FILE], total: FILE.size, sources: [] });
  await emit(page, { type: 'progress', transferId: 't1', direction: 'incoming', bytes: FILE.size / 2, total: FILE.size });
  await expect(page.getByText('50%')).toBeVisible();
  await emit(page, { type: 'transferDone', transferId: 't1', direction: 'incoming', paths: ['C:\\Users\\me\\Downloads\\sendit\\IMG_2044.jpg'] });
  await page.getByRole('button', { name: 'Show in folder' }).click();
  expect((await calls(page, 'reveal'))[0].args[0]).toContain('IMG_2044.jpg');
});

test('flow B: verify a sendit device by comparing faces', async ({ page }) => {
  await boot(page, { devices: [MAC] });
  await page.getByRole('button', { name: /Nont's MacBook/ }).click();
  await expect(page.getByText('Same face on both screens?')).toBeVisible();
  await page.getByRole('button', { name: 'Yes, trust' }).click();
  expect(await calls(page, 'setVerified')).toEqual([{ name: 'setVerified', args: [MAC.fingerprint, true] }]);
});

test('flow C: a changed key is blocked from the warning', async ({ page }) => {
  await boot(page, { devices: [PIXEL] });
  await page.getByRole('button', { name: /Pixel 8/ }).click();
  const suspect = { ...PIXEL, fingerprint: 'DEAD'.padEnd(64, '3'), trust: 'suspect' };
  await emit(page, { type: 'identityChanged', alias: 'Pixel 8', oldFingerprint: PIXEL.fingerprint, device: suspect });
  await expect(page.getByRole('alert')).toContainText("This isn't the Pixel 8 you know");
  await page.getByRole('button', { name: 'Block' }).click();
  expect(await calls(page, 'block')).toEqual([{ name: 'block', args: [suspect.fingerprint] }]);
});

test('send by drop, fail, retry with the same files', async ({ page }) => {
  await boot(page, { devices: [PIXEL] });
  await page.getByRole('button', { name: /Pixel 8/ }).click();
  await drag(page, { kind: 'enter' });
  await expect(page.getByText('Drop to send to Pixel 8')).toBeVisible();
  await drag(page, { kind: 'drop', paths: ['C:\\a.txt'] });
  expect(await calls(page, 'sendFiles')).toEqual([{ name: 'sendFiles', args: [PIXEL.fingerprint, ['C:\\a.txt']] }]);
  const f = { id: '1', fileName: 'a.txt', size: 3, fileType: 'text/plain' };
  await emit(page, { type: 'transferStarted', transferId: 'o1', requestId: null, direction: 'outgoing', peer: PIXEL.fingerprint, files: [f], total: 3, sources: ['C:\\a.txt'] });
  await emit(page, { type: 'transferFailed', transferId: 'o1', direction: 'outgoing', reason: { kind: 'peerGone' } });
  await expect(page.getByText('Connection lost')).toBeVisible();
  await page.getByRole('button', { name: 'Retry' }).click();
  expect(await calls(page, 'sendFiles')).toHaveLength(2);
});

test('dropping with no device selected explains what to do', async ({ page }) => {
  await boot(page, { devices: [PIXEL] });
  await drag(page, { kind: 'drop', paths: ['C:\\a.txt'] });
  await expect(page.getByRole('alert')).toContainText('Pick a device first');
  expect(await calls(page, 'sendFiles')).toEqual([]);
});
