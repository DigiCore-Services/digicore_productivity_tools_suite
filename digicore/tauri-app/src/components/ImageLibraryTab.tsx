import React, { useState, useEffect, useMemo } from 'react';
import {
    Search,
    Grid,
    List,
    ChevronLeft,
    ChevronRight,
    FileImage,
    Download,
    Copy,
    Trash2,
    Eye,
    Info,
    Clock,
    MoreVertical,
    ExternalLink,
    Image as ImageIcon
} from 'lucide-react';
import { getTaurpc } from '@/lib/taurpc';
import { convertFileSrc } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { save, ask } from "@tauri-apps/plugin-dialog";
import { showNativeContextMenu, NativeContextMenuAction } from "@/lib/nativeContextMenu";
import { ClipEntry } from '@/types';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';
import { Skeleton } from '@/components/ui/skeleton';
import { useToast } from '@/components/ui/use-toast';

interface ImageLibraryTabProps {
    onOpenImage?: (img: ClipEntry, context: ClipEntry[]) => void;
    refreshTrigger?: number;
}

export const ImageLibraryTab: React.FC<ImageLibraryTabProps> = ({ onOpenImage, refreshTrigger = 0 }) => {
    const { toast } = useToast();
    const [images, setImages] = useState<ClipEntry[]>([]);
    const [totalCount, setTotalCount] = useState(0);
    const [page, setPage] = useState(1);
    const [pageSize, setPageSize] = useState(() => {
        return parseInt(localStorage.getItem('imageLibraryPageSize') || '25');
    });
    const [search, setSearch] = useState('');
    const [isLoading, setIsLoading] = useState(true);
    const [viewMode, setViewMode] = useState<'grid' | 'list'>('grid');

    const fetchImages = async () => {
        setIsLoading(true);
        const taurpc = getTaurpc();
        try {
            const result = await taurpc.get_image_gallery(search || null, page, pageSize);
            if (result) {
                const [data, total] = result;
                setImages(data as any);
                setTotalCount(total);
            }
        } catch (err) {
            console.error('Failed to fetch images:', err);
            toast({
                title: "Error",
                description: "Failed to load image library.",
                variant: "destructive",
            });
        } finally {
            setIsLoading(false);
        }
    };

    useEffect(() => {
        fetchImages();
    }, [page, pageSize, search, refreshTrigger]);

    useEffect(() => {
        localStorage.setItem('imageLibraryPageSize', pageSize.toString());
    }, [pageSize]);

    const totalPages = Math.ceil(totalCount / pageSize);

    const handlePageChange = (newPage: number) => {
        if (newPage >= 1 && newPage <= totalPages) {
            setPage(newPage);
        }
    };

    const handleSaveAs = async (img: ClipEntry) => {
        if (!img.id) return;
        const path = await save({
            title: "Save Image As",
            defaultPath: "image.png",
            filters: [{ name: "PNG", extensions: ["png"] }],
        });
        if (!path) return;
        const taurpc = getTaurpc();
        try {
            await taurpc.save_clipboard_image_by_id(img.id, String(path));
            toast({ description: `Image saved to ${String(path)}` });
        } catch (err) {
            toast({ description: "Failed to save image.", variant: "destructive" });
        }
    };

    const handleOpenViewer = (img: ClipEntry) => {
        onOpenImage?.(img, images);
    };

    const handleContextMenu = (e: React.MouseEvent, img: ClipEntry) => {
        e.preventDefault();
        const actions: NativeContextMenuAction[] = [
            {
                id: 'open',
                text: 'Open Image',
                icon: '👁',
                onClick: () => handleOpenViewer(img)
            },
            {
                id: 'save-as',
                text: 'Save Image As',
                icon: '💾',
                onClick: () => handleSaveAs(img)
            },
            {
                id: 'copy',
                text: 'Copy Image',
                icon: '⧉',
                onClick: () => handleCopy(img.id!)
            },
            {
                id: 'delete',
                text: 'Delete',
                icon: '🗑',
                onClick: () => handleDelete(img.id!)
            }
        ];
        showNativeContextMenu(e.clientX, e.clientY, actions);
    };

    const handleCopy = async (id: number) => {
        const taurpc = getTaurpc();
        try {
            await taurpc.copy_clipboard_image_by_id(id);
            toast({ description: "Image copied to clipboard." });
        } catch (err) {
            toast({ description: "Failed to copy image.", variant: "destructive" });
        }
    };

    const handleDelete = async (id: number) => {
        const confirmed = await ask("Are you sure you want to delete this image?", {
            title: "DigiCore Text Expander",
            kind: "warning",
        });

        if (confirmed) {
            const taurpc = getTaurpc();
            try {
                await taurpc.delete_clip_entry_by_id(id);
                fetchImages();
                toast({ description: "Image deleted." });
            } catch (err) {
                console.error("Failed to delete image:", err);
                toast({ description: "Failed to delete image.", variant: "destructive" });
            }
        }
    };

    const formatFileSize = (bytes?: number | null) => {
        if (!bytes) return '0 B';
        const k = 1024;
        const sizes = ['B', 'KB', 'MB', 'GB'];
        const i = Math.floor(Math.log(bytes) / Math.log(k));
        return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
    };

    return (
        <div className="flex flex-col h-full bg-background p-4 space-y-4 overflow-hidden">
            {/* Header & Controls */}
            <div className="flex items-center justify-between space-x-4">
                <div className="relative flex-1 max-w-md">
                    <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
                    <Input
                        placeholder="Search images by source or window title..."
                        className="pl-8"
                        value={search}
                        onChange={(e: React.ChangeEvent<HTMLInputElement>) => {
                            setSearch(e.target.value);
                            setPage(1); // Reset to page 1 on search
                        }}
                    />
                </div>

                <div className="flex items-center space-x-2">
                    <Badge variant="outline" className="h-9 px-3 font-normal">
                        {totalCount} Images
                    </Badge>

                    <div className="flex border rounded-md overflow-hidden">
                        <Button
                            variant={viewMode === 'grid' ? 'secondary' : 'ghost'}
                            size="icon"
                            className="rounded-none h-9 w-9"
                            onClick={() => setViewMode('grid')}
                        >
                            <Grid className="h-4 w-4" />
                        </Button>
                        <Button
                            variant={viewMode === 'list' ? 'secondary' : 'ghost'}
                            size="icon"
                            className="rounded-none h-9 w-9"
                            onClick={() => setViewMode('list')}
                        >
                            <List className="h-4 w-4" />
                        </Button>
                    </div>

                    <select
                        className="h-9 px-3 rounded-md bg-secondary text-sm border-none focus:ring-1 focus:ring-primary"
                        value={pageSize}
                        onChange={(e) => {
                            const val = parseInt(e.target.value);
                            setPageSize(val);
                            setPage(1);
                            localStorage.setItem('imageLibraryPageSize', val.toString());
                        }}
                    >
                        <option value={10}>10 / page</option>
                        <option value={25}>25 / page</option>
                        <option value={50}>50 / page</option>
                        <option value={100}>100 / page</option>
                    </select>
                </div>
            </div>

            {/* Main Content Area */}
            <div className="flex-1 overflow-y-auto min-h-0 custom-scrollbar pr-2">
                {isLoading ? (
                    <div className={`grid gap-4 ${viewMode === 'grid' ? 'grid-cols-2 md:grid-cols-3 lg:grid-cols-5' : 'grid-cols-1'}`}>
                        {[...Array(pageSize)].map((_, i) => (
                            <Skeleton key={i} className={viewMode === 'grid' ? "aspect-[3/2] rounded-lg" : "h-20 w-full rounded-lg"} />
                        ))}
                    </div>
                ) : images.length === 0 ? (
                    <div className="flex flex-col items-center justify-center h-full text-muted-foreground space-y-4">
                        <FileImage className="h-16 w-16 opacity-20" />
                        <p>No images found in your library.</p>
                    </div>
                ) : viewMode === 'grid' ? (
                    <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 gap-4">
                        {images.map((img) => (
                            <div
                                key={img.id}
                                className="group relative aspect-[3/2] rounded-lg border bg-card overflow-hidden hover:ring-2 hover:ring-primary/50 transition-all cursor-pointer"
                                onClick={() => onOpenImage?.(img, images)}
                                onContextMenu={(e) => handleContextMenu(e, img)}
                            >
                                {img.thumb_path ? (
                                    <img
                                        src={convertFileSrc(img.thumb_path)}
                                        alt={img.content}
                                        className="w-full h-full object-cover"
                                    />
                                ) : (
                                    <div className="w-full h-full flex items-center justify-center bg-muted">
                                        <FileImage className="h-8 w-8 text-muted-foreground opacity-50" />
                                    </div>
                                )}

                                {/* Overlay Info */}
                                <div className="absolute inset-x-0 bottom-0 p-2 bg-gradient-to-t from-black/80 to-transparent translate-y-2 group-hover:translate-y-0 transition-transform opacity-0 group-hover:opacity-100">
                                    <p className="text-[10px] text-white font-medium truncate">{img.process_name}</p>
                                    <p className="text-[9px] text-white/70 truncate">{img.window_title}</p>
                                </div>

                                {/* Context Actions */}
                                <div className="absolute top-2 right-2 opacity-0 group-hover:opacity-100 transition-opacity">
                                    <div className="absolute top-2 right-2">
                                        <Button
                                            variant="secondary"
                                            size="icon"
                                            className="h-8 w-8 bg-background/80 backdrop-blur-sm opacity-0 group-hover:opacity-100 transition-opacity"
                                            onClick={(e: React.MouseEvent) => {
                                                e.stopPropagation();
                                                const menu = e.currentTarget.nextElementSibling as HTMLElement;
                                                if (menu) menu.classList.toggle('hidden');
                                            }}
                                        >
                                            <MoreVertical className="h-4 w-4" />
                                        </Button>
                                        <div className="hidden absolute right-0 mt-1 w-48 rounded-md shadow-lg bg-popover text-popover-foreground border z-50 py-1" onClick={(e) => e.stopPropagation()}>
                                            <button className="w-full text-left px-4 py-2 text-sm hover:bg-accent flex items-center" onClick={() => { onOpenImage?.(img, images); }}>
                                                <Eye className="mr-2 h-4 w-4" /> View Details
                                            </button>
                                            <button className="w-full text-left px-4 py-2 text-sm hover:bg-accent flex items-center" onClick={() => { img.id && handleCopy(img.id); }}>
                                                <Copy className="mr-2 h-4 w-4" /> Copy Image
                                            </button>
                                            <button className="w-full text-left px-4 py-2 text-sm hover:bg-accent flex items-center" onClick={() => { img.id && handleSaveAs(img); }}>
                                                <Download className="mr-2 h-4 w-4" /> Save As
                                            </button>
                                            <button className="w-full text-left px-4 py-2 text-sm hover:bg-accent flex items-center text-destructive" onClick={() => { img.id && handleDelete(img.id); }}>
                                                <Trash2 className="mr-2 h-4 w-4" /> Delete
                                            </button>
                                        </div>
                                    </div>
                                </div>
                            </div>
                        ))}
                    </div>
                ) : (
                    <div className="space-y-2">
                        {images.map((img) => (
                            <div
                                key={img.id}
                                className="flex items-center p-3 rounded-lg border bg-card hover:bg-accent/10 transition-colors group cursor-pointer"
                                onClick={() => onOpenImage?.(img, images)}
                                onContextMenu={(e) => handleContextMenu(e, img)}
                            >
                                <div className="h-12 w-20 rounded border bg-muted overflow-hidden mr-4">
                                    {img.thumb_path ? (
                                        <img
                                            src={convertFileSrc(img.thumb_path)}
                                            alt=""
                                            className="w-full h-full object-cover"
                                        />
                                    ) : <div className="w-full h-full flex items-center justify-center"><FileImage className="h-4 w-4" /></div>}
                                </div>

                                <div className="flex-1 min-w-0">
                                    <div className="flex items-center space-x-2">
                                        <span className="font-medium text-sm truncate">{img.process_name}</span>
                                        <Badge variant="outline" className="text-[10px] py-0 h-4 font-normal">
                                            {img.image_width}x{img.image_height}
                                        </Badge>
                                    </div>
                                    <p className="text-xs text-muted-foreground truncate">{img.window_title}</p>
                                </div>

                                <div className="flex items-center space-x-4 text-muted-foreground ml-4">
                                    <div className="flex flex-col items-end whitespace-nowrap">
                                        <span className="text-[11px] font-medium flex items-center">
                                            <Info className="h-3 w-3 mr-1" /> {formatFileSize(img.image_bytes)}
                                        </span>
                                        <span className="text-[10px] flex items-center">
                                            <Clock className="h-3 w-3 mr-1" /> {img.created_at ? new Date(parseInt(img.created_at)).toLocaleString() : '-'}
                                        </span>
                                    </div>

                                    <div className="flex items-center space-x-1 opacity-0 group-hover:opacity-100 transition-opacity">
                                        <Button variant="ghost" size="icon" className="h-8 w-8" onClick={(e: React.MouseEvent) => { e.stopPropagation(); img.id && handleCopy(img.id); }}>
                                            <Copy className="h-4 w-4" />
                                        </Button>
                                        <Button variant="ghost" size="icon" className="h-8 w-8" onClick={(e: React.MouseEvent) => { e.stopPropagation(); img.id && handleSaveAs(img); }}>
                                            <Download className="h-4 w-4" />
                                        </Button>
                                        <Button variant="ghost" size="icon" className="h-8 w-8 text-destructive hover:text-destructive" onClick={(e: React.MouseEvent) => { e.stopPropagation(); img.id && handleDelete(img.id); }}>
                                            <Trash2 className="h-4 w-4" />
                                        </Button>
                                    </div>
                                </div>
                            </div>
                        ))}
                    </div>
                )}
            </div>

            {/* Pagination Footer */}
            {totalPages > 1 && (
                <div className="flex items-center justify-between pt-4 border-t">
                    <div className="text-xs text-muted-foreground">
                        Showing <span className="font-medium">{(page - 1) * pageSize + 1}</span> to <span className="font-medium">{Math.min(page * pageSize, totalCount)}</span> of <span className="font-medium">{totalCount}</span> results
                    </div>

                    <div className="flex items-center space-x-2">
                        <Button
                            variant="secondary"
                            size="sm"
                            onClick={() => handlePageChange(page - 1)}
                            disabled={page === 1}
                        >
                            <ChevronLeft className="h-4 w-4 mr-1" /> Previous
                        </Button>

                        <div className="flex items-center space-x-1 px-4">
                            <Input
                                className="w-12 h-8 text-center"
                                value={page}
                                onChange={(e: React.ChangeEvent<HTMLInputElement>) => {
                                    const val = parseInt(e.target.value);
                                    if (!isNaN(val)) handlePageChange(val);
                                }}
                            />
                            <span className="text-sm text-muted-foreground">of {totalPages}</span>
                        </div>

                        <Button
                            variant="secondary"
                            size="sm"
                            onClick={() => handlePageChange(page + 1)}
                            disabled={page === totalPages}
                        >
                            Next <ChevronRight className="h-4 w-4 ml-1" />
                        </Button>
                    </div>
                </div>
            )}
        </div>
    );
};
