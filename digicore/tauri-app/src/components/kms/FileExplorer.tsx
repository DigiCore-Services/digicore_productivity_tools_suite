import React, { useState } from "react";
import { Folder, FileText, ChevronRight, ChevronDown, Plus, MoreVertical, Trash2, Edit2 } from "lucide-react";
import { KmsFileSystemItemDto, KmsNoteDto } from "../../bindings";
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuTrigger,
} from "../ui/dropdown-menu";

interface FileExplorerProps {
    structure: KmsFileSystemItemDto | null;
    activeNote: KmsNoteDto | null;
    onSelectNote: (note: KmsNoteDto) => void;
    onCreateNote: (parentPath: string) => void;
    onCreateFolder: (parentPath: string) => void;
    onRenameNote: (oldPath: string, newName: string) => Promise<void>;
    onDeleteNote: (path: string) => Promise<void>;
    onRenameFolder: (path: string) => Promise<void>;
    onDeleteFolder: (path: string) => Promise<void>;
    onMoveItem: (path: string, newParentPath: string) => Promise<void>;
}

export default function FileExplorer(props: FileExplorerProps) {
    if (!props.structure) return null;

    return (
        <div className="space-y-1">
            {props.structure.children?.map((item) => (
                <TreeItem
                    key={item.path}
                    item={item}
                    level={0}
                    {...props}
                />
            ))}
        </div>
    );
}

interface TreeItemProps extends Omit<FileExplorerProps, "structure"> {
    item: KmsFileSystemItemDto;
    level: number;
}

function TreeItem({
    item,
    level,
    activeNote,
    onSelectNote,
    onCreateNote,
    onCreateFolder,
    onRenameNote,
    onDeleteNote,
    onRenameFolder,
    onDeleteFolder,
    onMoveItem,
}: TreeItemProps) {
    const [isExpanded, setIsExpanded] = useState(false);
    const [isDragOver, setIsDragOver] = useState(false);

    const isFolder = item.item_type === "directory";
    const isActive = activeNote?.path === item.path;

    const handleToggle = (e: React.MouseEvent) => {
        e.stopPropagation();
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
                draggable
                onDragStart={handleDragStart}
                className={`flex items-center group px-2 py-1.5 rounded-md cursor-[grab] active:cursor-[grabbing] transition-colors ${isActive ? "bg-dc-bg-hover text-dc-accent font-medium" : "hover:bg-dc-bg-hover/50 text-dc-text-muted hover:text-dc-text"
                    }`}
                style={{ paddingLeft: `${level * 12 + 8}px` }}
                onClick={handleToggle}
            >
                <div className="w-4 h-4 mr-1 flex items-center justify-center opacity-60">
                    {isFolder ? (
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
                        <DropdownMenuContent align="end" className="w-40 bg-dc-bg-secondary border-dc-border shadow-2xl backdrop-blur-xl">
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
                            onCreateNote={onCreateNote}
                            onCreateFolder={onCreateFolder}
                            onRenameNote={onRenameNote}
                            onDeleteNote={onDeleteNote}
                            onRenameFolder={onRenameFolder}
                            onDeleteFolder={onDeleteFolder}
                            onMoveItem={onMoveItem}
                        />
                    ))}
                </div>
            )}
        </div>
    );
}
