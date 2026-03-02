import { useRef, useCallback, type MouseEvent } from "react";
import type { Segment, Subtitle } from "../types";
import { formatTime } from "../utils/format";

interface TimelineProps {
  duration: number;
  currentTime: number;
  segments: Segment[];
  subtitles: Subtitle[];
  onSeek: (time: number) => void;
  onSegmentUpdate: (id: string, start: number, end: number) => void;
}

export default function Timeline({
  duration,
  currentTime,
  segments,
  subtitles,
  onSeek,
  onSegmentUpdate,
}: TimelineProps) {
  const trackRef = useRef<HTMLDivElement>(null);
  const dragging = useRef<{
    segId: string;
    handle: "start" | "end" | "body";
    offsetRatio: number;
  } | null>(null);

  const getTimeFromX = useCallback(
    (clientX: number) => {
      const track = trackRef.current;
      if (!track || duration <= 0) return 0;
      const rect = track.getBoundingClientRect();
      const ratio = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
      return ratio * duration;
    },
    [duration],
  );

  const handleTrackClick = useCallback(
    (e: MouseEvent) => {
      if (dragging.current) return;
      onSeek(getTimeFromX(e.clientX));
    },
    [getTimeFromX, onSeek],
  );

  const handleSegmentMouseDown = useCallback(
    (e: MouseEvent, segId: string, handle: "start" | "end" | "body") => {
      e.stopPropagation();
      const seg = segments.find((s) => s.id === segId);
      if (!seg) return;

      const time = getTimeFromX(e.clientX);
      const offsetRatio = handle === "body" ? (time - seg.start) / duration : 0;
      dragging.current = { segId, handle, offsetRatio };

      function onMouseMove(ev: globalThis.MouseEvent) {
        if (!dragging.current || !trackRef.current) return;
        const d = dragging.current;
        const t = getTimeFromX(ev.clientX);
        const s = segments.find((s) => s.id === d.segId);
        if (!s) return;

        if (d.handle === "start") {
          onSegmentUpdate(d.segId, Math.min(t, s.end - 0.1), s.end);
        } else if (d.handle === "end") {
          onSegmentUpdate(d.segId, s.start, Math.max(t, s.start + 0.1));
        } else {
          const len = s.end - s.start;
          const newStart = Math.max(0, Math.min(duration - len, t - d.offsetRatio * duration));
          onSegmentUpdate(d.segId, newStart, newStart + len);
        }
      }

      function onMouseUp() {
        dragging.current = null;
        window.removeEventListener("mousemove", onMouseMove);
        window.removeEventListener("mouseup", onMouseUp);
      }

      window.addEventListener("mousemove", onMouseMove);
      window.addEventListener("mouseup", onMouseUp);
    },
    [segments, duration, getTimeFromX, onSegmentUpdate],
  );

  if (duration <= 0) return null;

  const playheadPos = (currentTime / duration) * 100;

  // Generate time labels
  const labelCount = Math.min(10, Math.max(2, Math.floor(duration / 10)));
  const labels = Array.from({ length: labelCount + 1 }, (_, i) => {
    const t = (i / labelCount) * duration;
    return { time: t, pos: (t / duration) * 100 };
  });

  return (
    <div className="px-4 py-3 select-none">
      {/* Time labels */}
      <div className="relative h-5 text-[10px] text-zinc-500">
        {labels.map((l) => (
          <span
            key={l.time}
            className="absolute -translate-x-1/2"
            style={{ left: `${l.pos}%` }}
          >
            {formatTime(l.time)}
          </span>
        ))}
      </div>

      {/* Track */}
      <div
        ref={trackRef}
        className="relative h-10 bg-zinc-800 rounded cursor-pointer"
        onMouseDown={handleTrackClick}
      >
        {/* Segments */}
        {segments.map((seg) => {
          const left = (seg.start / duration) * 100;
          const width = ((seg.end - seg.start) / duration) * 100;
          return (
            <div
              key={seg.id}
              className="absolute top-0 h-full bg-emerald-600/50 border border-emerald-400/60 rounded-sm group"
              style={{ left: `${left}%`, width: `${width}%` }}
              onMouseDown={(e) => handleSegmentMouseDown(e, seg.id, "body")}
            >
              {/* Left handle */}
              <div
                className="absolute left-0 top-0 w-2 h-full cursor-col-resize bg-emerald-400/80 rounded-l-sm opacity-0 group-hover:opacity-100 transition-opacity"
                onMouseDown={(e) => handleSegmentMouseDown(e, seg.id, "start")}
              />
              {/* Right handle */}
              <div
                className="absolute right-0 top-0 w-2 h-full cursor-col-resize bg-emerald-400/80 rounded-r-sm opacity-0 group-hover:opacity-100 transition-opacity"
                onMouseDown={(e) => handleSegmentMouseDown(e, seg.id, "end")}
              />
            </div>
          );
        })}

        {/* Subtitles (smaller markers below segments) */}
        {subtitles.map((sub) => {
          const left = (sub.start / duration) * 100;
          const width = ((sub.end - sub.start) / duration) * 100;
          return (
            <div
              key={sub.id}
              className="absolute bottom-0 h-2 bg-cyan-600/60 border border-cyan-400/60 rounded-sm hover:bg-cyan-500/80 cursor-pointer"
              style={{ left: `${left}%`, width: `${width}%` }}
              onMouseDown={(e) => {
                e.stopPropagation();
                onSeek(sub.start);
              }}
              title={sub.text || "Subtitle"}
            />
          );
        })}

        {/* Playhead */}
        <div
          className="absolute top-0 w-0.5 h-full bg-red-500 pointer-events-none z-10"
          style={{ left: `${playheadPos}%` }}
        >
          <div className="absolute -top-1.5 left-1/2 -translate-x-1/2 w-2.5 h-2.5 bg-red-500 rounded-full" />
        </div>
      </div>
    </div>
  );
}
