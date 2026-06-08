import type { MouseEvent } from 'react';
import { useEffect, useRef, useState } from 'react';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { StatusLabel } from './components/StatusLabel';
import { StatusLight } from './components/StatusLight';
import { useSoundToggle } from './hooks/useSoundToggle';
import { useStatusState } from './hooks/useStatusState';
import { useViewportScale } from './hooks/useViewportScale';
import {
  getSetupNotice,
  getStatusLabelText,
  SETUP_SUCCESS_NOTICE_DURATION_MS,
  shouldDismissSetupNoticeForStatus,
  type ClaudeSetupStatus
} from './lib/claude-setup';
import {
  DESIGN_HEIGHT,
  DESIGN_WIDTH,
  VIEWPORT_PADDING_X,
  VIEWPORT_PADDING_Y
} from './lib/design';
import { playStatusSound, primeAudioPlayback, shouldPlayStatusSound } from './lib/sound';

function SoundIcon({ muted }: { muted: boolean }) {
  return (
    <svg
      aria-hidden="true"
      className="sound-toggle__icon"
      viewBox="0 0 24 24"
      focusable="false"
    >
      <path
        className="sound-toggle__speaker"
        d="M4.5 9.5H8l4.5-4v13l-4.5-4H4.5z"
        fill="none"
        stroke="currentColor"
        strokeLinecap="round"
        strokeLinejoin="round"
        strokeWidth="1.8"
      />
      {muted ? (
        <>
          <path
            d="M15.75 8.25l4.5 7.5"
            fill="none"
            stroke="currentColor"
            strokeLinecap="round"
            strokeWidth="1.8"
          />
          <path
            d="M20.25 8.25l-4.5 7.5"
            fill="none"
            stroke="currentColor"
            strokeLinecap="round"
            strokeWidth="1.8"
          />
        </>
      ) : (
        <>
          <path
            d="M16 9.25a4.75 4.75 0 010 5.5"
            fill="none"
            stroke="currentColor"
            strokeLinecap="round"
            strokeWidth="1.8"
          />
          <path
            d="M18.75 6.75a8.25 8.25 0 010 10.5"
            fill="none"
            stroke="currentColor"
            strokeLinecap="round"
            strokeWidth="1.8"
          />
        </>
      )}
    </svg>
  );
}

export default function App() {
  const state = useStatusState('/state/state.json', 500);
  const { soundEnabled, toggleSound } = useSoundToggle(true);
  const previousStatus = useRef(state.status);
  const toggleSoundRef = useRef(toggleSound);
  const [setupStatus, setSetupStatus] = useState<ClaudeSetupStatus | null>(null);
  const [dismissedSetupNotice, setDismissedSetupNotice] = useState(false);
  const scale = useViewportScale({
    designWidth: DESIGN_WIDTH,
    designHeight: DESIGN_HEIGHT,
    paddingX: VIEWPORT_PADDING_X,
    paddingY: VIEWPORT_PADDING_Y
  });
  const statusLabelText = getStatusLabelText(state.status, setupStatus);
  const setupNotice = dismissedSetupNotice ? null : getSetupNotice(setupStatus);

  function handleMouseDown(event: MouseEvent<HTMLElement>) {
    void primeAudioPlayback();

    if (event.button !== 0 || !isTauri()) {
      return;
    }

    event.preventDefault();
    void getCurrentWindow().startDragging();
  }

  useEffect(() => {
    toggleSoundRef.current = toggleSound;
  }, [toggleSound]);

  useEffect(() => {
    if (shouldPlayStatusSound(previousStatus.current, state.status)) {
      void playStatusSound({
        status: state.status,
        muted: !soundEnabled
      });
      previousStatus.current = state.status;
    }
  }, [soundEnabled, state.status]);

  useEffect(() => {
    if (!setupStatus || setupStatus.kind !== 'configured') {
      setDismissedSetupNotice(false);
      return;
    }

    setDismissedSetupNotice(false);
    const timeoutId = window.setTimeout(() => {
      setDismissedSetupNotice(true);
    }, SETUP_SUCCESS_NOTICE_DURATION_MS);

    return () => {
      window.clearTimeout(timeoutId);
    };
  }, [setupStatus]);

  useEffect(() => {
    if (shouldDismissSetupNoticeForStatus(setupStatus, state.status)) {
      setDismissedSetupNotice(true);
    }
  }, [setupStatus, state.status]);

  useEffect(() => {
    let unlistenToggleSound: (() => void) | undefined;
    let unlistenReconnectSession: (() => void) | undefined;
    let unlistenSetupStatus: (() => void) | undefined;

    async function bindTrayEvents() {
      if (!isTauri()) {
        return;
      }

      try {
        const currentSetupStatus = await invoke<ClaudeSetupStatus>('get_claude_setup_status');
        setSetupStatus(currentSetupStatus);
      } catch {
        setSetupStatus(null);
      }

      unlistenToggleSound = await listen('toggle-sound', () => {
        toggleSoundRef.current();
      });

      unlistenReconnectSession = await listen('reconnect-session', () => {
        void invoke('reset_session_binding');
      });

      unlistenSetupStatus = await listen<ClaudeSetupStatus>('claude-setup-status', (event) => {
        setSetupStatus(event.payload);
      });
    }

    void bindTrayEvents();

    return () => {
      if (unlistenToggleSound) {
        unlistenToggleSound();
      }
      if (unlistenReconnectSession) {
        unlistenReconnectSession();
      }
      if (unlistenSetupStatus) {
        unlistenSetupStatus();
      }
    };
  }, []);

  return (
    <main className="app-shell" data-tauri-drag-region onMouseDown={handleMouseDown}>
      <div
        className="app-scale-frame"
        style={{
          width: `${DESIGN_WIDTH}px`,
          height: `${DESIGN_HEIGHT}px`,
          transform: `scale(${scale})`
        }}
      >
        <div className="app-signal-wrap">
          <StatusLight status={state.status} />
        </div>
        <div className="app-status-stack">
          <StatusLabel status={state.status} text={statusLabelText} />
          {setupNotice ? (
            <div
              className={`setup-note setup-note--${setupNotice.tone}`}
              title={setupNotice.detail ?? undefined}
            >
              <div className="setup-note__title">{setupNotice.title}</div>
              {setupNotice.detail ? (
                <div className="setup-note__detail">{setupNotice.detail}</div>
              ) : null}
            </div>
          ) : null}
        </div>
        <button
          type="button"
          className="sound-toggle sound-toggle--bottom"
          aria-label={soundEnabled ? 'Mute status sounds' : 'Enable status sounds'}
          aria-pressed={soundEnabled}
          title={soundEnabled ? 'Mute status sounds' : 'Enable status sounds'}
          onMouseDown={(event) => {
            event.stopPropagation();
            void primeAudioPlayback();
          }}
          onClick={() => {
            toggleSound();
          }}
        >
          <SoundIcon muted={!soundEnabled} />
        </button>
      </div>
    </main>
  );
}
