import React, { useState, useEffect, useCallback } from 'react';
import {
    X,
    ChevronLeft,
    ChevronRight,
    Download,
    Copy,
    Info,
    Layers,
    Search,
    Maximize2,
    Minimize2,
    Clock
} from 'lucide-react';
import { getTaurpc } from '@/lib/taurpc';
import { convertFileSrc } from '@tauri-apps/api/core';
import { save, ask } from "@tauri-apps/plugin-dialog";
import { showNativeContextMenu, NativeContextMenuAction } from "@/lib/nativeContextMenu";
import { ClipEntry } from '@/types';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Separator } from '@/components/ui/separator';
import { Tooltip } from '@/components/ui/tooltip';
import { useToast } from '@/components/ui/use-toast';

interface ImageViewerModalProps {
    isOpen: boolean;
    onClose: () => void;
    currentImage: ClipEntry | null;
    allImages: ClipEntry[];
    onNavigate: (image: ClipEntry) => void;
    onDeleteSuccess?: () => void;
}

interface OcrMetadata {
    adapter: string;
    adaptive_profile?: string;
    performance_metrics?: {
        complexity_class?: string;
        entropy?: number;
        extraction_ms?: number;
        total_timer_ms?: number;
    };
}

interface OcrBox {
    text: string;
    x: number;
    y: number;
    w: number;
    h: number;
    confidence: number;
    type: 'normal' | 'low_confidence';
}

