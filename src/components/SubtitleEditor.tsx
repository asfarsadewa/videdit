import { useState, useEffect, useRef, useCallback } from "react";
import type { Subtitle } from "../types";
import { formatTime } from "../utils/format";

interface SubtitleEditorProps {
  subtitle: Subtitle | null;
  onSave: (text: string) => void;
  onCancel: () => void;
}

export default function SubtitleEditor({
  subtitle,
  onSave,
  onCancel,
}: SubtitleEditorProps) {
  const [text, setText] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (subtitle) {
      setText(subtitle.text);
      inputRef.current?.focus();
    }
  }, [subtitle]);

  const handleSave = useCallback(() => {
    if (text.trim()) {
      onSave(text.trim());
      setText("");
    }
  }, [text, onSave]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter") {
        e.preventDefault();
        handleSave();
      } else if (e.key === "Escape") {
        e.preventDefault();
        onCancel();
      }
    },
    [handleSave, onCancel],
  );

  if (!subtitle) return null;

  return (
    <div className="flex items-center gap-2 px-3 py-2 bg-zinc-800 border-t border-zinc-700">
      <span className="text-xs text-zinc-500 font-mono whitespace-nowrap">
        {formatTime(subtitle.start)} - {formatTime(subtitle.end)}
      </span>
      <input
        ref={inputRef}
        type="text"
        value={text}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={handleKeyDown}
        placeholder="Type subtitle text... (Enter to save, Esc to cancel)"
        className="flex-1 bg-zinc-900 border border-zinc-700 rounded px-2 py-1 text-sm text-zinc-100 placeholder-zinc-600 focus:outline-none focus:border-emerald-500"
      />
      <div className="flex items-center gap-1">
        <button
          onClick={handleSave}
          className="px-2 py-1 bg-emerald-600 hover:bg-emerald-500 rounded text-xs text-white font-medium"
        >
          Save
        </button>
        <button
          onClick={onCancel}
          className="px-2 py-1 bg-zinc-700 hover:bg-zinc-600 rounded text-xs text-zinc-300"
        >
          Cancel
        </button>
      </div>
    </div>
  );
}
