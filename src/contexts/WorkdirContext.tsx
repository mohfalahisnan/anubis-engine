import { invoke } from "@tauri-apps/api/core";
import {
  createContext,
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";

export type WorkdirInfo = {
  id: string;
  path: string;
  created_at: string;
  last_used: string;
  doc_count: number | null;
};

type WorkdirContextValue = {
  activeWorkdir: string | null;
  activeWorkdirId: string | null;
  knownWorkdirs: WorkdirInfo[];
  setActiveWorkdir: (path: string | null) => void;
  refreshKnownWorkdirs: () => Promise<void>;
  deleteWorkdir: (path: string) => Promise<void>;
};

const STORAGE_KEY = "anubis.activeWorkdir";

const WorkdirContext = createContext<WorkdirContextValue | null>(null);

export function WorkdirProvider({ children }: { children: ReactNode }) {
  const [activeWorkdir, setActiveWorkdirState] = useState<string | null>(() => {
    if (typeof window === "undefined") return null;
    return window.localStorage.getItem(STORAGE_KEY);
  });
  const [knownWorkdirs, setKnownWorkdirs] = useState<WorkdirInfo[]>([]);

  const refreshKnownWorkdirs = useCallback(async () => {
    try {
      const list = await invoke<WorkdirInfo[]>("list_workdirs");
      setKnownWorkdirs(list);
      // Clear stale active selection if the path no longer exists on disk.
      // Don't auto-clear if the registry has zero entries — the user may
      // have just picked a folder that hasn't been registered yet (registry
      // is populated lazily on first index call).
      if (
        activeWorkdir &&
        list.length > 0 &&
        !list.some((w) => w.path === activeWorkdir)
      ) {
        setActiveWorkdirState(null);
        window.localStorage.removeItem(STORAGE_KEY);
      }
    } catch (reason) {
      const message = String(reason);
      if (!message.toLowerCase().includes("still initialising")) {
        console.warn("list_workdirs failed:", message);
      }
    }
  }, [activeWorkdir]);

  const setActiveWorkdir = useCallback((path: string | null) => {
    setActiveWorkdirState(path);
    if (typeof window !== "undefined") {
      if (path) {
        window.localStorage.setItem(STORAGE_KEY, path);
      } else {
        window.localStorage.removeItem(STORAGE_KEY);
      }
    }
  }, []);

  const deleteWorkdir = useCallback(
    async (path: string) => {
      await invoke<void>("delete_workdir", { workdir: path });
      if (activeWorkdir === path) {
        setActiveWorkdir(null);
      }
      await refreshKnownWorkdirs();
    },
    [activeWorkdir, refreshKnownWorkdirs, setActiveWorkdir],
  );

  useEffect(() => {
    void refreshKnownWorkdirs();
  }, [refreshKnownWorkdirs]);

  const activeWorkdirId = useMemo(() => {
    if (!activeWorkdir) return null;
    const match = knownWorkdirs.find((w) => w.path === activeWorkdir);
    return match?.id ?? null;
  }, [activeWorkdir, knownWorkdirs]);

  const value = useMemo<WorkdirContextValue>(
    () => ({
      activeWorkdir,
      activeWorkdirId,
      knownWorkdirs,
      setActiveWorkdir,
      refreshKnownWorkdirs,
      deleteWorkdir,
    }),
    [
      activeWorkdir,
      activeWorkdirId,
      knownWorkdirs,
      setActiveWorkdir,
      refreshKnownWorkdirs,
      deleteWorkdir,
    ],
  );

  return <WorkdirContext.Provider value={value}>{children}</WorkdirContext.Provider>;
}

export function useWorkdir(): WorkdirContextValue {
  const ctx = useContext(WorkdirContext);
  if (!ctx) {
    throw new Error("useWorkdir must be used inside <WorkdirProvider>");
  }
  return ctx;
}
