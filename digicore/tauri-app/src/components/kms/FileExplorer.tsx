import React, { useEffect, useMemo, useState } from "react";
import { Folder, FileText, ChevronRight, ChevronDown, Plus, MoreVertical, Trash2, Edit2, Star, BookOpen } from "lucide-react";
import { KmsFileSystemItemDto, KmsNoteDto } from "../../bindings";
import { filterVaultStructure } from "../../lib/kmsVaultTreeFilter";
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuTrigger,
} from "../ui/dropdown-menu";

function favoriteFromNotesList(
    notes: KmsNoteDto[] | undefined,
    notePath: string,
    fallback: boolean
): boolean {
    if (!notes?.length) return fallback;
    const hit = notes.find((n) => n.path === notePath);
    return hit?.is_favorite ?? fallback;
}

interface FileExplorerProps {
    structure: KmsFileSystemItemDto | null;
    activeNote: KmsNoteDto | null;
    /** When set, favorite labels/icons use this list so they stay in sync after toggles without refetching the tree. */
    notes?: KmsNoteDto[];
    onSelectNote: (note: KmsNoteDto) => void;
    /** Open in read-only split pane while keeping the current note in the editor. */
    onOpenNoteAsReference?: (note: KmsNoteDto) => void;
    onCreateNote: (parentPath: string) => void;
    onCreateFolder: (parentPath: string) => void;
    onRenameNote: (oldPath: string, newName: string) => Promise<void>;
    onDeleteNote: (path: string) => Promise<void>;
    onRenameFolder: (path: string) => Promise<void>;
    onDeleteFolder: (path: string) => Promise<void>;
    onMoveItem: (path: string, newParentPath: string) => Promise<void>;
    /** Toggle favorite for a note (absolute path, same as `KmsNoteDto.path`). */
    onSetNoteFavorite?: (path: string, favorite: boolean) => Promise<void>;
    filterQuery?: string;
    /** Indexed note tags (YAML); comma/space tokens match any tag (substring). */
    tagFilter?: string;
    bulkSelectMode?: boolean;
    bulkSelectedPaths?: ReadonlySet<string>;
    onToggleBulkPath?: (path: string) => void;
}

export default function FileExplorer(props: FileExplorerProps) {
    const filterActive = Boolean((props.filterQuery ?? "").trim()) || Boolean((props.tagFilter ?? "").trim());
    const displayStructure = useMemo(() => {
        if (!props.structure) return null;
        return filterVaultStructure(props.structure, props.filterQuery ?? "", props.tagFilter ?? "");
    }, [props.structure, props.filterQuery, props.tagFilter]);

    if (!props.structure) return null;

    if (!displayStructure) {
        if (filterActive) {
            return (
                <div className="px-2 py-8 text-center text-[10px] text-dc-text-muted">
                    No files or folders match this filter.
                </div>
            );
        }
        return null;
    }

    const fq = props.filterQuery ?? "";
    return (
        <div className="space-y-1">
                    {displayStructure.children?.map((item) => (
                <TreeItem
                    key={item.path}
                    item={item}
                    level={0}
                    {...props}
                    filterQuery={fq}
                    bulkSelectMode={props.bulkSelectMode}
                    bulkSelectedPaths={props.bulkSelectedPaths}
                    onToggleBulkPath={props.onToggleBulkPath}
                />
            ))}
        </div>
    );
}

interface TreeItemProps extends Omit<FileExplorerProps, "structure"> {
    item: KmsFileSystemItemDto;
    level: number;
    filterQuery?: string;
}

