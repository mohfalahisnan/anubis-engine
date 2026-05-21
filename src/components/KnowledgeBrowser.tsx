import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import {
  FileCode2,
  FileImage,
  FileSpreadsheet,
  FileText,
  FileType,
  FileVideo,
  Files,
  Loader2,
  RefreshCw,
  Search,
} from "lucide-react";
import { cn } from "../lib/utils";
import { Input } from "./ui/input";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";

export type DocumentRow = {
  id: string;
  filename: string;
  path: string;
  format: string;
  status: string;
  error_msg?: string | null;
};

type Props = {
  refreshKey: number;
  selectedId: string | null;
  onSelect: (document: DocumentRow) => void;
  onReindex?: (document: DocumentRow) => Promise<void> | void;
};

const formatIcon: Record<string, React.ComponentType<{ className?: string }>> = {
  md: FileText,
  text: FileText,
  pdf: FileType,
  docx: FileCode2,
  xlsx: FileSpreadsheet,
  image: FileImage,
  video: FileVideo,
};

export default function KnowledgeBrowser({
  refreshKey,
  selectedId,
  onSelect,
  onReindex,
}: Props) {
  const [documents, setDocuments] = useState<DocumentRow[]>([]);
  const [filter, setFilter] = useState("");
  const [reindexingId, setReindexingId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<DocumentRow[]>("list_documents")
      .then(setDocuments)
      .catch(() => setDocuments([]));
  }, [refreshKey]);

  async function reindexDocument(document: DocumentRow) {
    if (!onReindex || reindexingId) return;
    setReindexingId(document.id);
    setError(null);
    try {
      await onReindex(document);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setReindexingId(null);
    }
  }

  const filtered = filter
    ? documents.filter((doc) =>
        doc.filename.toLowerCase().includes(filter.toLowerCase()),
      )
    : documents;

  if (documents.length === 0) {
    return (
      <div className="flex flex-1 flex-col items-center justify-center gap-2 rounded-lg border border-dashed border-[var(--color-border)] py-8 text-center">
        <Files className="size-6 text-[var(--color-muted-foreground)]" />
        <p className="text-xs text-[var(--color-muted-foreground)]">
          No documents indexed yet
        </p>
      </div>
    );
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2">
      <div className="flex items-center gap-2 text-xs font-medium text-[var(--color-muted-foreground)]">
        <Files className="size-3.5" />
        <span>Indexed documents</span>
        <span className="ml-auto rounded-full bg-[var(--color-accent)] px-2 py-0.5 font-mono text-[10px]">
          {documents.length}
        </span>
      </div>

      <div className="relative">
        <Search className="pointer-events-none absolute left-2.5 top-1/2 size-3.5 -translate-y-1/2 text-[var(--color-muted-foreground)]" />
        <Input
          value={filter}
          onChange={(event) => setFilter(event.target.value)}
          placeholder="Filter documents"
          className="h-8 pl-8 text-xs"
        />
      </div>

      <div className="flex min-h-0 flex-1 flex-col gap-1 overflow-y-auto pr-1">
        {filtered.map((document) => {
          const Icon = formatIcon[document.format] ?? FileText;
          const isSelected = document.id === selectedId;
          const isReindexing = reindexingId === document.id;
          return (
            <div
              key={document.id}
              className={cn(
                "group flex w-full items-start gap-1 rounded-md border p-1 transition-colors",
                isSelected
                  ? "border-[var(--color-primary)]/50 bg-[var(--color-accent)]"
                  : "border-transparent hover:bg-[var(--color-accent)]/70",
              )}
            >
              <button
                type="button"
                onClick={() => onSelect(document)}
                className="flex min-w-0 flex-1 items-start gap-2 rounded-[calc(var(--radius-md)-2px)] p-1 text-left"
              >
                <Icon className="mt-0.5 size-4 shrink-0 text-[var(--color-muted-foreground)] group-hover:text-[var(--color-foreground)]" />
                <div className="min-w-0 flex-1">
                  <div className="truncate text-xs font-medium">
                    {document.filename}
                  </div>
                  <div className="mt-0.5 flex items-center gap-1.5">
                    <Badge
                      variant={
                        document.status === "indexed"
                          ? "success"
                          : document.status === "error"
                            ? "destructive"
                            : ("secondary" as any)
                      }
                      className="text-[9px]"
                    >
                      {document.format}
                    </Badge>
                    <span className="truncate text-[10px] text-[var(--color-muted-foreground)]">
                      {document.status}
                    </span>
                  </div>
                </div>
              </button>
              {onReindex ? (
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  className="mt-0.5 size-7 opacity-70 hover:opacity-100"
                  title={`Reindex ${document.filename}`}
                  disabled={Boolean(reindexingId)}
                  onClick={() => void reindexDocument(document)}
                >
                  {isReindexing ? (
                    <Loader2 className="size-3.5 animate-spin" />
                  ) : (
                    <RefreshCw className="size-3.5" />
                  )}
                </Button>
              ) : null}
            </div>
          );
        })}
      </div>

      {error ? (
        <div className="rounded-md border border-[var(--color-destructive)]/30 bg-[var(--color-destructive)]/10 px-2 py-1.5 text-[11px] text-[var(--color-destructive)]">
          {error}
        </div>
      ) : null}
    </div>
  );
}
