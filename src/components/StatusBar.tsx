import React from "react";

interface StatusBarProps {
  isConnected: boolean;
  deviceName: string | null;
  profileName: string | null;
}

export const StatusBar: React.FC<StatusBarProps> = ({
  isConnected,
  deviceName,
  profileName,
}) => {
  return (
    <footer className="flex items-center justify-between px-4 py-1.5 bg-gray-950 border-t border-gray-800 text-xs text-gray-500 flex-shrink-0">
      <div className="flex items-center gap-3">
        <span
          className={[
            "flex items-center gap-1.5",
            isConnected ? "text-emerald-400" : "text-gray-600",
          ].join(" ")}
        >
          <span
            className={[
              "w-1.5 h-1.5 rounded-full",
              isConnected ? "bg-emerald-400 animate-pulse" : "bg-gray-600",
            ].join(" ")}
          />
          {isConnected ? (deviceName ?? "Connected") : "No device"}
        </span>
      </div>
      <div className="flex items-center gap-3">
        {profileName && (
          <span className="text-gray-600">
            Profile:{" "}
            <span className="text-gray-400">{profileName}</span>
          </span>
        )}
        <span className="text-gray-700">Soundbox Studio v0.1.0</span>
      </div>
    </footer>
  );
};
