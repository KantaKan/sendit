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
