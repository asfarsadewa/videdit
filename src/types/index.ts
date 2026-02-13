export interface VideoInfo {
  path: string;
  duration: number;
  width: number;
  height: number;
  codec: string;
  fps: number;
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
}

export interface ExportProgress {
  segmentIndex: number;
  totalSegments: number;
  percent: number;
  phase: "cutting" | "merging" | "done" | "error";
  message: string;
}
