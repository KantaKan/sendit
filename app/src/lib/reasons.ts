import type { FailReason } from './types';

export function reasonText(r: FailReason): string {
  switch (r.kind) {
    case 'declined': return 'Declined';
    case 'cancelled': return 'Cancelled';
    case 'peerGone': return 'Connection lost';
    case 'busy': return 'They are busy with another transfer';
    case 'pinRequired': return 'This device asks for a PIN. Send from the CLI with --pin for now.';
    case 'blocked': return 'This device is blocked';
    case 'checksum': return 'The file arrived damaged';
    case 'diskFull': return 'Not enough disk space';
    case 'identityMismatch': return 'Key mismatch: this is not the device you trusted';
    case 'protocol': return `Protocol error: ${r.detail}`;
    case 'io': return `Disk error: ${r.detail}`;
  }
}
