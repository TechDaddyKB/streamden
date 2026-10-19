import { EVENT_KINDS, type EventKind } from "./generated/event-kind.ts";

export function parseEventKind(raw: string): EventKind | undefined {
  return EVENT_KINDS.find((kind) => kind === raw);
}
