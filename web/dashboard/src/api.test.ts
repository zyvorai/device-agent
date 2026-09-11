import { describe, expect, it } from 'vitest';
import { bitrate, bytes, duration } from './api';

describe('formatters', () => {
  it('formats bytes', () => expect(bytes(8 * 1024 ** 3)).toBe('8.0 GiB'));
  it('formats uptime', () => expect(duration(90061)).toBe('1d 1h'));
  it('formats CAN bitrate', () => expect(bitrate(500000)).toBe('500 kbit/s'));
  it('formats CAN-FD data bitrate', () => expect(bitrate(2000000)).toBe('2 Mbit/s'));
});
