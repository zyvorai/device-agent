import { beforeEach, describe, expect, it } from 'vitest';
import { bitrate, bytes, duration, getToken, setToken, withStreamTicket } from './api';

describe('formatters', () => {
  it('formats bytes', () => expect(bytes(8 * 1024 ** 3)).toBe('8.0 GiB'));
  it('formats uptime', () => expect(duration(90061)).toBe('1d 1h'));
  it('formats CAN bitrate', () => expect(bitrate(500000)).toBe('500 kbit/s'));
  it('formats CAN-FD data bitrate', () => expect(bitrate(2000000)).toBe('2 Mbit/s'));
});

describe('bearer token storage', () => {
  beforeEach(() => setToken(''));

  it('defaults to an empty token', () => expect(getToken()).toBe(''));

  it('round-trips a saved token', () => {
    setToken('abc123');
    expect(getToken()).toBe('abc123');
  });

  it('clears a token when saved as empty', () => {
    setToken('abc123');
    setToken('');
    expect(getToken()).toBe('');
  });
});

describe('withStreamTicket', () => {
  beforeEach(() => setToken(''));

  it('returns the path unchanged when no bearer is stored', async () => {
    expect(await withStreamTicket('/api/v1/camera/dock/stream')).toBe('/api/v1/camera/dock/stream');
  });
});
