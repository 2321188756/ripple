import { memo } from "react";
import { Pencil, Check, X } from "lucide-react";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import { FileCfg } from "./file-cfg";
import type { DocMeta } from "./DocPropsDialog";
import type { Document } from "@/types";

interface DocCardProps {
  doc: Document;
  kbId: string;
  inSelectMode: boolean;
  isSelected: boolean;
  isRenaming: boolean;
  renameVal: string;
  onOpen: (docId: string, docName: string, ext: string) => void;
  onToggleSelect: (id: string) => void;
  onContextMenu: (e: React.MouseEvent, doc: DocMeta) => void;
  onRenameStart: (doc: Document) => void;
  onRenameValChange: (v: string) => void;
  onRenameCommit: (docId: string, kbId: string) => void;
  onRenameCancel: () => void;
  onDelete: (docId: string, docName: string, kbId: string) => void;
}

/** 单个文档卡片（memo 缓存）。从 KnowledgePanel 拆出。 */
export const DocCard = memo(function DocCard({
  doc, kbId, inSelectMode, isSelected, isRenaming, renameVal,
  onOpen, onToggleSelect, onContextMenu, onRenameStart, onRenameValChange, onRenameCommit, onRenameCancel, onDelete,
}: DocCardProps) {
  const ext = doc.file_type || "txt";
  const cfg = FileCfg(ext);
  const Icon = cfg.icon;

  return (
    <div
      className={cn(
        "group relative bg-muted/20 border rounded-lg px-3 py-2.5 transition-all duration-150",
        inSelectMode
          ? isSelected
            ? "border-primary/60 bg-primary/5 cursor-pointer"
            : "border-border hover:border-primary/30 cursor-pointer"
          : "border-border hover:border-primary/40 hover:bg-accent/30 hover:shadow-sm cursor-pointer",
      )}
      onClick={() => {
        if (inSelectMode) { onToggleSelect(doc.id); return; }
        onOpen(doc.id, doc.file_name, ext);
      }}
      onContextMenu={(e) => {
        if (inSelectMode) return;
        onContextMenu(e, {
          id: doc.id, name: doc.file_name, ext, kbId,
          docType: doc.file_type, status: doc.status, createdAt: doc.created_at,
        });
      }}
    >
      {/* Select checkbox */}
      {inSelectMode && (
        <div className="absolute top-1 left-1 z-10">
          <div className={cn(
            "w-4 h-4 rounded border-2 flex items-center justify-center transition-colors",
            isSelected ? "bg-primary border-primary" : "bg-background border-muted-foreground/40",
          )}>
            {isSelected && <Check className="w-3 h-3 text-primary-foreground" />}
          </div>
        </div>
      )}
      {/* top row: icon + status */}
      <div className="flex items-center justify-between mb-1.5">
        <Icon className={`w-4 h-4 ${cfg.color}`} />
        <Badge variant={doc.status === "ready" ? "success" : doc.status === "error" ? "destructive" : "warning"}
          className="text-[9px] px-1 py-0 leading-none">{doc.status}</Badge>
      </div>
      {/* file name (editable) */}
      {isRenaming ? (
        <div className="flex items-center gap-1 mb-1" onClick={(e) => e.stopPropagation()}>
          <Input
            autoFocus
            value={renameVal}
            onChange={(e) => onRenameValChange(e.target.value)}
            onBlur={() => onRenameCommit(doc.id, kbId)}
            onKeyDown={(e) => {
              if (e.key === "Enter") onRenameCommit(doc.id, kbId);
              if (e.key === "Escape") onRenameCancel();
            }}
            className="h-6 text-xs px-1"
          />
        </div>
      ) : (
        <div className="flex items-center gap-1 mb-1 group/name">
          <div
            className="text-xs font-medium truncate flex-1"
            title={doc.file_name}
          >{doc.file_name}</div>
          {!inSelectMode && (
            <Pencil
              className="w-3 h-3 shrink-0 text-muted-foreground opacity-0 group-hover/name:opacity-100 cursor-pointer hover:text-primary transition-all"
              onClick={(e) => { e.stopPropagation(); onRenameStart(doc); }}
            />
          )}
        </div>
      )}
      {/* bottom: type + date */}
      <div className="flex items-center justify-between text-[10px] text-muted-foreground">
        <span className="bg-muted px-1 rounded">{cfg.label}</span>
        <span>{doc.created_at?.slice(0, 10)}</span>
      </div>

      {/* Delete button (hidden in select mode) */}
      {!inSelectMode && (
        <button
          className="absolute top-1 right-1 w-5 h-5 rounded-full bg-background/80 opacity-0 group-hover:opacity-100
                     flex items-center justify-center hover:bg-destructive/10 hover:text-destructive transition-all"
          onClick={(e) => { e.stopPropagation(); onDelete(doc.id, doc.file_name, kbId); }}
        >
          <X className="w-2.5 h-2.5" />
        </button>
      )}
    </div>
  );
});
