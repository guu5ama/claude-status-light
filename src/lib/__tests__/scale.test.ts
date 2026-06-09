import { describe, expect, it } from 'vitest';
import {
  DESIGN_HEIGHT,
  DESIGN_WIDTH,
  VIEWPORT_PADDING_X,
  VIEWPORT_PADDING_Y
} from '../design';
import { calculateFitScale } from '../scale';

describe('calculateFitScale', () => {
  it('returns 1 when the viewport fits the design size', () => {
    expect(
      calculateFitScale({
        viewportWidth: 128,
        viewportHeight: 418,
        designWidth: DESIGN_WIDTH,
        designHeight: DESIGN_HEIGHT,
        paddingX: VIEWPORT_PADDING_X,
        paddingY: VIEWPORT_PADDING_Y
      })
    ).toBe(1);
  });

  it('shrinks proportionally when height is constrained', () => {
    expect(
      calculateFitScale({
        viewportWidth: 128,
        viewportHeight: 312,
        designWidth: DESIGN_WIDTH,
        designHeight: DESIGN_HEIGHT,
        paddingX: VIEWPORT_PADDING_X,
        paddingY: VIEWPORT_PADDING_Y
      })
    ).toBeCloseTo((312 - VIEWPORT_PADDING_Y) / DESIGN_HEIGHT, 4);
  });

  it('shrinks proportionally when width is constrained', () => {
    expect(
      calculateFitScale({
        viewportWidth: 88,
        viewportHeight: 432,
        designWidth: DESIGN_WIDTH,
        designHeight: DESIGN_HEIGHT,
        paddingX: VIEWPORT_PADDING_X,
        paddingY: VIEWPORT_PADDING_Y
      })
    ).toBeCloseTo((88 - VIEWPORT_PADDING_X) / DESIGN_WIDTH, 4);
  });

  it('never returns less than a safe minimum scale', () => {
    expect(
      calculateFitScale({
        viewportWidth: 20,
        viewportHeight: 20,
        designWidth: DESIGN_WIDTH,
        designHeight: DESIGN_HEIGHT,
        paddingX: VIEWPORT_PADDING_X,
        paddingY: VIEWPORT_PADDING_Y
      })
    ).toBe(0.4);
  });
});
