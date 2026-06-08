import { useEffect, useState } from 'react';

const STORAGE_KEY = 'claude-status-light:sound-enabled';

export function useSoundToggle(defaultValue = true) {
  const [enabled, setEnabled] = useState(defaultValue);

  useEffect(() => {
    const saved = window.localStorage.getItem(STORAGE_KEY);
    if (saved === 'true' || saved === 'false') {
      setEnabled(saved === 'true');
    }
  }, []);

  useEffect(() => {
    window.localStorage.setItem(STORAGE_KEY, String(enabled));
  }, [enabled]);

  return {
    soundEnabled: enabled,
    toggleSound: () => setEnabled((current) => !current)
  };
}
