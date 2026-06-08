import { useEffect, useState } from 'react';
import { calculateFitScale } from '../lib/scale';

interface ViewportScaleOptions {
  designWidth: number;
  designHeight: number;
  paddingX: number;
  paddingY: number;
}

function readViewportScale(options: ViewportScaleOptions) {
  return calculateFitScale({
    viewportWidth: window.innerWidth,
    viewportHeight: window.innerHeight,
    ...options
  });
}

export function useViewportScale(options: ViewportScaleOptions) {
  const { designWidth, designHeight, paddingX, paddingY } = options;
  const [scale, setScale] = useState(() =>
    typeof window === 'undefined' ? 1 : readViewportScale(options)
  );

  useEffect(() => {
    function refreshScale() {
      setScale(readViewportScale(options));
    }

    refreshScale();
    window.addEventListener('resize', refreshScale);
    window.visualViewport?.addEventListener('resize', refreshScale);

    return () => {
      window.removeEventListener('resize', refreshScale);
      window.visualViewport?.removeEventListener('resize', refreshScale);
    };
  }, [designWidth, designHeight, paddingX, paddingY]);

  return scale;
}
