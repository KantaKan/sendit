const UNITS = ['B', 'KB', 'MB', 'GB', 'TB'];

export function formatBytes(n: number): string {
  let v = n;
  let i = 0;
  while (v >= 1000 && i < UNITS.length - 1) {
    v /= 1000;
    i++;
  }
  if (i === 0) return `${n} B`;
  return `${v < 10 ? v.toFixed(1) : Math.round(v)} ${UNITS[i]}`;
}

export const percent = (bytes: number, total: number) =>
  total === 0 ? 100 : Math.min(100, Math.floor((bytes * 100) / total));

export const clock = (ms: number) => new Date(ms).toTimeString().slice(0, 5);

export function extLabel(name: string): string {
  const m = /\.([a-z0-9]{1,4})$/i.exec(name);
  return m ? m[1].toUpperCase() : 'FILE';
}

/** "a91f 3c07 e2b4": enough to compare by eye. */
export const shortFp = (fp: string) => fp.toLowerCase().slice(0, 12).replace(/(.{4})(?=.)/g, '$1 ');

/** Last two path segments, for compact labels like "Downloads/sendit". */
export const lastTwo = (path: string) => path.split(/[\\/]+/).filter(Boolean).slice(-2).join('/');

/** The address other devices most likely reach us on: home LAN ranges first,
 * then anything that is not link-local (169.254.x). */
export function primaryIp(addresses: string[]): string | null {
  const rank = (ip: string) =>
    ip.startsWith('192.168.') ? 0 : ip.startsWith('10.') ? 1 : ip.startsWith('169.254.') ? 3 : 2;
  return [...addresses].sort((a, b) => rank(a) - rank(b))[0] ?? null;
}

/** "Cmd" on macOS, "Ctrl" elsewhere. */
export const modKey = () => (/Mac|iPhone|iPad/.test(navigator.userAgent) ? 'Cmd' : 'Ctrl');

const IMAGE = /\.(jpe?g|png|gif|webp|heic|heif|bmp|avif)$/i;
const VIDEO = /\.(mp4|mov|m4v|mkv|webm|avi)$/i;
export const isImage = (name: string) => IMAGE.test(name);
export const isVideo = (name: string) => VIDEO.test(name);

/** "204 of 318 MB": both numbers in the total's unit. */
export function bytesOf(bytes: number, total: number): string {
  let i = 0;
  while (total / 1000 ** (i + 1) >= 1 && i < UNITS.length - 1) i++;
  const f = (n: number) => {
    const v = n / 1000 ** i;
    return i === 0 ? String(n) : v < 10 ? v.toFixed(1) : String(Math.round(v));
  };
  return `${f(bytes)} of ${f(total)} ${UNITS[i]}`;
}

/** Seconds as m:ss or h:mm:ss. */
export function eta(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return '--';
  const s = Math.round(seconds);
  const pad = (n: number) => String(n).padStart(2, '0');
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  return h ? `${h}:${pad(m)}:${pad(s % 60)}` : `${m}:${pad(s % 60)}`;
}

/** "2 photos, 1 video" / "3 files". */
export function summarize(files: { fileName: string }[]): string {
  const photos = files.filter((f) => isImage(f.fileName)).length;
  const videos = files.filter((f) => isVideo(f.fileName)).length;
  const other = files.length - photos - videos;
  const part = (n: number, word: string) => (n ? `${n} ${word}${n === 1 ? '' : 's'}` : '');
  return [part(photos, 'photo'), part(videos, 'video'), part(other, 'file')].filter(Boolean).join(', ');
}
