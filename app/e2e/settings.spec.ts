import { expect, test, type Page } from '@playwright/test';

const PIXEL = { fingerprint: 'A91F'.padEnd(64, '1'), alias: 'Pixel 8', deviceModel: 'Samsung', deviceType: 'mobile', addr: '192.168.1.24:53317', trust: 'known', isSendit: false };
const FILE = { id: '1', fileName: 'IMG_2044.jpg', size: 3_900_000, fileType: 'image/jpeg' };
const DONE = {
  key: 't0', transferId: 't0', peer: PIXEL.fingerprint, direction: 'incoming', files: [FILE], total: FILE.size, bytes: FILE.size,
  status: 'done', paths: ['C:\\Users\\me\\Downloads\\sendit\\IMG_2044.jpg'], sources: [], at: 1_700_000_000_000, doneAt: 1_700_000_000_000,
};
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
const calls = (page: Page, name: string): Promise<Call[]> =>
  page.evaluate((n) => (window as any).__sendit.calls.filter((c: Call) => c.name === n), name);
const openSettings = (page: Page) => page.getByRole('button', { name: 'Settings', exact: true }).click();

test('rename this device', async ({ page }) => {
  await boot(page, { devices: [PIXEL] });
  await openSettings(page);
  await expect(page.getByRole('heading', { name: 'Settings' })).toBeVisible();
  const name = page.getByLabel('Device name');
  await name.fill('Studio PC');
  await name.press('Enter');
  await expect(page.getByRole('navigation').getByText('Studio PC')).toBeVisible();
  expect((await calls(page, 'updateSettings')).at(-1)!.args[0]).toMatchObject({ alias: 'Studio PC' });
});

test('require a PIN', async ({ page }) => {
  await boot(page, { devices: [PIXEL] });
  await openSettings(page);
  await page.getByRole('switch', { name: 'Require a PIN' }).click();
  await page.getByLabel('PIN', { exact: true }).fill('4321');
  await page.getByLabel('PIN', { exact: true }).press('Enter');
  await expect(page.getByRole('switch', { name: 'Require a PIN' })).toHaveAttribute('aria-checked', 'true');
  expect((await calls(page, 'updateSettings')).at(-1)!.args[0]).toMatchObject({ pin: '4321' });
});

test('clear history empties the timeline', async ({ page }) => {
  await boot(page, { devices: [PIXEL], history: [DONE] });
  await page.getByRole('button', { name: /Pixel 8/ }).click();
  await expect(page.getByText('IMG_2044.jpg')).toBeVisible();
  await openSettings(page);
  await page.getByRole('button', { name: 'Clear history' }).click();
  await page.getByRole('button', { name: 'Yes, clear' }).click();
  await page.getByRole('button', { name: /Pixel 8/ }).click();
  await expect(page.getByText('IMG_2044.jpg')).toHaveCount(0);
  await expect.poll(async () => (await calls(page, 'saveHistory')).at(-1)?.args[0]).toEqual([]);
});

test('a finished transfer is still there after a reload', async ({ page }) => {
  await boot(page, { devices: [PIXEL], persist: true });
  await emit(page, { type: 'transferStarted', transferId: 't1', requestId: null, direction: 'incoming', peer: PIXEL.fingerprint, files: [FILE], total: FILE.size, sources: [] });
  await emit(page, { type: 'transferDone', transferId: 't1', direction: 'incoming', paths: ['C:\\Users\\me\\Downloads\\sendit\\IMG_2044.jpg'] });
  await expect.poll(async () => (await calls(page, 'saveHistory')).length).toBeGreaterThan(0);
  await page.reload();
  await page.getByRole('button', { name: /Pixel 8/ }).click();
  await expect(page.getByText('IMG_2044.jpg')).toBeVisible();
});
