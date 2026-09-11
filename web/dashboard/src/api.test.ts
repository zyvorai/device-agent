import { describe, expect, it } from 'vitest';
import { bytes, duration } from './api';

describe('formatters', () => {
  it('formats bytes', () => expect(bytes(8 * 1024 ** 3)).toBe('8.0 GiB'));
  it('formats uptime', () => expect(duration(90061)).toBe('1d 1h'));
});
