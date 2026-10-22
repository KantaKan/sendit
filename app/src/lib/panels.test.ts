import { fireEvent, render, screen } from '@testing-library/svelte';
import { expect, test, vi } from 'vitest';
import IdentityPanel from './IdentityPanel.svelte';
import VerifyPanel from './VerifyPanel.svelte';

test('verify panel shows both faces and answers', async () => {
  const onyes = vi.fn();
  const onskip = vi.fn();
  render(VerifyPanel, { props: { theirs: 'A'.repeat(64), mine: 'B'.repeat(64), onyes, onskip } });
  expect(screen.getByText('Same face on both screens?')).toBeInTheDocument();
  expect(screen.getAllByRole('img')).toHaveLength(2);
  await fireEvent.click(screen.getByRole('button', { name: 'Yes, trust' }));
  await fireEvent.click(screen.getByRole('button', { name: 'Skip' }));
  expect(onyes).toHaveBeenCalled();
  expect(onskip).toHaveBeenCalled();
});

test('identity panel warns with both key prefixes', async () => {
  const onblock = vi.fn();
  const ontrust = vi.fn();
  render(IdentityPanel, {
    props: { alias: 'Pixel 8', oldFingerprint: 'A91F3C07E2B4'.padEnd(64, '0'), newFingerprint: '7D10B6E904FA'.padEnd(64, '0'), onblock, ontrust },
  });
  expect(screen.getByRole('alert')).toHaveTextContent("This isn't the Pixel 8 you know");
  expect(screen.getByText('a91f 3c07 e2b4')).toBeInTheDocument();
  expect(screen.getByText('7d10 b6e9 04fa')).toBeInTheDocument();
  await fireEvent.click(screen.getByRole('button', { name: 'Block' }));
  await fireEvent.click(screen.getByRole('button', { name: "It's a reinstall, trust new key" }));
  expect(onblock).toHaveBeenCalled();
  expect(ontrust).toHaveBeenCalled();
});

test('identity panel copes with an unknown old key', () => {
  render(IdentityPanel, { props: { alias: 'x', newFingerprint: 'C'.repeat(64), onblock: vi.fn(), ontrust: vi.fn() } });
  expect(screen.getByText('unknown')).toBeInTheDocument();
});
