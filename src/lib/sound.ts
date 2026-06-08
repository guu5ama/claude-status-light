import type { StatusKind } from './types';

export function shouldPlayStatusSound(previous: StatusKind, next: StatusKind): boolean {
  return previous !== next;
}

function createNativeAudio(src: string): { play: () => Promise<void> | void } | null {
  if (typeof window.Audio !== 'function') {
    return null;
  }

  return new window.Audio(src);
}

type MinimalAudioContext = Pick<
  AudioContext,
  'state' | 'resume' | 'createOscillator' | 'createGain' | 'destination'
>;
type UnlockableAudioContext = Pick<AudioContext, 'state' | 'resume'>;

let sharedAudioContext: MinimalAudioContext | null = null;

function resolveAudioContextCtor() {
  const AudioContextCtor = window.AudioContext ?? (window as typeof window & {
    webkitAudioContext?: typeof AudioContext;
  }).webkitAudioContext;

  if (!AudioContextCtor) {
    return null;
  }

  return AudioContextCtor;
}

function getOrCreateAudioContext(
  createAudioContext?: () => MinimalAudioContext | null
): MinimalAudioContext | null {
  if (sharedAudioContext) {
    return sharedAudioContext;
  }

  if (createAudioContext) {
    sharedAudioContext = createAudioContext();
    return sharedAudioContext;
  }

  const AudioContextCtor = resolveAudioContextCtor();
  if (!AudioContextCtor) {
    return null;
  }

  sharedAudioContext = new AudioContextCtor();
  return sharedAudioContext;
}

export async function primeAudioPlayback({
  createAudioContext
}: {
  createAudioContext?: () => UnlockableAudioContext | null;
} = {}) {
  const context =
    createAudioContext?.() ?? getOrCreateAudioContext();
  if (!context) {
    return;
  }

  if (context.state === 'suspended') {
    await context.resume();
  }
}

async function playTone(frequency: number, durationMs: number) {
  const context = getOrCreateAudioContext();
  if (!context) {
    return;
  }

  if (context.state === 'suspended') {
    await context.resume();
  }

  const oscillator = context.createOscillator();
  const gain = context.createGain();

  oscillator.type = 'sine';
  oscillator.frequency.value = frequency;
  gain.gain.value = 0.05;

  oscillator.connect(gain);
  gain.connect(context.destination);
  oscillator.start();

  await new Promise((resolve) => window.setTimeout(resolve, durationMs));

  oscillator.stop();
  oscillator.disconnect();
  gain.disconnect();
}

export async function playStatusSound({
  status,
  muted,
  createAudio
}: {
  status: StatusKind;
  muted: boolean;
  createAudio?: (src: string) => { play: () => Promise<void> | void };
}) {
  if (muted || status === 'idle_unbound' || status === 'bridge_disconnected') {
    return;
  }

  if (status === 'running') {
    return;
  }

  const src = status === 'done' ? '/sounds/done.mp3' : '/sounds/pending.mp3';
  const player = createAudio ? createAudio(src) : createNativeAudio(src);

  if (player) {
    try {
      await player.play();
      return;
    } catch {
      // Fall through to synthesized tones when file playback is unavailable.
    }
  }

  if (status === 'done') {
    await playTone(880, 140);
    return;
  }

  await playTone(520, 180);
}
