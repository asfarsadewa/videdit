import { useState } from "react";
import type { Segment, AudioTrack } from "../types";
import { formatTime } from "../utils/format";

interface MicDevice {
  id: string;
  name: string;
  isDefault: boolean;
}

interface SegmentListProps {
  segments: Segment[];
  audioTracks: AudioTrack[];
  onDelete: (id: string) => void;
  onSeek: (time: number) => void;
  onAddAudio: (segmentId: string) => void;
  onDeleteAudio: (segmentId: string) => void;
  onRecordAudio: (segmentId: string, deviceId: string, durationSecs: number) => void;
  onStopRecording: () => void;
  micDevices: MicDevice[];
  isRecording: boolean;
  recordingSegmentId: string | null;
}

export default function SegmentList({
  segments,
  audioTracks,
  onDelete,
  onSeek,
  onAddAudio,
  onDeleteAudio,
  onRecordAudio,
  onStopRecording,
  micDevices,
  isRecording,
  recordingSegmentId,
}: SegmentListProps) {
  const [selectedMicDevice, setSelectedMicDevice] = useState<string>(
    () => micDevices.find(d => d.isDefault)?.id || micDevices[0]?.id || ""
  );

  if (segments.length === 0) {
    return (
      <div className="text-zinc-500 text-sm text-center py-4">
        No segments. Press <kbd className="px-1.5 py-0.5 bg-zinc-700 rounded text-xs">I</kbd> to mark in and{" "}
        <kbd className="px-1.5 py-0.5 bg-zinc-700 rounded text-xs">O</kbd> to mark out.
      </div>
    );
  }

  return (
    <div className="space-y-1 max-h-40 overflow-y-auto">
      {/* Mic device selector */}
      {micDevices.length > 0 && !isRecording && (
        <div className="flex items-center gap-2 px-2 py-1.5 mb-1">
          <span className="text-[10px] text-zinc-500">Mic:</span>
          <select
            value={selectedMicDevice}
            onChange={(e) => setSelectedMicDevice(e.target.value)}
            className="bg-zinc-800 text-[10px] text-zinc-300 rounded px-1 py-0.5 border border-zinc-700 outline-none"
          >
            {micDevices.map((d) => (
              <option key={d.id} value={d.id}>
                {d.name} {d.isDefault ? "(default)" : ""}
              </option>
            ))}
          </select>
        </div>
      )}
      
      {segments.map((seg, i) => {
        const audioTrack = audioTracks.find(t => t.segmentId === seg.id);
        const isThisRecording = isRecording && recordingSegmentId === seg.id;
        const segDuration = seg.end - seg.start;

        return (
          <div
            key={seg.id}
            className={`flex flex-col gap-1.5 px-2 py-1.5 rounded text-sm group ${
              isThisRecording ? "bg-red-900/30 border border-red-500/50" : "bg-zinc-800 hover:bg-zinc-750"
            }`}
          >
            {/* Row 1: segment info */}
            <div className="flex items-center gap-2">
              <span className="text-emerald-400 font-mono text-xs w-5">{i + 1}</span>
              <button
                className="text-zinc-300 hover:text-white font-mono text-xs"
                onClick={() => onSeek(seg.start)}
              >
                {formatTime(seg.start)}
              </button>
              <span className="text-zinc-600">→</span>
              <button
                className="text-zinc-300 hover:text-white font-mono text-xs"
                onClick={() => onSeek(seg.end)}
              >
                {formatTime(seg.end)}
              </button>
              <span className="text-zinc-500 text-xs ml-auto">
                {(seg.end - seg.start).toFixed(1)}s
              </span>
              <button
                className="text-zinc-600 hover:text-red-400 opacity-0 group-hover:opacity-100 transition-opacity text-xs px-1"
                onClick={() => onDelete(seg.id)}
                title="Remove segment"
              >
                ✕
              </button>
            </div>

            {/* Row 2: audio assignment */}
            <div className="flex items-center gap-2 pl-7">
              {audioTrack ? (
                <>
                  <span className="text-[10px] text-amber-400 truncate max-w-[120px]" title={audioTrack.fileName}>
                    ♪ {audioTrack.fileName}
                  </span>
                  <button
                    className="text-[10px] text-zinc-500 hover:text-red-400 transition-colors"
                    onClick={() => onDeleteAudio(seg.id)}
                    title="Remove audio"
                  >
                    ✕
                  </button>
                </>
              ) : isThisRecording ? (
                <>
                  <span className="text-[10px] text-red-400 animate-pulse">
                    ● Recording...
                  </span>
                  <button
                    className="text-[10px] bg-zinc-700 hover:bg-zinc-600 text-white px-1.5 py-0.5 rounded transition-colors ml-2"
                    onClick={onStopRecording}
                    title="Stop recording (F11)"
                  >
                    ■ Stop
                  </button>
                </>
              ) : (
                <div className="flex items-center gap-1.5">
                  <button
                    className="text-[10px] text-zinc-600 hover:text-amber-400 transition-colors"
                    onClick={() => onAddAudio(seg.id)}
                    title="Add audio file to this segment"
                  >
                    + File
                  </button>
                  <span className="text-zinc-700">|</span>
                  <button
                    className="text-[10px] bg-red-700 hover:bg-red-600 text-white px-1.5 py-0.5 rounded transition-colors"
                    onClick={() => onRecordAudio(seg.id, selectedMicDevice, segDuration)}
                    disabled={!selectedMicDevice}
                    title={`Record microphone for ${segDuration.toFixed(1)}s`}
                  >
                    ● Rec
                  </button>
                </div>
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
}
