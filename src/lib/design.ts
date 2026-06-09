export const DESIGN_WIDTH = 112;
export const DESIGN_HEIGHT = 412;
export const VIEWPORT_PADDING_X = 16;
export const VIEWPORT_PADDING_Y = 6;

const TRAFFIC_LIGHT_VISUAL_HEIGHT = 246;
const STATUS_LABEL_VISUAL_HEIGHT = 12;
const USAGE_PANEL_VISUAL_HEIGHT = 80;
const SOUND_TOGGLE_VISUAL_HEIGHT = 22;
const STACK_GAP = 12;
const FRAME_VERTICAL_PADDING = 22;

// The window is sized to the steady-state layout (no setup note). The setup
// note is a transient startup message and may briefly overflow when shown.
export const MINIMUM_REQUIRED_HEIGHT =
  TRAFFIC_LIGHT_VISUAL_HEIGHT +
  STATUS_LABEL_VISUAL_HEIGHT +
  USAGE_PANEL_VISUAL_HEIGHT +
  SOUND_TOGGLE_VISUAL_HEIGHT +
  STACK_GAP +
  FRAME_VERTICAL_PADDING;

// Height when the area below the traffic light is hidden: just the signal
// body and the sound control. Keeps the light at full scale when collapsed.
export const COLLAPSED_DESIGN_HEIGHT =
  TRAFFIC_LIGHT_VISUAL_HEIGHT +
  SOUND_TOGGLE_VISUAL_HEIGHT +
  STACK_GAP +
  FRAME_VERTICAL_PADDING;

// Physical window sizes (logical px). WINDOW_WIDTH/WINDOW_HEIGHT mirror
// tauri.conf.json; the collapsed height preserves the same breathing room.
export const WINDOW_WIDTH = 128;
export const WINDOW_HEIGHT = 418;
export const COLLAPSED_WINDOW_HEIGHT = WINDOW_HEIGHT - DESIGN_HEIGHT + COLLAPSED_DESIGN_HEIGHT;
