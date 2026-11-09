import { describe, expect, it } from 'vitest';
import { poolOptions } from './client';

describe('db pool options', () => {
  it('bounds every query with a server-side statement timeout', () => {
    expect(poolOptions.connection.statement_timeout).toBeGreaterThan(0);
    expect(poolOptions.connection.statement_timeout).toBeLessThanOrEqual(30000);
  });
});
