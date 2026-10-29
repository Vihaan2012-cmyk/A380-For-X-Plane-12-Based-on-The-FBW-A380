import type { HostSource } from './host.ts';
import { describeError } from './host.ts';

interface ErrorTarget {
  addEventListener(type: string, listener: (event: any) => void): void;
}

export function errorEventText(event: { message?: unknown; error?: unknown }): string {
  const message = event.message !== undefined && event.message !== null ? String(event.message) : '';
  const err = event.error as { stack?: unknown } | null | undefined;
  if (message === '' && err !== undefined && err !== null) {
    return describeError(err);
  }
  if (err && typeof err === 'object' && typeof err.stack === 'string' && err.stack !== '') {
    return message + '\n' + err.stack;
  }
  return message;
}

export function installErrorForwarding(target: ErrorTarget, host: HostSource): void {
  target.addEventListener('error', (event: { message?: unknown; error?: unknown }) => {
    const h = host();
    if (h) {
      h.log(2, errorEventText(event));
    }
  });
  target.addEventListener('unhandledrejection', (event: { reason?: unknown }) => {
    const h = host();
    if (h) {
      h.log(2, describeError(event.reason));
    }
  });
}
