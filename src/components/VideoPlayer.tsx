import { useRef, useEffect, useCallback } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import type { AudioTrack } from "../types";

interface VideoPlayerProps {
  src: string | null;
  currentTime: number;
  audioTracks: AudioTrack[];
  onTimeUpdate: (time: number) => void;
  onDurationChange: (duration: number) => void;
  onMarkIn: () => void;
  onMarkOut: () => void;
  onAddSubtitle: () => void;
  onSubtitleMarkIn: () => void;
  onSubtitleMarkOut: () => void;
  onAddAudio: () => void;
}

export default function VideoPlayer({
  src,
  currentTime,
  audioTracks,
  onTimeUpdate,
  onDurationChange,
  onMarkIn,
  onMarkOut,
  onAddSubtitle,
  onSubtitleMarkIn,
  onSubtitleMarkOut,
  onAddAudio,
}: VideoPlayerProps) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const isSeeking = useRef(false);
  const currentTimeRef = useRef(currentTime);
  const audioElsRef = useRef<Map<string, HTMLAudioElement>>(new Map());

  // Keep ref updated with current time
  useEffect(() => {
    currentTimeRef.current = currentTime;
  }, [currentTime]);

  // Create/update/remove audio elements when tracks change
  useEffect(() => {
    const els = audioElsRef.current;

    const activeIds = new Set(audioTracks.map((t) => t.id));
    for (const [id, el] of els) {
      if (!activeIds.has(id)) {
        el.pause();
        el.src = "";
        els.delete(id);
      }
    }

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

    return () => {
      for (const [, el] of els) {
        el.pause();
        el.src = "";
      }
      els.clear();
    };
  }, [audioTracks]);

  // Sync audio elements with video playback
  const syncAudioTracks = useCallback(
    (videoTime: number, playing: boolean) => {
      for (const track of audioTracks) {
        const el = audioElsRef.current.get(track.id);
        if (!el) continue;

        const inRange = videoTime >= track.start && videoTime < track.end;
        if (inRange && playing) {
          const targetTime = videoTime - track.start;
          if (Math.abs(el.currentTime - targetTime) > 0.3) {
            el.currentTime = targetTime;
          }
          if (el.paused) el.play().catch(() => {});
        } else {
          if (!el.paused) el.pause();
          if (inRange) {
            el.currentTime = videoTime - track.start;
          }
        }
      }
    },
    [audioTracks],
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
        case "d":
          e.preventDefault();
          onAddAudio();
          break;
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [src, onMarkIn, onMarkOut, onAddSubtitle, onSubtitleMarkIn, onSubtitleMarkOut, onAddAudio]);

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
