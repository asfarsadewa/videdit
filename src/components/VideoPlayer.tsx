import { useRef, useEffect, useCallback } from "react";

interface VideoPlayerProps {
  src: string | null;
  currentTime: number;
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

  // Keep ref updated with current time
  useEffect(() => {
    currentTimeRef.current = currentTime;
  }, [currentTime]);

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
    }
  }, [onTimeUpdate]);

  const handleLoadedMetadata = useCallback(() => {
    const video = videoRef.current;
    if (video) {
      onDurationChange(video.duration);
    }
  }, [onDurationChange]);

  const handleSeeking = useCallback(() => {
    isSeeking.current = true;
  }, []);

  const handleSeeked = useCallback(() => {
    isSeeking.current = false;
    const video = videoRef.current;
    if (video) {
      onTimeUpdate(video.currentTime);
    }
  }, [onTimeUpdate]);

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
        console.log('Shift+I pressed - marking subtitle start at', currentTimeRef.current);
        onSubtitleMarkIn();
        return;
      }

      // Shift+O for subtitle mark out
      if (e.key === "O" && e.shiftKey) {
        e.preventDefault();
        console.log('Shift+O pressed - marking subtitle end at', currentTimeRef.current);
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
          // Quick add subtitle at current time (3s duration)
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
      />
    </div>
  );
}
