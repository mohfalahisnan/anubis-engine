import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

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
};

export default function KnowledgeBrowser({ refreshKey, selectedId, onSelect }: Props) {
  const [documents, setDocuments] = useState<DocumentRow[]>([]);

  useEffect(() => {
    invoke<DocumentRow[]>("list_documents")
      .then(setDocuments)
      .catch(() => setDocuments([]));
  }, [refreshKey]);

  return (
    <section className="document-list">
      {documents.map((document) => (
        <button
          key={document.id}
          className={document.id === selectedId ? "document-row selected" : "document-row"}
          onClick={() => onSelect(document)}
        >
          <span>{document.filename}</span>
          <small>{document.format} · {document.status}</small>
        </button>
      ))}
    </section>
  );
}
