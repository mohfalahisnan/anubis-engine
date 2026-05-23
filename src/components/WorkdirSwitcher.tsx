import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { Check, ChevronDown, FolderPlus, Trash2 } from "lucide-react";
import { useState } from "react";

import { useWorkdir, WorkdirInfo } from "../contexts/WorkdirContext";
import { Button } from "./ui/button";

function basename(path: string): string {
  const trimmed = path.replace(/[\\/]+$/, "");
  const parts = trimmed.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

export default function WorkdirSwitcher() {
  const {
    activeWorkdir,
    knownWorkdirs,
    setActiveWorkdir,
    deleteWorkdir,
    refreshKnownWorkdirs,
  } = useWorkdir();
  const [isOpen, setIsOpen] = useState(false);

  async function pickFolder() {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "Pick a workdir to index into",
    });
    if (typeof selected === "string") {
      try {
        // Round-trip through the backend so we use the canonical form of the
        // path (on Windows the dialog returns `D:\foo` but canonicalize gives
        // `\\?\D:\foo` — using the canonical form in localStorage + the
        // dropdown keeps everything consistent).
        const info = await invoke<WorkdirInfo>("register_workdir", {
          workdir: selected,
        });
        setActiveWorkdir(info.path);
        await refreshKnownWorkdirs();
      } catch (reason) {
        window.alert(`Failed to register workdir: ${reason}`);
      }
    }
    setIsOpen(false);
  }

  async function handleDelete(entry: WorkdirInfo) {
    const confirmed = window.confirm(
      `Delete the Anubis index for ${entry.path}?\n\nThe folder itself stays on disk; only the index data is removed.`,
    );
    if (!confirmed) return;
    await deleteWorkdir(entry.path);
  }

  const label = activeWorkdir ? basename(activeWorkdir) : "No workdir";

  return (
    <div className="relative">
      <Button
        variant="outline"
        size="sm"
        className="w-full justify-between gap-2"
        onClick={() => setIsOpen((value) => !value)}
      >
        <span className="max-w-[220px] truncate text-sm">{label}</span>
        <ChevronDown className="size-3.5" />
      </Button>

      {isOpen && (
        <div
          className="absolute left-0 right-0 z-50 mt-1 rounded-lg border border-[var(--color-border)] bg-[var(--color-card)] p-1 shadow-lg"
          onMouseLeave={() => setIsOpen(false)}
        >
          {knownWorkdirs.length === 0 ? (
            <div className="px-3 py-4 text-center text-xs text-[var(--color-muted-foreground)]">
              No workdirs yet — pick a folder to get started.
            </div>
          ) : (
            <div className="max-h-[280px] overflow-y-auto py-1">
              {knownWorkdirs.map((entry) => {
                const isActive = entry.path === activeWorkdir;
                return (
                  <div
                    key={entry.id}
                    className="group flex items-center gap-2 rounded-md px-2 py-2 hover:bg-[var(--color-accent)]"
                  >
                    <button
                      type="button"
                      className="flex flex-1 items-start gap-2 text-left"
                      onClick={() => {
                        setActiveWorkdir(entry.path);
                        setIsOpen(false);
                      }}
                    >
                      <Check
                        className={`mt-0.5 size-3.5 shrink-0 ${
                          isActive ? "text-[var(--color-primary)]" : "text-transparent"
                        }`}
                      />
                      <div className="min-w-0 flex-1">
                        <div className="truncate text-sm font-medium">{basename(entry.path)}</div>
                        <div className="truncate text-[11px] text-[var(--color-muted-foreground)]">
                          {entry.path}
                        </div>
                        <div className="text-[10px] text-[var(--color-muted-foreground)]">
                          {entry.doc_count != null ? `${entry.doc_count} docs · ` : ""}
                          last used {new Date(entry.last_used).toLocaleString()}
                        </div>
                      </div>
                    </button>
                    <button
                      type="button"
                      className="rounded-md p-1 text-[var(--color-muted-foreground)] opacity-0 transition group-hover:opacity-100 hover:bg-[var(--color-destructive)]/10 hover:text-[var(--color-destructive)]"
                      onClick={(e) => {
                        e.stopPropagation();
                        void handleDelete(entry);
                      }}
                      title="Delete index"
                    >
                      <Trash2 className="size-3.5" />
                    </button>
                  </div>
                );
              })}
            </div>
          )}

          <div className="border-t border-[var(--color-border)] p-1">
            <button
              type="button"
              className="flex w-full items-center gap-2 rounded-md px-2 py-2 text-sm hover:bg-[var(--color-accent)]"
              onClick={() => void pickFolder()}
            >
              <FolderPlus className="size-3.5" />
              Add workdir…
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