function TreeItem({
    item,
    level,
    activeNote,
    onSelectNote,
    onOpenNoteAsReference,
    onCreateNote,
    onCreateFolder,
    onRenameNote,
    onDeleteNote,
    onRenameFolder,
    onDeleteFolder,
    onMoveItem,
    onSetNoteFavorite,
    notes,
    filterQuery = "",
    bulkSelectMode = false,
    bulkSelectedPaths,
    onToggleBulkPath,
}: TreeItemProps) {
    const [isExpanded, setIsExpanded] = useState(false);
    const [isDragOver, setIsDragOver] = useState(false);

    const isFolder = item.item_type === "directory";
    const isActive = activeNote?.path === item.path;

    useEffect(() => {
        if (isFolder && filterQuery.trim()) {
            setIsExpanded(true);
        }
    }, [isFolder, filterQuery]);

    const handleToggle = (e: React.MouseEvent) => {
        e.stopPropagation();
        if (bulkSelectMode && !isFolder && item.note && onToggleBulkPath) {
            onToggleBulkPath(item.path);
            return;
        }
        if (isFolder) {
            setIsExpanded(!isExpanded);
        } else if (item.note) {
            onSelectNote(item.note);
        }
    };

    // Drag & Drop
    const handleDragStart = (e: React.DragEvent) => {
        e.dataTransfer.setData("kms/path", item.path);
        e.dataTransfer.setData("kms/type", item.item_type);
        e.dataTransfer.effectAllowed = "move";
    };

    const handleDragOver = (e: React.DragEvent) => {
        if (!isFolder) return;
        e.preventDefault();
        e.stopPropagation();
        setIsDragOver(true);
    };

    const handleDragLeave = () => {
        setIsDragOver(false);
    };

    const handleDrop = (e: React.DragEvent) => {
        if (!isFolder) return;
        e.preventDefault();
        e.stopPropagation();
        setIsDragOver(false);

        const sourcePath = e.dataTransfer.getData("kms/path");
        if (sourcePath && sourcePath !== item.path) {
            // Avoid dropping folder into itself or its direct parent (no-op)
            if (!item.path.startsWith(sourcePath)) {
                onMoveItem(sourcePath, item.path);
            }
        }
    };

    return (
        <div
            className={`select-none transition-all ${isDragOver ? "bg-dc-accent/10 outline outline-1 outline-dc-accent/30 rounded-md" : ""}`}
            onDragOver={handleDragOver}
            onDragLeave={handleDragLeave}
            onDrop={handleDrop}
        >
            <div
                draggable={!bulkSelectMode}
                onDragStart={handleDragStart}
                className={`flex items-center group px-2 py-1.5 rounded-md cursor-[grab] active:cursor-[grabbing] transition-colors ${isActive ? "bg-dc-bg-hover text-dc-accent font-medium" : "hover:bg-dc-bg-hover/50 text-dc-text-muted hover:text-dc-text"
                    }`}
                style={{ paddingLeft: `${level * 12 + 8}px` }}
                onClick={handleToggle}
            >
                <div className="w-4 h-4 mr-1 flex items-center justify-center opacity-60">
                    {bulkSelectMode && !isFolder && item.note ? (
                        <input
                            type="checkbox"
                            className="h-3.5 w-3.5 rounded border-dc-border accent-dc-accent"
                            checked={bulkSelectedPaths?.has(item.path) ?? false}
                            onChange={(e) => {
                                e.stopPropagation();
                                onToggleBulkPath?.(item.path);
                            }}
                            onClick={(e) => e.stopPropagation()}
                            title="Select for bulk actions"
                        />
                    ) : isFolder ? (
                        isExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />
                    ) : null}
                </div>

                {isFolder ? (
                    <Folder size={14} className="mr-2 text-dc-accent/70" />
                ) : (
                    <FileText size={14} className={`mr-2 ${isActive ? "text-dc-accent" : "text-dc-text-muted"}`} />
                )}

                <span className="text-sm truncate flex-1">{item.name}</span>

                <div className="opacity-0 group-hover:opacity-100 flex items-center gap-1 transition-opacity">
                    {isFolder && (
                        <Plus
                            size={12}
                            className="hover:text-dc-accent cursor-pointer"
                            onClick={(e: React.MouseEvent) => {
                                e.stopPropagation();
                                onCreateNote(item.path);
                            }}
                        />
                    )}
                    <DropdownMenu>
                        <DropdownMenuTrigger asChild onClick={(e: React.MouseEvent) => e.stopPropagation()}>
                            <div className="p-0.5 hover:bg-dc-bg-hover rounded-md transition-colors cursor-pointer">
                                <MoreVertical size={12} className="text-dc-text-muted hover:text-dc-accent" />
                            </div>
                        </DropdownMenuTrigger>
                        <DropdownMenuContent align="end" className="min-w-[11rem] bg-dc-bg-secondary border-dc-border shadow-2xl backdrop-blur-xl">
                            {isFolder && (
                                <>
                                    <DropdownMenuItem onClick={() => onCreateNote(item.path)} className="gap-2 text-xs">
                                        <FileText size={12} /> New Note
                                    </DropdownMenuItem>
                                    <DropdownMenuItem onClick={() => onCreateFolder(item.path)} className="gap-2 text-xs border-b border-dc-border/30 pb-2 mb-1">
                                        <Folder size={12} /> New Notebook
                                    </DropdownMenuItem>
                                </>
                            )}
                            {!isFolder && item.note && onOpenNoteAsReference && (
                                <DropdownMenuItem
                                    onClick={() => onOpenNoteAsReference(item.note!)}
                                    className="gap-2 text-xs"
                                >
                                    <BookOpen size={12} /> Open as reference
                                </DropdownMenuItem>
                            )}
                            {!isFolder && item.note && onSetNoteFavorite && (() => {
                                const n = item.note;
                                if (!n) return null;
                                const isFav = favoriteFromNotesList(notes, n.path, n.is_favorite);
                                return (
                                    <DropdownMenuItem
                                        onClick={() => void onSetNoteFavorite(n.path, !isFav)}
                                        className="gap-2 text-xs"
                                    >
                                        <Star
                                            size={12}
                                            className={isFav ? "fill-dc-accent text-dc-accent" : ""}
                                        />
                                        {isFav ? "Remove from favorites" : "Add to favorites"}
                                    </DropdownMenuItem>
                                );
                            })()}
                            <DropdownMenuItem
                                onClick={() => {
                                    if (isFolder) {
                                        onRenameFolder(item.path);
                                    } else {
                                        const newName = window.prompt("Rename note:", item.name.replace(/\.md$/i, ""));
                                        if (newName) onRenameNote(item.path, newName);
                                    }
                                }}
                                className="gap-2 text-xs"
                            >
                                <Edit2 size={12} /> Rename
                            </DropdownMenuItem>
                            <DropdownMenuItem
                                onClick={() => {
                                    if (isFolder) {
                                        onDeleteFolder(item.path);
                                    } else {
                                        onDeleteNote(item.path);
                                    }
                                }}
                                className="gap-2 text-xs text-dc-error hover:text-dc-error hover:bg-dc-error/10"
                            >
                                <Trash2 size={12} /> Delete
                            </DropdownMenuItem>
                        </DropdownMenuContent>
                    </DropdownMenu>
                </div>
            </div>

            {isFolder && isExpanded && item.children && (
                <div className="mt-0.5">
                    {item.children.map((child) => (
                        <TreeItem
                            key={child.path}
                            item={child}
                            level={level + 1}
                            activeNote={activeNote}
                            onSelectNote={onSelectNote}
                            onOpenNoteAsReference={onOpenNoteAsReference}
                            onCreateNote={onCreateNote}
                            onCreateFolder={onCreateFolder}
                            onRenameNote={onRenameNote}
                            onDeleteNote={onDeleteNote}
                            onRenameFolder={onRenameFolder}
                            onDeleteFolder={onDeleteFolder}
                            onMoveItem={onMoveItem}
                            onSetNoteFavorite={onSetNoteFavorite}
                            notes={notes}
                            filterQuery={filterQuery}
                            bulkSelectMode={bulkSelectMode}
                            bulkSelectedPaths={bulkSelectedPaths}
                            onToggleBulkPath={onToggleBulkPath}
                        />
                    ))}
                </div>
            )}
        </div>
    );
}
