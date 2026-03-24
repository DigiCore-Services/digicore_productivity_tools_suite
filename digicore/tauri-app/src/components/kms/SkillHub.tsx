import React, { useState, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import {
    Cpu,
    Search,
    Plus,
    RefreshCw,
    ExternalLink,
    Trash2,
    Code,
    Terminal,
    BookOpen,
    Settings,
    Layers,
    Github,
    FileCode,
    Sparkles,
    ChevronRight,
    Search as SearchIcon
} from "lucide-react";
import { getTaurpc } from "../../lib/taurpc";
import { SkillDto } from "../../bindings";
import { Button } from "../ui/button";
import { Input } from "../ui/input";
import { Badge } from "../ui/badge";
import { useToast } from "../ui/use-toast";

interface SkillHubProps {
    onSelectSkill: (skill: SkillDto) => void;
    onCreateNew: () => void;
    refreshKey?: number;
}

export default function SkillHub({ onSelectSkill, onCreateNew, refreshKey = 0 }: SkillHubProps) {
    const { toast } = useToast();
    const [skills, setSkills] = useState<SkillDto[]>([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [searchQuery, setSearchQuery] = useState("");
    const [isSyncing, setIsSyncing] = useState(false);

    const refreshSkills = async () => {
        setLoading(true);
        setError(null);
        try {
            const list = await getTaurpc().kms_list_skills();
            setSkills(list);
        } catch (err) {
            setError(String(err));
            toast({
                title: "Error Loading Skills",
                description: String(err),
                variant: "destructive",
            });
        } finally {
            setLoading(false);
        }
    };

    useEffect(() => {
        refreshSkills();
    }, [refreshKey]);

    const handleSync = async () => {
        setIsSyncing(true);
        try {
            await getTaurpc().kms_sync_skills();
            await refreshSkills();
            toast({
                title: "Skills Synchronized",
                description: "Directories scanned and search index updated.",
            });
        } catch (error) {
            toast({
                title: "Sync Failed",
                description: String(error),
                variant: "destructive",
            });
        } finally {
            setIsSyncing(false);
        }
    };

    const handleDelete = async (e: React.MouseEvent, name: string) => {
        e.stopPropagation();
        if (!window.confirm(`Are you sure you want to delete skill "${name}"?`)) return;

        try {
            await getTaurpc().kms_delete_skill(name);
            refreshSkills();
            toast({
                title: "Skill Deleted",
                description: `${name} has been removed.`,
            });
        } catch (error) {
            toast({
                title: "Delete Failed",
                description: String(error),
                variant: "destructive",
            });
        }
    };

    const filteredSkills = skills.filter(s =>
        s.metadata.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
        s.metadata.description.toLowerCase().includes(searchQuery.toLowerCase()) ||
        s.metadata.tags.some(t => t.toLowerCase().includes(searchQuery.toLowerCase()))
    );

    const containerVariants = {
        hidden: { opacity: 0 },
        visible: {
            opacity: 1,
            transition: {
                staggerChildren: 0.05
            }
        }
    };

    const itemVariants = {
        hidden: { y: 20, opacity: 0 },
        visible: {
            y: 0,
            opacity: 1,
            transition: {
                type: "spring",
                stiffness: 300,
                damping: 24
            } as any
        }
    };

    return (
        <div className="flex flex-col h-full bg-dc-bg/50 backdrop-blur-xl overflow-hidden p-8 gap-8">
            {/* Header Section */}
            <div className="flex items-center justify-between">
                <div className="space-y-1">
                    <div className="flex items-center gap-3">
                        <div className="p-2 bg-dc-accent/10 rounded-xl">
                            <Cpu className="text-dc-accent w-6 h-6" />
                        </div>
                        <h1 className="text-3xl font-bold tracking-tight bg-gradient-to-r from-dc-text to-dc-text/50 bg-clip-text text-transparent">
                            Skill Hub
                        </h1>
                    </div>
                    <p className="text-dc-text-muted text-sm max-w-lg">
                        Manage specialized AI capabilities. Skills are portable, version-controlled packages that teach agents how to perform domain-specific tasks.
                    </p>
                </div>

                <div className="flex gap-3">
                    <Button
                        variant="secondary"
                        size="sm"
                        onClick={handleSync}
                        disabled={isSyncing}
                        className="bg-dc-bg-secondary/40 border-dc-border/50 backdrop-blur-md hover:bg-dc-bg-hover transition-all group"
                    >
                        <RefreshCw className={`w-4 h-4 mr-2 ${isSyncing ? "animate-spin" : "group-hover:rotate-180 transition-transform duration-500"}`} />
                        {isSyncing ? "Syncing..." : "Sync Rules"}
                    </Button>
                    <Button
                        onClick={onCreateNew}
                        size="sm"
                        className="bg-dc-accent hover:bg-dc-accent-hover text-white shadow-lg shadow-dc-accent/20 transition-all active:scale-95"
                    >
                        <Plus className="w-4 h-4 mr-2" />
                        New Skill
                    </Button>
                </div>
            </div>

            {/* toolbar */}
            <div className="flex items-center gap-4">
                <div className="relative flex-1 group">
                    <Search className="absolute left-4 top-1/2 -translate-y-1/2 w-4 h-4 text-dc-text-muted group-focus-within:text-dc-accent transition-colors" />
                    <Input
                        placeholder="Search skills by name, description or tags..."
                        className="w-full bg-dc-bg-secondary/30 border-dc-border/30 pl-12 h-12 rounded-2xl focus:ring-dc-accent/20 focus:border-dc-accent/50 transition-all text-base"
                        value={searchQuery}
                        onChange={(e) => setSearchQuery(e.target.value)}
                    />
                </div>
                <div className="flex gap-1 p-1 bg-dc-bg-secondary/30 rounded-xl border border-dc-border/30">
                    <Button variant="ghost" size="sm" className="h-9 px-3 text-xs bg-dc-bg-hover text-dc-text">All</Button>
                    <Button variant="ghost" size="sm" className="h-9 px-3 text-xs text-dc-text-muted hover:text-dc-text">Created</Button>
                    <Button variant="ghost" size="sm" className="h-9 px-3 text-xs text-dc-text-muted hover:text-dc-text">Synced</Button>
                </div>
            </div>

            {/* Grid */}
            <div className="flex-1 overflow-y-auto pr-2 custom-scrollbar">
                {loading ? (
                    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
                        {[1, 2, 3, 4, 5, 6].map(i => (
                            <div key={i} className="h-48 bg-dc-bg-secondary/20 rounded-3xl border border-dc-border/20 animate-pulse" />
                        ))}
                    </div>
                ) : error ? (
                    <div className="h-full flex flex-col items-center justify-center text-center p-12 space-y-4">
                        <div className="w-20 h-20 bg-dc-red/10 rounded-full flex items-center justify-center border border-dc-red/30">
                            <Trash2 className="w-8 h-8 text-dc-red opacity-50" />
                        </div>
                        <div className="space-y-1">
                            <h3 className="text-xl font-semibold text-dc-red">Failed to load skills</h3>
                            <p className="text-dc-text-muted max-w-md mx-auto font-mono text-xs p-4 bg-dc-bg-secondary/50 rounded-xl border border-dc-border/30">
                                {error}
                            </p>
                        </div>
                        <Button onClick={refreshSkills} variant="secondary" className="mt-4 border-dc-accent/30 text-dc-accent">
                            <RefreshCw className="w-4 h-4 mr-2" />
                            Retry Loading
                        </Button>
                    </div>
                ) : filteredSkills.length === 0 ? (
                    <div className="h-full flex flex-col items-center justify-center text-center p-12 space-y-4">
                        <div className="w-20 h-20 bg-dc-bg-secondary/30 rounded-full flex items-center justify-center border border-dc-border/30">
                            <Sparkles className="w-8 h-8 text-dc-text-muted opacity-30" />
                        </div>
                        <div className="space-y-1">
                            <h3 className="text-xl font-semibold">No skills found</h3>
                            <p className="text-dc-text-muted max-w-xs mx-auto">
                                {searchQuery ? "Try adjusting your search query or filters." : "Start by creating your first skill or syncing with your IDE folders."}
                            </p>
                        </div>
                        {!searchQuery && (
                            <Button onClick={onCreateNew} variant="secondary" className="mt-4 border-dc-accent/30 text-dc-accent">
                                Create My First Skill
                            </Button>
                        )}
                    </div>
                ) : (
                    <motion.div
                        variants={containerVariants}
                        initial="hidden"
                        animate="visible"
                        className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-6"
                    >
                        {filteredSkills.map((skill) => (
                            <motion.div
                                key={`${skill.metadata.name}-${skill.metadata.version}`}
                                variants={itemVariants}
                                whileHover={{ y: -5, scale: 1.02 }}
                                onClick={() => onSelectSkill(skill)}
                                className="group relative bg-dc-bg-secondary/30 hover:bg-dc-bg-hover/40 border border-dc-border/40 hover:border-dc-accent/40 rounded-3xl p-6 transition-all cursor-pointer backdrop-blur-sm shadow-xl shadow-transparent hover:shadow-black/5"
                            >
                                <div className="flex flex-col h-full gap-4">
                                    <div className="flex justify-between items-start">
                                        <div className="p-3 bg-dc-accent/5 rounded-2xl group-hover:bg-dc-accent/10 transition-colors">
                                            {skill.path?.includes(".cursor") ? (
                                                <Layers className="text-dc-accent w-6 h-6" />
                                            ) : skill.path?.includes(".claude") ? (
                                                <Sparkles className="text-dc-amber-500 w-6 h-6" />
                                            ) : (
                                                <Code className="text-dc-blue-500 w-6 h-6" />
                                            )}
                                        </div>
                                        <div className="flex gap-2 opacity-0 group-hover:opacity-100 transition-opacity">
                                            <button
                                                onClick={(e) => handleDelete(e, skill.metadata.name)}
                                                className="p-2 text-dc-text-muted hover:text-dc-red hover:bg-dc-red/10 rounded-lg transition-all"
                                            >
                                                <Trash2 size={16} />
                                            </button>
                                        </div>
                                    </div>

                                    <div className="space-y-2">
                                        <div className="flex items-center gap-2">
                                            <h3 className="font-bold text-lg group-hover:text-dc-accent transition-colors truncate">
                                                {skill.metadata.name}
                                            </h3>
                                            <span className="text-[10px] font-mono font-bold px-1.5 py-0.5 bg-dc-bg-secondary rounded border border-dc-border/50 text-dc-text-muted">
                                                v{skill.metadata.version}
                                            </span>
                                        </div>
                                        <p className="text-dc-text-muted text-xs line-clamp-3 leading-relaxed min-h-[3rem]">
                                            {skill.metadata.description || "No description provided."}
                                        </p>
                                    </div>

                                    <div className="mt-auto pt-4 border-t border-dc-border/20 flex items-center justify-between">
                                        <div className="flex flex-wrap gap-1.5 overflow-hidden h-6">
                                            {skill.metadata.tags.slice(0, 2).map(tag => (
                                                <Badge key={tag} variant="secondary" className="bg-dc-bg-secondary/50 text-[10px] px-2 py-0 h-5 font-normal border-transparent">
                                                    #{tag}
                                                </Badge>
                                            ))}
                                            {skill.metadata.tags.length > 2 && (
                                                <span className="text-[10px] text-dc-text-muted flex items-center">+{skill.metadata.tags.length - 2}</span>
                                            )}
                                        </div>
                                        <div className="flex items-center text-[10px] font-bold text-dc-accent uppercase tracking-widest group/btn opacity-70 group-hover:opacity-100 transition-all">
                                            Details
                                            <ChevronRight size={12} className="ml-0.5 group-hover/btn:translate-x-1 transition-transform" />
                                        </div>
                                    </div>
                                </div>

                                {/* Sync identifier */}
                                {skill.path && (
                                    <div className="absolute -top-2 -right-2 p-1.5 bg-dc-bg rounded-full border border-dc-border shadow-md" title={skill.path}>
                                        <div className={`w-2 h-2 rounded-full ${skill.path.includes("global") ? "bg-dc-blue" : "bg-dc-green"}`} />
                                    </div>
                                )}
                            </motion.div>
                        ))}
                    </motion.div>
                )}
            </div>
        </div>
    );
}
