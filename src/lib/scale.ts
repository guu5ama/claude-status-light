interface ScaleOptions {
  viewportWidth: number;
  viewportHeight: number;
  designWidth: number;
  designHeight: number;
  paddingX: number;
  paddingY: number;
  minimumScale?: number;
}

export function calculateFitScale({
  viewportWidth,
  viewportHeight,
  designWidth,
  designHeight,
  paddingX,
  paddingY,
  minimumScale = 0.4
}: ScaleOptions): number {
  const availableWidth = Math.max(viewportWidth - paddingX, 1);
  const availableHeight = Math.max(viewportHeight - paddingY, 1);
  const widthScale = availableWidth / designWidth;
  const heightScale = availableHeight / designHeight;

  return Math.max(minimumScale, Math.min(widthScale, heightScale, 1));
}
