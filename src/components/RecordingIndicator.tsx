import { useState, useEffect } from "react";

interface RecordingIndicatorProps {
  startTime: number;
  hasAudio: boolean;
}

export default function RecordingIndicator({ startTime, hasAudio }: RecordingIndicatorProps) {
  const [elapsed, setElapsed] = useState(0);

  useEffect(() => {
    const interval = setInterval(() => {
      setElapsed(Math.floor((Date.now() - startTime) / 1000));
    }, 1000);
    return () => clearInterval(interval);
  }, [startTime]);

  const minutes = Math.floor(elapsed / 60);
  const seconds = elapsed % 60;
  const timeStr = `${minutes.toString().padStart(2, "0")}:${seconds.toString().padStart(2, "0")}`;

  return (
    <div className="flex items-center gap-2 px-3 py-1.5 bg-red-900/40 border border-red-800/50 rounded">
      <span className="relative flex h-2.5 w-2.5">
        <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-red-500 opacity-75" />
        <span className="relative inline-flex rounded-full h-2.5 w-2.5 bg-red-500" />
      </span>
      <span className="text-sm text-red-300 font-medium">Recording {timeStr}</span>
      {hasAudio ? (
        <span className="text-xs text-emerald-400">Audio</span>
      ) : (
        <span className="text-xs text-amber-400">No audio — enable Stereo Mix</span>
      )}
      <span className="text-xs text-red-400/70">(F10 to stop)</span>
    </div>
  );
}
