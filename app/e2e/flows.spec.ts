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
  // main.ts installs the mock bridge from a dynamic import, after 'load'.
  await page.waitForFunction(() => (window as any).__sendit);
}
const emit = (page: Page, e: object) => page.evaluate((ev) => (window as any).__sendit.emit(ev), e);
const drag = (page: Page, d: object) => page.evaluate((x) => (window as any).__sendit.drag(x), d);
const calls = (page: Page, name: string): Promise<Call[]> =>
  page.evaluate((n) => (window as any).__sendit.calls.filter((c: Call) => c.name === n), name);

test('empty state explains how devices appear', async ({ page }) => {
  await boot(page, {});
  await expect(page.getByRole('heading', { name: /Looking on this Wi-Fi/ })).toBeVisible();
});

test('flow A: accept an incoming file and reveal it', async ({ page }) => {
  await boot(page, { devices: [PIXEL] });
  await emit(page, { type: 'incomingRequest', request: { requestId: 'r1', from: PIXEL, files: [FILE], totalSize: FILE.size, fitsOnDisk: true, senderVerified: true } });
  await page.getByRole('button', { name: 'Accept' }).click();
  const [r] = await calls(page, 'respond');
  expect(r.args.slice(0, 2)).toEqual(['r1', true]);
  await emit(page, { type: 'transferStarted', transferId: 't1', requestId: 'r1', direction: 'incoming', peer: PIXEL.fingerprint, files: [FILE], total: FILE.size, sources: [] });
  await emit(page, { type: 'progress', transferId: 't1', direction: 'incoming', bytes: FILE.size / 2, total: FILE.size });
  await expect(page.getByRole('progressbar')).toHaveAttribute('aria-valuenow', '50');
  await emit(page, { type: 'transferDone', transferId: 't1', direction: 'incoming', paths: ['C:\\Users\\me\\Downloads\\sendit\\IMG_2044.jpg'] });
  await page.getByRole('button', { name: 'Show' }).click();
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

test('a text message from LocalSend shows as text', async ({ page }) => {
  await boot(page, { devices: [PIXEL] });
  await emit(page, { type: 'messageReceived', from: PIXEL, text: 'hello from my phone', senderVerified: true });
  await expect(page.getByText('hello from my phone')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Show' })).toHaveCount(0);
});

test('Ctrl+V pastes to the open conversation', async ({ page }) => {
  await boot(page, { devices: [PIXEL], pasted: { kind: 'files', count: 2, name: 'a.pdf' } });
  await page.getByRole('button', { name: /Pixel 8/ }).click();
  await page.keyboard.press('Control+V');
  await expect.poll(() => calls(page, 'paste')).toHaveLength(1);
  expect(await calls(page, 'paste')).toEqual([{ name: 'paste', args: [PIXEL.fingerprint] }]);
});

async function history(page: Page) {
  await boot(page, { devices: [PIXEL] });
  await page.getByRole('button', { name: /Pixel 8/ }).click();
  for (const [i, name] of ['a.jpg', 'b.jpg', 'c.pdf', 'd.jpg'].entries()) {
    const id = `t${i}`;
    const out = i % 2 === 0;
    await emit(page, { type: 'transferStarted', transferId: id, requestId: null, direction: out ? 'outgoing' : 'incoming', peer: PIXEL.fingerprint,
      files: [{ id, fileName: name, size: 1000, fileType: '' }], total: 1000, sources: out ? [`C:/${name}`] : [] });
    await emit(page, { type: 'transferDone', transferId: id, direction: out ? 'outgoing' : 'incoming', paths: out ? [] : [`C:/d/${name}`] });
  }
  await page.waitForTimeout(700); // let entry animations finish
}

test('the timeline never scrolls sideways', async ({ page }) => {
  await history(page);
  const overflow = await page.evaluate(() => {
    const el = document.querySelector('.list')!;
    return el.scrollWidth - el.clientWidth;
  });
  expect(overflow).toBe(0);
});

test('hovering the top thumbnail shows its preview fully, on top of everything', async ({ page }) => {
  await history(page);
  await page.locator('.row img.tile').first().hover();
  const peek = page.getByTestId('peek');
  await expect(peek).toBeVisible();
  const box = (await peek.boundingBox())!;
  const vp = page.viewportSize()!;
  expect(box.y).toBeGreaterThanOrEqual(0);
  expect(box.y + box.height).toBeLessThanOrEqual(vp.height);
  // The preview ignores the mouse (pointer-events: none); allow hits just for this check.
  const topmost = await page.evaluate(([x, y]) => {
    (document.querySelector('[data-testid=peek]') as HTMLElement).style.pointerEvents = 'auto';
    return document.elementFromPoint(x, y)?.getAttribute('data-testid');
  }, [box.x + 10, box.y + 10]);
  expect(topmost).toBe('peek');
});

test('right-click a history row: show, copy, remove', async ({ page }) => {
  await history(page);
  const row = page.locator('.row', { hasText: 'b.jpg' });
  await row.click({ button: 'right' });
  const menu = page.getByRole('menu');
  await expect(menu.getByRole('menuitem')).toHaveText(['Show in folder', 'Copy path', 'Remove from history']);
  await menu.getByRole('menuitem', { name: 'Show in folder' }).click();
  expect((await calls(page, 'reveal'))[0].args[0]).toContain('b.jpg');
  await expect(menu).toHaveCount(0);
  await row.click({ button: 'right' });
  await page.getByRole('menuitem', { name: 'Remove from history' }).click();
  await expect(page.locator('.row', { hasText: 'b.jpg' })).toHaveCount(0);
});

test('right-click a message row offers to copy the text', async ({ page }) => {
  await boot(page, { devices: [PIXEL] });
  await emit(page, { type: 'messageReceived', from: PIXEL, text: 'copy me', senderVerified: true });
  await page.locator('.row', { hasText: 'copy me' }).click({ button: 'right' });
  await expect(page.getByRole('menu').getByRole('menuitem')).toHaveText(['Copy text', 'Remove from history']);
  await page.keyboard.press('Escape');
  await expect(page.getByRole('menu')).toHaveCount(0);
});
