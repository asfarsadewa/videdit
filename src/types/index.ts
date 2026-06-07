export interface VideoInfo {
  path: string;
  duration: number;
  width: number;
  height: number;
  codec: string;
  fps: number;
  subtitleTracks: EmbeddedSubtitleTrack[];
}

export interface EmbeddedSubtitleTrack {
  streamIndex: number;
  subtitleIndex: number;
  codec: string;
  language: string | null;
  title: string | null;
  isDefault: boolean;
  isForced: boolean;
  supportedForBurn: boolean;
  label: string;
}

export interface Segment {
  id: string;
  start: number;
  end: number;
}

export interface ExportOptions {
  inputPath: string;
  segments: Segment[];
  outputPath: string;
  merge: boolean;
  compress: boolean;
  quality: number;
  exportRange: 'segments' | 'whole';
  vhsEffect: boolean;
  vhsIntensity: number;
  vhsScanlines: boolean;
  vhsColorProfile: 'faded' | 'preserved';
}

export interface RecordingStartedPayload {
  hasAudio: boolean;
  audioDevice: string | null;
}

export interface ExportProgress {
  segmentIndex: number;
  totalSegments: number;
  percent: number;
  phase: "cutting" | "merging" | "done" | "error";
  message: string;
}

export interface Subtitle {
  id: string;
  start: number;
  end: number;
  text: string;
}

export interface AudioTrack {
  id: string;
  segmentId: string;     // Which segment this audio is attached to
  filePath: string;
  fileName: string;
  duration: number;
  audioSourceStart: number;  // Start time within the audio file (trim start)
  audioSourceEnd: number;    // End time within the audio file (trim end)
  volume: number;
  radioEffect: boolean;
  radioIntensity: number;
}
