// Records the README demo (video + screenshots) against the mock backend.
// Run: DEMO=1 npx playwright test e2e/demo.spec.ts   (skipped otherwise)
import { expect, test, type Page } from '@playwright/test';

test.skip(!process.env.DEMO, 'demo recording only');
test.use({ viewport: { width: 980, height: 640 }, video: { mode: 'on', size: { width: 980, height: 640 } } });

const IPHONE = { fingerprint: 'E7D4C3B2A1908F7E'.padEnd(64, '8'), alias: 'iPhone', deviceModel: 'iPhone', deviceType: 'mobile', addr: '192.168.1.100:53317', trust: 'known', isSendit: false };
const MAC = { fingerprint: '5B1E9A0C44D2E8F1'.padEnd(64, '2'), alias: "Nont's MacBook", deviceModel: 'macos', deviceType: 'desktop', addr: '192.168.1.42:53317', trust: 'verified', isSendit: true };
const f = (id: string, fileName: string, size: number) => ({ id, fileName, size, fileType: '' });

// Playwright videos have no pointer, so draw one.
const cursor = () => {
  const c = document.createElement('div');
  c.innerHTML = `<svg width="22" height="22" viewBox="0 0 24 24"><path d="M4 2l16 9-7 2-3 7z" fill="#fff" stroke="#111" stroke-width="1.5" stroke-linejoin="round"/></svg>`;
  Object.assign(c.style, { position: 'fixed', left: '0', top: '0', zIndex: '99999', pointerEvents: 'none', transition: 'transform 0.05s linear' });
  addEventListener('DOMContentLoaded', () => document.body.appendChild(c));
  addEventListener('mousemove', (e) => (c.style.transform = `translate(${e.clientX - 3}px, ${e.clientY - 2}px)`));
};

async function glide(page: Page, x: number, y: number) {
  await page.mouse.move(x, y, { steps: 18 });
}
const emit = (page: Page, e: object) => page.evaluate((ev) => (window as any).__sendit.emit(ev), e);
const wait = (page: Page, ms: number) => page.waitForTimeout(ms);
const SHOTS = '../docs/media';

test('demo', async ({ page }) => {
  test.setTimeout(120_000);
  await page.addInitScript(cursor);
  await page.addInitScript(() => {
    (window as any).__senditSeed = {
      info: { alias: 'studio-pc', fingerprint: 'C0FFEE51D3'.padEnd(64, '0'), addresses: ['192.168.1.101'] },
      pasted: { kind: 'text', preview: 'demo' },
    };
  });
  await page.goto('/');
  // Real photos for thumbnails in the recording.
  await page.evaluate(() => {
    (window as any).__sendit.bridge.fileUrl = (p: string) =>
      `https://picsum.photos/seed/sendit-${encodeURIComponent(p.split(/[\\/]/).pop() ?? '')}/440/300`;
  });
  await page.mouse.move(700, 120);
  await wait(page, 2400);
  await page.screenshot({ path: `${SHOTS}/discovery.png` });

  // Devices show up.
  await emit(page, { type: 'deviceUpdated', device: IPHONE });
  await wait(page, 500);
  await emit(page, { type: 'deviceUpdated', device: MAC });
  await wait(page, 900);
  const iphone = page.getByRole('button', { name: /iPhone/ });
  const b = (await iphone.boundingBox())!;
  await glide(page, b.x + 60, b.y + b.height / 2);
  await wait(page, 300);
  await iphone.click();
  await wait(page, 1200);

  // The phone sends photos.
  const files = [f('1', 'IMG_2044.jpg', 3_900_000), f('2', 'IMG_2045.jpg', 4_100_000), f('3', 'VID_0091.mp4', 88_000_000)];
  await emit(page, { type: 'incomingRequest', request: { requestId: 'r1', from: IPHONE, files, totalSize: 96_000_000, fitsOnDisk: true, senderVerified: true } });
  await wait(page, 700);
  const fan = (await page.locator('article[aria-label="Incoming files"]').boundingBox())!;
  await glide(page, fan.x + 40, fan.y + fan.height / 2);
  await wait(page, 900);
  const accept = (await page.getByRole('button', { name: 'Accept' }).boundingBox())!;
  await glide(page, accept.x + accept.width / 2, accept.y + accept.height / 2);
  await page.getByRole('button', { name: 'Accept' }).click();
  await emit(page, { type: 'transferStarted', transferId: 't1', requestId: 'r1', direction: 'incoming', peer: IPHONE.fingerprint, files, total: 96_000_000, sources: [] });
  for (let i = 1; i <= 16; i++) {
    await wait(page, 160);
    await emit(page, { type: 'progress', transferId: 't1', direction: 'incoming', bytes: i * 6_000_000, total: 96_000_000 });
    if (i === 12) await page.screenshot({ path: `${SHOTS}/receiving.png` });
  }
  await emit(page, { type: 'transferDone', transferId: 't1', direction: 'incoming', paths: ['C:/Users/me/Downloads/sendit/IMG_2044.jpg'] });
  await wait(page, 1300);

  // A single photo arrives too (verified device, auto-accepted).
  const photo = [f('5', 'IMG_2051.jpg', 4_200_000)];
  await emit(page, { type: 'transferStarted', transferId: 't0', requestId: null, direction: 'incoming', peer: IPHONE.fingerprint, files: photo, total: 4_200_000, sources: [] });
  for (let i = 1; i <= 4; i++) {
    await wait(page, 150);
    await emit(page, { type: 'progress', transferId: 't0', direction: 'incoming', bytes: i * 1_050_000, total: 4_200_000 });
  }
  await emit(page, { type: 'transferDone', transferId: 't0', direction: 'incoming', paths: ['C:/Users/me/Downloads/sendit/IMG_2051.jpg'] });
  await wait(page, 1300);

  // Ctrl+V a link, then a screenshot.
  await page.keyboard.press('Control+V');
  await emit(page, { type: 'messageSending', id: 'm1', peer: IPHONE.fingerprint, text: 'https://github.com/KantaKan/sendit' });
  await wait(page, 700);
  await emit(page, { type: 'messageSent', id: 'm1' });
  await wait(page, 900);
  await page.keyboard.press('Control+V');
  const shot = [f('9', 'screenshot-1421.png', 1_200_000)];
  await emit(page, { type: 'transferStarted', transferId: 't2', requestId: null, direction: 'outgoing', peer: IPHONE.fingerprint, files: shot, total: 1_200_000, sources: ['C:/Users/me/Pictures/screenshot-1421.png'] });
  await wait(page, 900);
  for (let i = 1; i <= 6; i++) {
    await wait(page, 140);
    await emit(page, { type: 'progress', transferId: 't2', direction: 'outgoing', bytes: i * 200_000, total: 1_200_000 });
  }
  await emit(page, { type: 'transferDone', transferId: 't2', direction: 'outgoing', paths: [] });
  await wait(page, 1400);
  await page.screenshot({ path: `${SHOTS}/timeline.png` });

  // Hover a photo, then right-click it.
  const thumb = page.locator('.row img.tile').first();
  await expect(thumb).toBeVisible();
  const t = (await thumb.boundingBox())!;
  await glide(page, t.x + t.width / 2, t.y + t.height / 2);
  await wait(page, 1500);
  const row = (await page.locator('.row').first().boundingBox())!;
  await glide(page, row.x + 320, row.y + row.height / 2);
  await page.mouse.click(row.x + 320, row.y + row.height / 2, { button: 'right' });
  await wait(page, 1600);
  await page.keyboard.press('Escape');
  await wait(page, 600);
});
