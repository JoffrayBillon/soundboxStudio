import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Profile } from "../types";

interface ProfileManagerProps {
  activeProfile: Profile | null;
  onProfileLoad: (profile: Profile) => void;
}

export const ProfileManager: React.FC<ProfileManagerProps> = ({
  activeProfile,
  onProfileLoad,
}) => {
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [isCreating, setIsCreating] = useState(false);
  const [newName, setNewName] = useState("");

  const refreshProfiles = async () => {
    try {
      const result = await invoke<Profile[]>("list_profiles");
      setProfiles(result);
    } catch {
      // Directory may be empty on first run
    }
  };

  useEffect(() => {
    refreshProfiles();
  }, []);

  const handleCreate = async () => {
    if (!newName.trim()) return;
    try {
      const profile = await invoke<Profile>("create_profile", {
        name: newName.trim(),
        deviceName: "Generic MIDI",
        rows: 8,
        cols: 8,
      });
      onProfileLoad(profile);
      setNewName("");
      setIsCreating(false);
      await refreshProfiles();
    } catch (e) {
      console.error("Failed to create profile:", e);
    }
  };

  const handleLoad = async (id: string) => {
    try {
      const profile = await invoke<Profile>("load_profile", { id });
      onProfileLoad(profile);
    } catch (e) {
      console.error("Failed to load profile:", e);
    }
  };

  const handleDelete = async (id: string, e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      await invoke("delete_profile", { id });
      await refreshProfiles();
    } catch (e) {
      console.error("Failed to delete profile:", e);
    }
  };

  return (
    <div className="flex flex-col gap-3 p-4 bg-gray-900 rounded-xl border border-gray-800">
      <div className="flex items-center justify-between">
        <h3 className="text-xs font-semibold text-gray-400 uppercase tracking-wider">
          Profiles
        </h3>
        <button
          onClick={() => setIsCreating((v) => !v)}
          className="text-xs text-violet-400 hover:text-violet-300 transition-colors"
        >
          {isCreating ? "Cancel" : "+ New"}
        </button>
      </div>

      {isCreating && (
        <div className="flex gap-2">
          <input
            type="text"
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleCreate()}
            placeholder="Profile name…"
            className="flex-1 min-w-0 bg-gray-800 text-gray-200 text-xs rounded-lg px-3 py-1.5 border border-gray-700 focus:outline-none focus:ring-1 focus:ring-violet-500"
            autoFocus
          />
          <button
            onClick={handleCreate}
            className="px-3 py-1.5 bg-violet-600 hover:bg-violet-500 text-white text-xs rounded-lg transition-colors flex-shrink-0"
          >
            Create
          </button>
        </div>
      )}

      <div className="flex flex-col gap-1 max-h-52 overflow-y-auto">
        {profiles.length === 0 ? (
          <p className="text-xs text-gray-600 italic py-1">No profiles yet.</p>
        ) : (
          profiles.map((profile) => {
            const isActive = activeProfile?.id === profile.id;
            return (
              <button
                key={profile.id}
                onClick={() => handleLoad(profile.id)}
                className={[
                  "flex items-center justify-between px-3 py-2 rounded-lg text-xs transition-colors text-left group",
                  isActive
                    ? "bg-violet-600/20 text-violet-300 border border-violet-500/40"
                    : "bg-gray-800 text-gray-400 hover:bg-gray-700 border border-transparent",
                ].join(" ")}
              >
                <span className="truncate">{profile.name}</span>
                <button
                  onClick={(e) => handleDelete(profile.id, e)}
                  className="opacity-0 group-hover:opacity-100 text-red-500 hover:text-red-400 ml-2 transition-opacity flex-shrink-0"
                  title="Delete profile"
                >
                  ✕
                </button>
              </button>
            );
          })
        )}
      </div>
    </div>
  );
};
