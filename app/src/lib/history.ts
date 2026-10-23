import type { State, Ticket, TicketStatus } from './state';

const FINISHED: TicketStatus[] = ['done', 'failed', 'declined', 'cancelled', 'expired', 'message'];

export const isFinished = (t: Ticket) => FINISHED.includes(t.status);

/** What goes in history.json: finished items, oldest first, without live-only fields. */
export function toSaved(tickets: Ticket[]): Ticket[] {
  return tickets
    .filter(isFinished)
    .map((t) => {
      const c = { ...t };
      delete c.speeds;
      delete c.lastAt;
      return c;
    })
    .sort((a, b) => (a.doneAt ?? a.at) - (b.doneAt ?? b.at));
}

/** Saved items go before anything that arrived while loading. */
export function restore(s: State, saved: Ticket[]): State {
  const have = new Set(s.tickets.map((t) => t.key));
  return { ...s, tickets: [...saved.filter((t) => !have.has(t.key)), ...s.tickets] };
}

/** "Clear history": forget finished items; files on disk are untouched. */
export const clearFinished = (s: State): State => ({ ...s, tickets: s.tickets.filter((t) => !isFinished(t)) });
