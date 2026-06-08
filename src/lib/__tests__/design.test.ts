import { describe, expect, it } from 'vitest';
import { DESIGN_HEIGHT, MINIMUM_REQUIRED_HEIGHT } from '../design';

describe('design metrics', () => {
  it('reserves enough vertical space for the traffic light and status label', () => {
    expect(DESIGN_HEIGHT).toBeGreaterThanOrEqual(MINIMUM_REQUIRED_HEIGHT);
  });
});
