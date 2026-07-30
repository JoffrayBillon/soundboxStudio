import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

/// Custom window chrome (minimize / maximize-restore / close), replacing
/// the native OS title bar entirely — `decorations: false` in
/// `tauri.conf.json` removes it on every platform, so these buttons are now
/// the only way to minimize/maximize/close the app. Meant to be rendered
/// inside the app's own header, which carries `data-tauri-drag-region` so
/// the window can still be dragged by its title area like a normal window.
export function WindowControls() {
  const [isMaximized, setIsMaximized] = useState(false);

  useEffect(() => {
    const win = getCurrentWindow();
    win.isMaximized().then(setIsMaximized);
    // Toggling maximize/restore (or double-clicking the drag region, or an
    // OS-level snap) all end up firing a resize — re-check the actual state
    // afterwards rather than trying to track it optimistically ourselves.
    const unlistenPromise = win.onResized(() => {
      win.isMaximized().then(setIsMaximized);
    });
    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, []);

  return (
    <div className="flex items-center h-full">
      <button
        className="w-11 h-full flex items-center justify-center text-gray-400 hover:bg-gray-800 hover:text-gray-100"
        onClick={() => getCurrentWindow().minimize()}
        aria-label="Minimize"
        title="Minimize"
      >
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
          <rect y="4.5" width="10" height="1" fill="currentColor" />
        </svg>
      </button>
      <button
        className="w-11 h-full flex items-center justify-center text-gray-400 hover:bg-gray-800 hover:text-gray-100"
        onClick={() => getCurrentWindow().toggleMaximize()}
        aria-label={isMaximized ? "Restore" : "Maximize"}
        title={isMaximized ? "Restore" : "Maximize"}
      >
        {isMaximized ? (
          <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
            <rect x="2.5" y="0.5" width="6" height="6" fill="none" stroke="currentColor" />
            <rect x="0.5" y="2.5" width="6" height="6" fill="#111827" stroke="currentColor" />
          </svg>
        ) : (
          <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
            <rect x="0.5" y="0.5" width="9" height="9" fill="none" stroke="currentColor" />
          </svg>
        )}
      </button>
      <button
        className="w-11 h-full flex items-center justify-center text-gray-400 hover:bg-red-600 hover:text-white"
        onClick={() => getCurrentWindow().close()}
        aria-label="Close"
        title="Close"
      >
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
          <line x1="0.5" y1="0.5" x2="9.5" y2="9.5" stroke="currentColor" strokeWidth="1.2" />
          <line x1="9.5" y1="0.5" x2="0.5" y2="9.5" stroke="currentColor" strokeWidth="1.2" />
        </svg>
      </button>
    </div>
  );
}
