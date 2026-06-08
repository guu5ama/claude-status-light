import { describe, expect, it, vi } from 'vitest';
import { playStatusSound, primeAudioPlayback, shouldPlayStatusSound } from '../sound';

describe('shouldPlayStatusSound', () => {
  it('plays when moving from running to done', () => {
    expect(shouldPlayStatusSound('running', 'done')).toBe(true);
  });

  it('does not play when state does not change', () => {
    expect(shouldPlayStatusSound('pending_user', 'pending_user')).toBe(false);
  });
});

describe('playStatusSound', () => {
  it('does nothing when muted', async () => {
    const play = vi.fn().mockResolvedValue(undefined);

    await playStatusSound({
      status: 'done',
      muted: true,
      createAudio: () => ({ play })
    });

    expect(play).not.toHaveBeenCalled();
  });

  it('uses the done audio file when status becomes done', async () => {
    const play = vi.fn().mockResolvedValue(undefined);
    const createAudio = vi.fn().mockReturnValue({ play });

    await playStatusSound({
      status: 'done',
      muted: false,
      createAudio
    });

    expect(createAudio).toHaveBeenCalledWith('/sounds/done.mp3');
    expect(play).toHaveBeenCalledTimes(1);
  });

  it('uses the pending audio file when status becomes pending_user', async () => {
    const play = vi.fn().mockResolvedValue(undefined);
    const createAudio = vi.fn().mockReturnValue({ play });

    await playStatusSound({
      status: 'pending_user',
      muted: false,
      createAudio
    });

    expect(createAudio).toHaveBeenCalledWith('/sounds/pending.mp3');
    expect(play).toHaveBeenCalledTimes(1);
  });

  it('uses a native Audio element by default when available', async () => {
    const play = vi.fn().mockResolvedValue(undefined);
    const originalAudio = window.Audio;
    const audioMock = vi.fn().mockImplementation(() => ({ play }));

    window.Audio = audioMock as unknown as typeof Audio;

    try {
      await playStatusSound({
        status: 'done',
        muted: false
      });
    } finally {
      window.Audio = originalAudio;
    }

    expect(audioMock).toHaveBeenCalledWith('/sounds/done.mp3');
    expect(play).toHaveBeenCalledTimes(1);
  });
});

describe('primeAudioPlayback', () => {
  it('resumes a suspended audio context when one is available', async () => {
    const resume = vi.fn().mockResolvedValue(undefined);

    await primeAudioPlayback({
      createAudioContext: () =>
        ({
          state: 'suspended',
          resume
        }) as Pick<AudioContext, 'state' | 'resume'>
    });

    expect(resume).toHaveBeenCalledTimes(1);
  });
});