export const ImageViewerModal: React.FC<ImageViewerModalProps> = ({
    isOpen,
    onClose,
    currentImage,
    allImages,
    onNavigate,
    onDeleteSuccess
}) => {
    const { toast } = useToast();
    const [showOcrOverlay, setShowOcrOverlay] = useState(false);
    const [ocrData, setOcrData] = useState<OcrBox[]>([]);
    const [childEntries, setChildEntries] = useState<ClipEntry[]>([]);
    const [isZoomed, setIsZoomed] = useState(false);

    const fetchOcrDetails = useCallback(async () => {
        if (!currentImage?.id) return;
        const taurpc = getTaurpc();
        try {
            const children = await taurpc.get_child_entries(currentImage.id);
            if (children) {
                setChildEntries(children as any);
                const extractedTextEntry = children.find((c: any) => c.entry_type === 'extracted_text');
                if (extractedTextEntry?.metadata) {
                    try {
                        const parsed = JSON.parse(extractedTextEntry.metadata);
                        if (parsed.performance_metrics?.details?.boxes) {
                            setOcrData(parsed.performance_metrics.details.boxes);
                        } else if (parsed.details?.boxes) {
                            setOcrData(parsed.details.boxes);
                        }
                    } catch (e) {
                        console.error("Failed to parse OCR metadata:", e);
                    }
                }
            }
        } catch (err) {
            console.error("Failed to fetch child entries:", err);
        }
    }, [currentImage?.id]);

    useEffect(() => {
        if (isOpen && currentImage) {
            fetchOcrDetails();
        }
    }, [isOpen, currentImage, fetchOcrDetails]);

    const currentIndex = allImages.findIndex(img => img.id === currentImage?.id);
    const canGoPrev = currentIndex > 0;
    const canGoNext = currentIndex < allImages.length - 1;

    const ocrTextEntry = childEntries.find(c => c.entry_type === 'extracted_text');
    const ocrMetadata = React.useMemo<OcrMetadata | null>(() => {
        if (!ocrTextEntry?.metadata) return null;
        try {
            return JSON.parse(ocrTextEntry.metadata);
        } catch (e) {
            return null;
        }
    }, [ocrTextEntry?.metadata]);

    const handlePrev = () => {
        if (canGoPrev) onNavigate(allImages[currentIndex - 1]);
    };

    const handleNext = () => {
        if (canGoNext) onNavigate(allImages[currentIndex + 1]);
    };

    const handleCopy = async () => {
        if (!currentImage?.id) return;
        try {
            await getTaurpc().copy_clipboard_image_by_id(currentImage.id);
            toast({ description: "Image copied to clipboard." });
        } catch (err) {
            toast({ description: "Failed to copy image.", variant: "destructive" });
        }
    };

    const handleSaveAs = async () => {
        if (!currentImage?.id) return;
        const path = await save({
            title: "Save Image As",
            defaultPath: "image.png",
            filters: [{ name: "PNG", extensions: ["png"] }],
        });
        if (!path) return;
        const taurpc = getTaurpc();
        try {
            await taurpc.save_clipboard_image_by_id(currentImage.id, String(path));
            toast({ description: `Image saved to ${String(path)}` });
        } catch (err) {
            toast({ description: "Failed to save image.", variant: "destructive" });
        }
    };

    const handleDelete = async () => {
        if (!currentImage?.id) return;

        const confirmed = await ask("Are you sure you want to delete this image?", {
            title: "DigiCore Text Expander",
            kind: "warning",
        });

        if (confirmed) {
            const taurpc = getTaurpc();
            try {
                await taurpc.delete_clip_entry_by_id(currentImage.id);
                toast({ description: "Image deleted." });
                onDeleteSuccess?.();
                onClose();
            } catch (err) {
                console.error("Failed to delete image:", err);
                toast({ description: "Failed to delete image.", variant: "destructive" });
            }
        }
    };

    const handleContextMenu = (e: React.MouseEvent) => {
        if (!currentImage) return;
        e.preventDefault();
        const actions: NativeContextMenuAction[] = [
            {
                id: 'save-as',
                text: 'Save Image As',
                icon: '💾',
                onClick: () => handleSaveAs()
            },
            {
                id: 'copy',
                text: 'Copy Image',
                icon: '⧉',
                onClick: () => handleCopy()
            },
            {
                id: 'delete',
                text: 'Delete',
                icon: '🗑',
                onClick: () => handleDelete()
            }
        ];
        showNativeContextMenu(e.clientX, e.clientY, actions);
    };

    if (!isOpen || !currentImage) return null;

    return (
        <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/90 backdrop-blur-sm p-4 animate-in fade-in duration-200">
            {/* Top Header */}
            <div className="absolute top-4 left-4 right-4 flex items-center justify-between z-10">
                <div className="flex flex-col">
                    <h2 className="text-white font-semibold text-lg leading-tight flex items-center">
                        {currentImage.process_name}
                        <Badge variant="secondary" className="ml-3 text-[10px] h-4 inline-flex items-center">
                            {currentImage.image_width}x{currentImage.image_height}
                        </Badge>
                    </h2>
                    <p className="text-white/60 text-xs truncate max-w-md">{currentImage.window_title}</p>
                </div>

                <div className="flex items-center space-x-2 bg-white/10 p-1.5 rounded-full backdrop-blur-md border border-white/10">
                    <div className="flex items-center space-x-1">
                        <Tooltip content={showOcrOverlay ? "Hide OCR Text" : "Show OCR Text Overlay"}>
                            <Button
                                variant={showOcrOverlay ? "default" : "ghost"}
                                size="icon"
                                onClick={() => setShowOcrOverlay(!showOcrOverlay)}
                                className="h-8 w-8"
                            >
                                <Layers className="h-4 w-4" />
                            </Button>
                        </Tooltip>

                        <Tooltip content="Copy Image">
                            <Button variant="ghost" size="icon" onClick={handleCopy} className="h-8 w-8">
                                <Copy className="h-4 w-4" />
                            </Button>
                        </Tooltip>

                        <Tooltip content="Save Image As">
                            <Button variant="ghost" size="icon" onClick={handleSaveAs} className="h-8 w-8">
                                <Download className="h-4 w-4" />
                            </Button>
                        </Tooltip>
                    </div>
                    <div className="w-px h-6 bg-white/20 mx-1" />
                    <Button variant="ghost" size="icon" className="h-9 w-9 rounded-full text-white/70 hover:text-white hover:bg-white/20" onClick={onClose}>
                        <X className="h-5 w-5" />
                    </Button>
                </div>
            </div>

            {/* Main View Area */}
            <div className="relative w-full h-full flex items-center justify-center p-12 select-none overflow-hidden">
                <Button
                    variant="ghost"
                    size="icon"
                    className="absolute left-4 h-12 w-12 z-20 rounded-full bg-black/40 text-white hover:bg-black/60 border border-white/10"
                    disabled={!canGoPrev}
                    onClick={handlePrev}
                >
                    <ChevronLeft className="h-8 w-8" />
                </Button>

                <Button
                    variant="ghost"
                    size="icon"
                    className="absolute right-4 h-12 w-12 z-20 rounded-full bg-black/40 text-white hover:bg-black/60 border border-white/10"
                    disabled={!canGoNext}
                    onClick={handleNext}
                >
                    <ChevronRight className="h-8 w-8" />
                </Button>

                <div
                    className={`relative max-w-full max-h-full transition-all duration-300 ${isZoomed ? 'scale-150 cursor-zoom-out' : 'cursor-zoom-in'}`}
                    onClick={() => setIsZoomed(!isZoomed)}
                    onContextMenu={handleContextMenu}
                >
                    {currentImage.image_path && (
                        <img
                            src={convertFileSrc(currentImage.image_path)}
                            alt="Captured"
                            className="max-w-full max-h-full object-contain shadow-2xl rounded"
                        />
                    )}

                    {showOcrOverlay && (
                        <div className="absolute inset-0 pointer-events-none">
                            {ocrData.map((box, idx) => (
                                <div
                                    key={idx}
                                    className="absolute border border-primary/40 bg-primary/10 rounded-sm flex items-center justify-center"
                                    style={{
                                        left: `${(box.x / currentImage.image_width!) * 100}%`,
                                        top: `${(box.y / currentImage.image_height!) * 100}%`,
                                        width: `${(box.w / currentImage.image_width!) * 100}%`,
                                        height: `${(box.h / currentImage.image_height!) * 100}%`,
                                    }}
                                >
                                    <span className="text-[6px] md:text-[8px] lg:text-[10px] text-primary-foreground font-medium drop-shadow-md whitespace-nowrap bg-primary/60 px-0.5 rounded">
                                        {box.text}
                                    </span>
                                </div>
                            ))}
                        </div>
                    )}
                </div>
            </div>

            {/* Bottom Bar */}
            <div className="absolute bottom-4 left-4 right-4 flex items-center justify-between z-10 text-white/60 text-xs">
                <div className="flex items-center space-x-4 bg-black/40 px-4 py-2 rounded-full border border-white/10 backdrop-blur-md">
                    <span className="flex items-center"><Clock className="h-3 w-3 mr-1.5" /> {currentImage.created_at ? new Date(parseInt(currentImage.created_at)).toLocaleString() : '-'}</span>
                    <span className="flex items-center"><Info className="h-3 w-3 mr-1.5" /> ID: {currentImage.id}</span>
                </div>

                {ocrMetadata && (
                    <div className="flex items-center space-x-4 bg-black/40 px-4 py-2 rounded-full border border-white/10 backdrop-blur-md">
                        <span className="flex items-center"><Search className="h-3 w-3 mr-1.5 text-emerald-400" /> {ocrData.length} Regions</span>
                        {ocrMetadata.performance_metrics && (
                            <span className="flex items-center"><Clock className="h-3 w-3 mr-1.5 text-emerald-400" /> {ocrMetadata.performance_metrics.extraction_ms || ocrMetadata.performance_metrics.total_timer_ms}ms</span>
                        )}
                    </div>
                )}

                <div className="flex items-center space-x-2 bg-black/40 px-4 py-2 rounded-full border border-white/10 backdrop-blur-md">
                    <span>Image {currentIndex + 1} of {allImages.length}</span>
                </div>
            </div>
        </div>
    );
};
