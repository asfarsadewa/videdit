import { useRef, useEffect, useCallback } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import type { AudioTrack, Segment } from "../types";

interface VideoPlayerProps {
  src: string | null;
  currentTime: number;
  audioTracks: AudioTrack[];
  segments: Segment[];
  onTimeUpdate: (time: number) => void;
  onDurationChange: (duration: number) => void;
  onMarkIn: () => void;
  onMarkOut: () => void;
  onAddSubtitle: () => void;
  onSubtitleMarkIn: () => void;
  onSubtitleMarkOut: () => void;
}

export default function VideoPlayer({
  src,
  currentTime,
  audioTracks,
  segments,
  onTimeUpdate,
  onDurationChange,
  onMarkIn,
  onMarkOut,
  onAddSubtitle,
  onSubtitleMarkIn,
  onSubtitleMarkOut,
}: VideoPlayerProps) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const isSeeking = useRef(false);
  const currentTimeRef = useRef(currentTime);
  const audioElsRef = useRef<Map<string, HTMLAudioElement>>(new Map());

  // Keep ref updated with current time
  useEffect(() => {
    currentTimeRef.current = currentTime;
  }, [currentTime]);

  // Create/update/remove audio elements when tracks change (diff-based, no full teardown)
  useEffect(() => {
    const els = audioElsRef.current;

    // Remove tracks that no longer exist
    const activeIds = new Set(audioTracks.map((t) => t.id));
    for (const [id, el] of els) {
      if (!activeIds.has(id)) {
        el.pause();
        el.src = "";
        els.delete(id);
      }
    }

    // Add new tracks or update volume on existing ones
    for (const track of audioTracks) {
      let el = els.get(track.id);
      if (!el) {
        el = new Audio();
        el.src = convertFileSrc(track.filePath);
        el.preload = "auto";
        els.set(track.id, el);
      }
      el.volume = Math.min(track.volume, 1.0);
    }
  }, [audioTracks]);

  // Full teardown of Audio elements only on unmount
  useEffect(() => {
    return () => {
      const els = audioElsRef.current;
      for (const [, el] of els) {
        el.pause();
        el.src = "";
      }
      els.clear();
    };
  }, []);

  // Sync audio elements with video playback
  const syncAudioTracks = useCallback(
    (videoTime: number, playing: boolean) => {
      for (const track of audioTracks) {
        const segment = segments.find(s => s.id === track.segmentId);
        if (!segment) continue;
        
        const el = audioElsRef.current.get(track.id);
        if (!el) continue;

        // Audio plays only when video is within the segment's time range
        const segmentStart = segment.start;
        const segmentEnd = segment.end;
        const inSegment = videoTime >= segmentStart && videoTime < segmentEnd;
        
        if (inSegment && playing) {
          // Calculate position within segment
          const positionInSegment = videoTime - segmentStart;
          // Map to audio source time (accounting for trim)
          const audioTime = track.audioSourceStart + positionInSegment;
          
          // Check if we're within the trimmed audio range
          if (audioTime >= track.audioSourceStart && audioTime < track.audioSourceEnd) {
            if (Math.abs(el.currentTime - audioTime) > 0.3) {
              el.currentTime = audioTime;
            }
            if (el.paused) el.play().catch(() => {});
          } else {
            if (!el.paused) el.pause();
          }
        } else {
          if (!el.paused) el.pause();
        }
      }
    },
    [audioTracks, segments],
  );

  // Sync video time when currentTime changes externally (e.g. timeline click)
  useEffect(() => {
    const video = videoRef.current;
    if (!video || isSeeking.current) return;
    if (Math.abs(video.currentTime - currentTime) > 0.1) {
      video.currentTime = currentTime;
    }
  }, [currentTime]);

  const handleTimeUpdate = useCallback(() => {
    const video = videoRef.current;
    if (video && !isSeeking.current) {
      onTimeUpdate(video.currentTime);
      syncAudioTracks(video.currentTime, !video.paused);
    }
  }, [onTimeUpdate, syncAudioTracks]);

  const handleLoadedMetadata = useCallback(() => {
    const video = videoRef.current;
    if (video) {
      onDurationChange(video.duration);
    }
  }, [onDurationChange]);

  const handleSeeking = useCallback(() => {
    isSeeking.current = true;
    syncAudioTracks(0, false);
  }, [syncAudioTracks]);

  const handleSeeked = useCallback(() => {
    isSeeking.current = false;
    const video = videoRef.current;
    if (video) {
      onTimeUpdate(video.currentTime);
      syncAudioTracks(video.currentTime, !video.paused);
    }
  }, [onTimeUpdate, syncAudioTracks]);

  const handlePlay = useCallback(() => {
    const video = videoRef.current;
    if (video) syncAudioTracks(video.currentTime, true);
  }, [syncAudioTracks]);

  const handlePause = useCallback(() => {
    syncAudioTracks(0, false);
  }, [syncAudioTracks]);

  // Keyboard shortcuts
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      const video = videoRef.current;
      if (!video || !src) return;
      // Don't capture if user is in an input
      if ((e.target as HTMLElement).tagName === "INPUT") return;

      // Shift+I for subtitle mark in
      if (e.key === "I" && e.shiftKey) {
        e.preventDefault();
        onSubtitleMarkIn();
        return;
      }

      // Shift+O for subtitle mark out
      if (e.key === "O" && e.shiftKey) {
        e.preventDefault();
        // Pause the video so user can type the subtitle
        if (!video.paused) {
          video.pause();
        }
        onSubtitleMarkOut();
        return;
      }

      switch (e.key.toLowerCase()) {
        case " ":
          e.preventDefault();
          if (video.paused) video.play();
          else video.pause();
          break;
        case "arrowleft":
          e.preventDefault();
          video.currentTime = Math.max(0, video.currentTime - 5);
          break;
        case "arrowright":
          e.preventDefault();
          video.currentTime = Math.min(video.duration, video.currentTime + 5);
          break;
        case "i":
          e.preventDefault();
          onMarkIn();
          break;
        case "o":
          e.preventDefault();
          onMarkOut();
          break;
        case "s":
          e.preventDefault();
          onAddSubtitle();
          break;
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [src, onMarkIn, onMarkOut, onAddSubtitle, onSubtitleMarkIn, onSubtitleMarkOut]);

  if (!src) {
    return null;
  }

  return (
    <div className="flex-1 flex items-center justify-center bg-black min-h-0">
      <video
        ref={videoRef}
        src={src}
        controls
        className="max-w-full max-h-full"
        onTimeUpdate={handleTimeUpdate}
        onLoadedMetadata={handleLoadedMetadata}
        onSeeking={handleSeeking}
        onSeeked={handleSeeked}
        onPlay={handlePlay}
        onPause={handlePause}
      />
    </div>
  );
}
