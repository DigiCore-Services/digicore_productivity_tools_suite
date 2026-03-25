import React, { useState, useEffect } from "react";
import {
    X,
    Save,
    ArrowLeft,
    Terminal,
    Book,
    Layers,
    Github,
    FileCode,
    Sparkles,
    Trash2,
    ChevronRight,
    Search,
    Wand2,
    CheckCircle2,
    Plus,
    File,
    FolderPlus,
    ExternalLink,
    RefreshCw,
    AlertTriangle
} from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { getTaurpc } from "../../lib/taurpc";
import { SkillDto, SkillMetadataDto } from "../../bindings";
import { Button } from "../ui/button";
import { Input } from "../ui/input";
import { Badge } from "../ui/badge";
import { Label } from "../ui/label";
import { useToast } from "../ui/use-toast";
import { motion, AnimatePresence } from "framer-motion";

const SUGGESTED_VERSIONS = ["1.0.0", "0.1.0", "0.0.1", "2.0.0-alpha", "1.1.0"];

interface SkillEditorProps {
    skill: SkillDto | null; // null means create new
    onClose: () => void;
    onSaved: () => void;
}

interface ResourceTreeNode {
    name: string;
    relPath: string;
    type: string;
    children: ResourceTreeNode[];
}

function buildResourceTree(resources: { name: string, type: string, rel_path: string }[]): ResourceTreeNode[] {
    const root: ResourceTreeNode[] = [];
    const pathMap = new Map<string, ResourceTreeNode>();

    // Sort to ensure parents are processed before children (shallowest first)
    const sorted = [...resources].sort((a, b) => a.rel_path.split('/').length - b.rel_path.split('/').length);

    for (const res of sorted) {
        const parts = res.rel_path.split('/');
        const name = parts[parts.length - 1];
        const parentPath = parts.slice(0, parts.length - 1).join('/');

        const node: ResourceTreeNode = {
            name: res.name || name,
            relPath: res.rel_path,
            type: res.type,
            children: []
        };

        pathMap.set(res.rel_path, node);

        if (parentPath === "") {
            root.push(node);
        } else {
            const parent = pathMap.get(parentPath);
            if (parent) {
                parent.children.push(node);
            } else {
                // If parent doesn't exist (shouldn't happen with our backend), stick to root
                root.push(node);
            }
        }
    }

    return root;
}

interface ResourceTreeItemProps {
    node: ResourceTreeNode;
    onRemove: (relPath: string) => void;
    level?: number;
}

function ResourceTreeItem({ node, onRemove, level = 0 }: ResourceTreeItemProps) {
    const [isExpanded, setIsExpanded] = useState(true);
    const isFolder = node.type === "Folder";

    return (
        <div className="flex flex-col">
            <div
                className={`flex items-center justify-between p-2 rounded-xl hover:bg-dc-accent/5 transition-all group ${level > 0 ? 'ml-4 border-l border-dc-border/20 pl-4' : ''}`}
            >
                <div className="flex items-center gap-2 overflow-hidden">
                    <div className="flex items-center gap-1.5 min-w-[20px]">
                        {isFolder && (
                            <button
                                onClick={() => setIsExpanded(!isExpanded)}
                                className="p-0.5 hover:bg-dc-accent/10 rounded-md transition-colors"
                            >
                                <ChevronRight
                                    size={12}
                                    className={`transition-transform duration-200 ${isExpanded ? 'rotate-90 text-dc-accent' : 'text-black/40 dark:text-white/40'}`}
                                />
                            </button>
                        )}
                        {!isFolder && <div className="w-1" />}
                    </div>

                    <div className="p-1.5 bg-dc-bg-secondary rounded-lg shrink-0">
                        {node.type === "Script" ? <Terminal size={12} className="text-blue-400" /> :
                            node.type === "Template" ? <FileCode size={12} className="text-green-400" /> :
                                node.type === "Reference" ? <Book size={12} className="text-purple-400" /> :
                                    isFolder ? <FolderPlus size={12} className="text-dc-accent/70" /> :
                                        <File size={12} className="text-black/40 dark:text-white/40" />}
                    </div>

                    <div className="overflow-hidden">
                        <div className={`text-xs truncate ${isFolder ? 'font-bold' : 'font-medium'}`}>{node.name}</div>
                        {level === 0 && !isFolder && <div className="text-[9px] text-black/40 dark:text-white/40 font-mono truncate">{node.relPath}</div>}
                    </div>
                </div>

                <div className="flex items-center gap-2 opacity-0 group-hover:opacity-100 transition-opacity ml-2 shrink-0">
                    <Badge variant="outline" className="text-[8px] uppercase tracking-tighter py-0 h-4">{node.type}</Badge>
                    <button
                        onClick={() => onRemove(node.relPath)}
                        className="p-1 hover:bg-red-400/10 text-black/30 hover:text-red-400 transition-colors rounded-md"
                        title={`Remove ${isFolder ? 'folder' : 'file'}`}
                    >
                        <Trash2 size={12} />
                    </button>
                </div>
            </div>

            {isFolder && isExpanded && node.children.length > 0 && (
                <div className="flex flex-col">
                    {node.children.map(child => (
                        <ResourceTreeItem key={child.relPath} node={child} onRemove={onRemove} level={level + 1} />
                    ))}
                </div>
            )}
            {isFolder && isExpanded && node.children.length === 0 && (
                <div className="ml-10 text-[10px] text-black/30 dark:text-white/30 italic py-1">Empty folder</div>
            )}
        </div>
    );
}

const TEMPLATES = [
    {
        id: "code-expert",
        name: "Code Expert",
        description: "Specialized in a specific language or framework with deep technical knowledge.",
        icon: <FileCode className="w-6 h-6 text-blue-400" />,
        defaultMetadata: {
            name: "code-expert",
            description: "Deep expertise in [Language/Framework]",
            version: "1.0.0",
            author: "",
            tags: ["coding", "development"],
            license: null,
            compatibility: null,
            metadata: null,
            disable_model_invocation: false,
            scope: "Global",
            sync_targets: []
        },
        defaultInstructions: "# Instructions\n\n- Follow [Style Guide]\n- Prioritize [Performance/Readability]\n- Always include [Error Handling]"
    },
    {
        id: "test-gen",
        name: "Test Generator",
        description: "Focuses on generating robust unit, integration, and end-to-end tests.",
        icon: <Terminal className="w-6 h-6 text-green-400" />,
        defaultMetadata: {
            name: "test-gen",
            description: "Automated test generation using [Library]",
            version: "1.0.0",
            author: "",
            tags: ["testing", "qa"],
            license: null,
            compatibility: null,
            metadata: null,
            disable_model_invocation: false,
            scope: "Global",
            sync_targets: []
        },
        defaultInstructions: "# Testing Strategy\n\n- Mock external dependencies\n- Cover edge cases\n- Use [Test Framework] patterns"
    },
    {
        id: "doc-arch",
        name: "Doc Architect",
        description: "For technical writing, system design documentation, and architectural overviews.",
        icon: <Book className="w-6 h-6 text-purple-400" />,
        defaultMetadata: {
            name: "documentation-engine",
            description: "Technical writing for [Project/Domain]",
            version: "1.0.0",
            author: "",
            tags: ["docs", "architecture"],
            license: "MIT",
            compatibility: null,
            metadata: null,
            disable_model_invocation: false,
            scope: "Global",
            sync_targets: []
        },
        defaultInstructions: "# Documentation Standards\n\n- Use Mermaid diagrams for architecture\n- Keep language concise and professional\n- Cross-link related components"
    }
];

export default function SkillEditor({ skill, onClose, onSaved }: SkillEditorProps) {
    const { toast } = useToast();
    const [step, setStep] = useState<"template" | "edit">(skill ? "edit" : "template");
    const [currentSkill, setCurrentSkill] = useState<SkillDto>(skill || {
        metadata: {
            name: "",
            description: "",
            version: "1.0.0",
            author: null,
            tags: [],
            license: null,
            compatibility: null,
            metadata: null,
            disable_model_invocation: false,
            scope: "Global",
            sync_targets: []
        },
        instructions: null,
        resources: [],
        path: null
    });
    const [conflicts, setConflicts] = useState<any[]>([]);
    const [showConflictDialog, setShowConflictDialog] = useState(false);
    const [saving, setSaving] = useState(false);
    const [tagInput, setTagInput] = useState("");
    const [existingTags, setExistingTags] = useState<string[]>([]);

    useEffect(() => {
        const fetchTags = async () => {
            try {
                const skills = await getTaurpc().kms_list_skills();
                const tags = new Set<string>();
                skills.forEach(s => s.metadata.tags.forEach(t => tags.add(t)));
                setExistingTags(Array.from(tags).sort());
            } catch (e) {
                console.error("Failed to fetch tags", e);
            }
        };
        fetchTags();
    }, []);

    const handleSelectTemplate = (template: typeof TEMPLATES[0]) => {
        setCurrentSkill({
            ...currentSkill,
            metadata: {
                ...template.defaultMetadata,
                author: currentSkill.metadata.author
            },
            instructions: template.defaultInstructions
        });
        setStep("edit");
    };

    const handleSave = async () => {
        if (!currentSkill.metadata.name) {
            toast({ title: "Name is required", variant: "destructive" });
            return;
        }

        const nameRegex = /^[a-z0-9-]+$/;
        if (!nameRegex.test(currentSkill.metadata.name)) {
            toast({
                title: "Invalid Skill Name",
                description: "Name must be lowercase alphanumeric with hyphens only.",
                variant: "destructive"
            });
            return;
        }

        setSaving(true);
        try {
            // Check for conflicts before saving if sync targets are selected
            if (currentSkill.metadata.sync_targets.length > 0) {
                const foundConflicts = await getTaurpc().kms_check_skill_conflicts(
                    currentSkill.metadata.name,
                    currentSkill.metadata.sync_targets
                );
                if (foundConflicts.length > 0) {
                    setConflicts(foundConflicts);
                    setShowConflictDialog(true);
                    setSaving(false);
                    return;
                }
            }

            await executeSave(false);
        } catch (error) {
            toast({ title: "Save Failed", description: String(error), variant: "destructive" });
            setSaving(false);
        }
    };

    const executeSave = async (overwrite: boolean) => {
        setSaving(true);
        try {
            await getTaurpc().kms_save_skill(currentSkill, overwrite);
            toast({ title: "Skill Saved", description: `${currentSkill.metadata.name} has been persisted.` });
            onSaved();
        } catch (error) {
            toast({ title: "Save Failed", description: String(error), variant: "destructive" });
        } finally {
            setSaving(false);
            setShowConflictDialog(false);
        }
    };

    const handleAddResource = async (isFolder: boolean) => {
        if (!skill) {
            toast({ title: "Save Skill first", description: "You must save the skill before adding resources.", variant: "destructive" });
            return;
        }

        try {
            const selected = await open({
                directory: isFolder,
                multiple: false,
                title: isFolder ? "Select Resource Folder" : "Select Resource File"
            });

            if (selected) {
                const path = Array.isArray(selected) ? selected[0] : selected;

                // Determine target subdir based on type if possible, or leave null for root
                let targetSubdir: string | null = null;
                if (path.toLowerCase().includes("script")) targetSubdir = "scripts";
                else if (path.toLowerCase().includes("template")) targetSubdir = "templates";
                else if (path.toLowerCase().includes("ref")) targetSubdir = "references";

                const newResource = await getTaurpc().kms_add_skill_resource(
                    currentSkill.metadata.name,
                    path,
                    targetSubdir
                );

                // Perform a full refresh to capture hierarchical contents (especially for folders)
                const updated = await getTaurpc().kms_get_skill(currentSkill.metadata.name);
                if (updated) {
                    setCurrentSkill(updated);
                } else {
                    // Fallback to manual addition if get_skill fails (shouldn't happen)
                    setCurrentSkill({
                        ...currentSkill,
                        resources: [...currentSkill.resources, newResource]
                    });
                }

                toast({ title: "Resource Added", description: `Attached ${newResource.name} to skill.` });
            }
        } catch (e) {
            toast({ title: "Add Failed", description: String(e), variant: "destructive" });
        }
    };

    const handleRefresh = async () => {
        if (!skill) return;
        try {
            const updated = await getTaurpc().kms_get_skill(currentSkill.metadata.name);
            if (updated) {
                setCurrentSkill(updated);
                toast({ title: "Resources Refreshed" });
            }
        } catch (e) {
            toast({ title: "Refresh Failed", description: String(e), variant: "destructive" });
        }
    };

    const handleRemoveResource = async (relPath: string) => {
        try {
            await getTaurpc().kms_remove_skill_resource(currentSkill.metadata.name, relPath);

            // Remove the resource and any children (if it was a folder)
            setCurrentSkill({
                ...currentSkill,
                resources: currentSkill.resources.filter(r =>
                    r.rel_path !== relPath && !r.rel_path.startsWith(relPath + "/")
                )
            });

            toast({ title: "Resource Removed" });
        } catch (e) {
            toast({ title: "Remove Failed", description: String(e), variant: "destructive" });
        }
    };

    return (
        <motion.div
            initial={{ opacity: 0, x: 100 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: 100 }}
            className="absolute inset-0 bg-dc-bg-secondary/95 backdrop-blur-3xl z-50 flex flex-col p-8 gap-8 overflow-hidden"
        >
            {/* Header */}
            <div className="flex items-center justify-between shrink-0">
                <div className="flex items-center gap-4">
                    <button
                        onClick={onClose}
                        className="p-2 hover:bg-dc-bg-hover rounded-xl transition-colors"
                    >
                        <ArrowLeft className="w-5 h-5" />
                    </button>
                    <div>
                        <h2 className="text-2xl font-bold tracking-tight">
                            {skill ? `Edit Skill: ${skill.metadata.name}` : "Create New Skill"}
                        </h2>
                        <p className="text-black/80 dark:text-white/80 text-xs">
                            {step === "template" ? "Select a blueprint to start from" : "Refine metadata and instructions"}
                        </p>
                    </div>
                </div>
                {step === "edit" && (
                    <div className="flex gap-3">
                        <Button variant="ghost" onClick={onClose} disabled={saving}>Cancel</Button>
                        <Button onClick={handleSave} disabled={saving} className="bg-dc-accent hover:bg-dc-accent-hover text-white">
                            <Save className="w-4 h-4 mr-2" />
                            {saving ? "Saving..." : skill ? "Update Skill" : "Create Skill"}
                        </Button>
                    </div>
                )}
            </div>

            <div className="flex-1 overflow-y-auto pr-4 custom-scrollbar">
                <AnimatePresence mode="wait">
                    {step === "template" ? (
                        <motion.div
                            key="template-step"
                            initial={{ opacity: 0, y: 20 }}
                            animate={{ opacity: 1, y: 0 }}
                            exit={{ opacity: 0, y: -20 }}
                            className="grid grid-cols-1 md:grid-cols-3 gap-6 py-8"
                        >
                            {TEMPLATES.map(t => (
                                <button
                                    key={t.id}
                                    onClick={() => handleSelectTemplate(t)}
                                    className="flex flex-col items-start text-left p-8 bg-dc-bg/40 border border-dc-border/40 rounded-3xl hover:border-dc-accent/50 hover:bg-dc-bg-hover transition-all group relative overflow-hidden h-full"
                                >
                                    <div className="absolute top-0 left-0 w-24 h-24 bg-dc-accent/5 rounded-full -translate-x-12 -translate-y-12 blur-2xl group-hover:bg-dc-accent/10 transition-colors" />

                                    <div className="mb-6 p-4 bg-dc-bg-secondary rounded-2xl border border-dc-border/30 group-hover:scale-110 group-hover:bg-dc-bg-hover transition-all">
                                        {t.icon}
                                    </div>
                                    <h3 className="text-xl font-bold mb-2 group-hover:text-dc-accent transition-colors">{t.name}</h3>
                                    <p className="text-black/80 dark:text-white/80 text-sm leading-relaxed mb-6">{t.description}</p>

                                    <div className="mt-auto flex items-center text-xs font-bold text-dc-accent uppercase tracking-widest opacity-0 group-hover:opacity-100 translate-x-[-10px] group-hover:translate-x-0 transition-all">
                                        Use Template <ChevronRight size={14} className="ml-1" />
                                    </div>
                                </button>
                            ))}
                            <button
                                onClick={() => setStep("edit")}
                                className="flex flex-col items-start text-left p-8 bg-dc-bg/40 border-2 border-dashed border-dc-border/40 rounded-3xl hover:border-dc-accent/50 hover:bg-dc-bg-hover transition-all group"
                            >
                                <div className="mb-6 p-4 bg-dc-bg-secondary rounded-2xl opacity-50 group-hover:opacity-100 transition-opacity">
                                    <Sparkles className="w-6 h-6 text-black/80 dark:text-white/80" />
                                </div>
                                <h3 className="text-xl font-bold mb-2">Blank Skill</h3>
                                <p className="text-black/80 dark:text-white/80 text-sm leading-relaxed mb-6">Start from scratch with progressive disclosure levels.</p>
                                <Button onClick={() => setStep("edit")} variant="secondary" className="mt-4 border-dc-accent/30 text-dc-accent">
                                    Start Fresh <ChevronRight size={14} className="ml-1" />
                                </Button>
                            </button>
                        </motion.div>
                    ) : (
                        <motion.div
                            key="edit-step"
                            initial={{ opacity: 0, scale: 0.95 }}
                            animate={{ opacity: 1, scale: 1 }}
                            className="max-w-4xl mx-auto w-full grid grid-cols-1 md:grid-cols-12 gap-12"
                        >
                            {/* Metadata Column */}
                            <div className="md:col-span-5 space-y-8">
                                <div className="space-y-4 bg-dc-bg/30 p-8 rounded-3xl border border-dc-border/30 backdrop-blur-md shadow-xl">
                                    <div className="flex items-center gap-2 mb-2">
                                        <Badge variant="secondary" className="border-dc-accent/30 text-dc-accent">Level 1: Metadata</Badge>
                                    </div>
                                    <div className="space-y-4">
                                        <div className="space-y-2">
                                            <Label className="text-[10px] font-bold uppercase tracking-widest text-black/80 dark:text-white/80 flex justify-between">
                                                <span>Skill Name <span className="text-red-400">*</span></span>
                                            </Label>
                                            <Input
                                                value={currentSkill.metadata.name}
                                                onChange={e => setCurrentSkill({ ...currentSkill, metadata: { ...currentSkill.metadata, name: e.target.value.toLowerCase().replace(/\s+/g, '-') } })}
                                                placeholder="e.g. react-architecture-master"
                                                className="bg-white text-black dark:bg-[#1e293b] dark:text-white border-dc-border/30 h-10 rounded-xl"
                                                disabled={!!skill}
                                            />
                                            <p className="text-[10px] text-black/50 dark:text-white/50 italic">Must be lowercase with hyphens.</p>
                                        </div>
                                        <div className="grid grid-cols-2 gap-4">
                                            <div className="space-y-2">
                                                <Label className="text-[10px] font-bold uppercase tracking-widest text-black/80 dark:text-white/80">Version</Label>
                                                <Input
                                                    list="version-suggestions"
                                                    value={currentSkill.metadata.version}
                                                    onChange={e => setCurrentSkill({ ...currentSkill, metadata: { ...currentSkill.metadata, version: e.target.value } })}
                                                    className="bg-white text-black dark:bg-[#1e293b] dark:text-white border-dc-border/30 h-10 rounded-xl font-mono"
                                                />
                                            </div>
                                            <div className="space-y-2">
                                                <Label className="text-[10px] font-bold uppercase tracking-widest text-black/80 dark:text-white/80">Scope</Label>
                                                <select
                                                    value={currentSkill.metadata.scope}
                                                    onChange={e => setCurrentSkill({ ...currentSkill, metadata: { ...currentSkill.metadata, scope: e.target.value as any } })}
                                                    className="w-full bg-white text-black dark:bg-[#1e293b] dark:text-white border border-dc-border/30 rounded-xl px-3 h-10 text-sm focus:outline-none focus:border-dc-accent/50 transition-all shadow-inner"
                                                >
                                                    <option value="Global">Global</option>
                                                    <option value="Project">Project</option>
                                                </select>
                                            </div>
                                        </div>
                                        <div className="grid grid-cols-2 gap-4">
                                            <div className="space-y-2">
                                                <Label className="text-[10px] font-bold uppercase tracking-widest text-black/80 dark:text-white/80">License</Label>
                                                <Input
                                                    value={currentSkill.metadata.license || ""}
                                                    onChange={e => setCurrentSkill({ ...currentSkill, metadata: { ...currentSkill.metadata, license: e.target.value } })}
                                                    placeholder="MIT / Proprietary"
                                                    className="bg-white text-black dark:bg-[#1e293b] dark:text-white border-dc-border/30 h-10 rounded-xl"
                                                />
                                            </div>
                                            <div className="space-y-2">
                                                <Label className="text-[10px] font-bold uppercase tracking-widest text-black/80 dark:text-white/80">Model Invocation</Label>
                                                <div className="flex items-center gap-2 h-10">
                                                    <input
                                                        type="checkbox"
                                                        checked={!currentSkill.metadata.disable_model_invocation}
                                                        onChange={e => setCurrentSkill({ ...currentSkill, metadata: { ...currentSkill.metadata, disable_model_invocation: !e.target.checked } })}
                                                        className="w-4 h-4 rounded border-dc-accent/30 text-dc-accent"
                                                    />
                                                    <span className="text-xs">Allow Auto-Invoke</span>
                                                </div>
                                            </div>
                                        </div>
                                        <div className="space-y-2">
                                            <Label className="text-[10px] font-bold uppercase tracking-widest text-black/80 dark:text-white/80">Compatibility</Label>
                                            <Input
                                                value={currentSkill.metadata.compatibility || ""}
                                                onChange={e => setCurrentSkill({ ...currentSkill, metadata: { ...currentSkill.metadata, compatibility: e.target.value } })}
                                                placeholder="e.g. Node.js >= 18, Windows/Linux"
                                                className="bg-white text-black dark:bg-[#1e293b] dark:text-white border-dc-border/30 h-10 rounded-xl"
                                            />
                                        </div>
                                        <div className="space-y-2">
                                            <Label className="text-[10px] font-bold uppercase tracking-widest text-black/80 dark:text-white/80">Arbitrary Metadata (JSON)</Label>
                                            <textarea
                                                value={currentSkill.metadata.metadata || ""}
                                                onChange={e => setCurrentSkill({ ...currentSkill, metadata: { ...currentSkill.metadata, metadata: e.target.value } })}
                                                className="w-full bg-white text-black dark:bg-[#1e293b] dark:text-white border-dc-border/30 rounded-xl p-3 text-xs font-mono focus:outline-none focus:border-dc-accent/50 h-20 transition-all resize-none shadow-inner"
                                                placeholder='{"source": "cursor-standard", "category": "expert"}'
                                            />
                                        </div>
                                        <div className="space-y-2">
                                            <Label className="text-[10px] font-bold uppercase tracking-widest text-black/80 dark:text-white/80">Description</Label>
                                            <textarea
                                                value={currentSkill.metadata.description}
                                                onChange={e => setCurrentSkill({ ...currentSkill, metadata: { ...currentSkill.metadata, description: e.target.value } })}
                                                className="w-full bg-white text-black dark:bg-[#1e293b] dark:text-white border-dc-border/30 rounded-xl p-3 text-sm focus:outline-none focus:border-dc-accent/50 h-24 transition-all resize-none shadow-inner"
                                                placeholder="What does this skill enable the agent to do?"
                                            />
                                        </div>
                                        <div className="space-y-2">
                                            <Label className="text-[10px] font-bold uppercase tracking-widest text-black/80 dark:text-white/80">Tags</Label>
                                            <div className="relative">
                                                <Input
                                                    value={tagInput}
                                                    onChange={e => setTagInput(e.target.value)}
                                                    onKeyDown={e => {
                                                        if (e.key === "Enter" && tagInput.trim()) {
                                                            e.preventDefault();
                                                            const newTag = tagInput.trim().toLowerCase();
                                                            if (!currentSkill.metadata.tags.includes(newTag)) {
                                                                setCurrentSkill({
                                                                    ...currentSkill,
                                                                    metadata: {
                                                                        ...currentSkill.metadata,
                                                                        tags: [...currentSkill.metadata.tags, newTag]
                                                                    }
                                                                });
                                                            }
                                                            setTagInput("");
                                                        }
                                                    }}
                                                    placeholder="Add tag and press Enter..."
                                                    className="bg-white text-black dark:bg-[#1e293b] dark:text-white border-dc-border/30 h-10 rounded-xl"
                                                    list="tag-suggestions"
                                                />
                                                <datalist id="tag-suggestions">
                                                    {existingTags.map(t => <option key={t} value={t} />)}
                                                </datalist>
                                            </div>
                                            {/* Tag Bubbles */}
                                            <div className="flex flex-wrap gap-2 mt-2 pt-2 border-t border-dc-border/10">
                                                {currentSkill.metadata.tags.map(tag => (
                                                    <Badge
                                                        key={tag}
                                                        variant="secondary"
                                                        className="pl-2 pr-1 py-1 rounded-full bg-dc-accent/10 hover:bg-dc-accent/20 border-dc-accent/30 text-dc-accent flex items-center gap-1 group transition-all"
                                                    >
                                                        {tag}
                                                        <button
                                                            onClick={() => {
                                                                setCurrentSkill({
                                                                    ...currentSkill,
                                                                    metadata: {
                                                                        ...currentSkill.metadata,
                                                                        tags: currentSkill.metadata.tags.filter(t => t !== tag)
                                                                    }
                                                                });
                                                            }}
                                                            className="p-0.5 rounded-full hover:bg-dc-accent/30 text-dc-accent/50 hover:text-dc-accent transition-colors"
                                                        >
                                                            <X className="w-3 h-3" />
                                                        </button>
                                                    </Badge>
                                                ))}
                                                {currentSkill.metadata.tags.length === 0 && (
                                                    <span className="text-[10px] text-black/80 dark:text-white/80 italic">No tags added yet</span>
                                                )}
                                            </div>
                                        </div>
                                    </div>
                                </div>

                                <div className="space-y-4 bg-dc-bg/20 p-8 rounded-3xl border border-dc-border/20 backdrop-blur-md">
                                    <Label className="text-[10px] font-bold uppercase tracking-widest text-black/80 dark:text-white/80">Sync Target</Label>
                                    <div className="flex flex-col gap-2">
                                        <p className="text-[10px] text-black/80 dark:text-white/80 leading-relaxed">
                                            By default, skills are saved to your KMS vault. You can also sync them to your IDE skill folders.
                                        </p>
                                        <div className="mt-2 space-y-2">
                                            {[
                                                { path: ".cursor/skills", label: "Cursor IDE" },
                                                { path: ".claude/skills", label: "Claude Code" },
                                                { path: ".gemini/antigravity/skills", label: "Antigravity IDE" }
                                            ].map(({ path: target, label }) => (
                                                <label key={target} className="flex items-center gap-2 p-3 bg-dc-bg-secondary rounded-xl border border-dc-border/30 cursor-pointer hover:bg-dc-bg-hover transition-all group shadow-sm">
                                                    <input
                                                        type="checkbox"
                                                        checked={currentSkill.metadata.sync_targets?.includes(target)}
                                                        onChange={(e) => {
                                                            const checked = e.target.checked;
                                                            const targets = checked
                                                                ? [...(currentSkill.metadata.sync_targets || []), target]
                                                                : (currentSkill.metadata.sync_targets || []).filter(t => t !== target);
                                                            setCurrentSkill({
                                                                ...currentSkill,
                                                                metadata: { ...currentSkill.metadata, sync_targets: targets }
                                                            });
                                                        }}
                                                        className="w-4 h-4 rounded-md border-dc-accent/30 text-dc-accent focus:ring-dc-accent/20"
                                                    />
                                                    <div className="flex flex-col">
                                                        <span className="text-xs font-bold text-black/80 dark:text-white/80">{label}</span>
                                                        <span className="text-[9px] font-mono text-black/40 dark:text-white/40 group-hover:text-dc-accent/60 transition-colors">~/{target}</span>
                                                    </div>
                                                </label>
                                            ))}
                                        </div>
                                    </div>
                                </div>
                            </div>

                            {/* Instructions Column */}
                            <div className="md:col-span-7 space-y-6 flex flex-col h-full min-h-[500px]">
                                <div className="flex-1 flex flex-col bg-dc-bg/30 rounded-3xl border border-dc-border/30 backdrop-blur-md shadow-2xl overflow-hidden">
                                    <div className="p-4 bg-dc-bg-secondary border-b border-dc-border/30 flex items-center justify-between">
                                        <div className="flex items-center gap-2">
                                            <Badge variant="secondary" className="border-dc-accent/30 text-dc-accent">Level 2: Instructions <span className="text-red-400 ml-1">*</span></Badge>
                                        </div>
                                        <div className="flex gap-1">
                                            <div className="w-2 h-2 rounded-full bg-red-400/50" />
                                            <div className="w-2 h-2 rounded-full bg-amber-400/50" />
                                            <div className="w-2 h-2 rounded-full bg-green-400/50" />
                                        </div>
                                    </div>
                                    <textarea
                                        value={currentSkill.instructions || ""}
                                        onChange={e => setCurrentSkill({ ...currentSkill, instructions: e.target.value })}
                                        className="flex-1 w-full bg-white text-black dark:bg-transparent dark:text-white p-6 font-mono text-sm focus:outline-none transition-all resize-none overflow-y-auto custom-scrollbar"
                                        placeholder="# System Instructions for the Agent..."
                                    />
                                </div>

                                {/* Resources Section */}
                                <div className="flex-1 flex flex-col bg-dc-bg/30 rounded-3xl border border-dc-border/30 backdrop-blur-md shadow-2xl overflow-hidden mt-6">
                                    <div className="p-4 bg-dc-bg-secondary border-b border-dc-border/30 flex items-center justify-between">
                                        <div className="flex items-center gap-2">
                                            <Badge variant="secondary" className="border-dc-accent/30 text-dc-accent">Level 3: Resources & Attachments</Badge>
                                        </div>
                                        <div className="flex gap-2">
                                            <Button
                                                variant="ghost"
                                                size="sm"
                                                className="h-7 text-[10px] font-bold uppercase tracking-wider hover:bg-dc-accent/10 text-dc-accent"
                                                onClick={handleRefresh}
                                                disabled={!skill}
                                            >
                                                <RefreshCw size={12} className="mr-1" /> Sync
                                            </Button>
                                            <Button
                                                variant="ghost"
                                                size="sm"
                                                className="h-7 text-[10px] font-bold uppercase tracking-wider hover:bg-dc-accent/10 text-dc-accent"
                                                onClick={() => handleAddResource(false)}
                                            >
                                                <Plus size={12} className="mr-1" /> File
                                            </Button>
                                            <Button
                                                variant="ghost"
                                                size="sm"
                                                className="h-7 text-[10px] font-bold uppercase tracking-wider hover:bg-dc-accent/10 text-dc-accent"
                                                onClick={() => handleAddResource(true)}
                                            >
                                                <FolderPlus size={12} className="mr-1" /> Folder
                                            </Button>
                                        </div>
                                    </div>
                                    <div className="flex-1 p-4 overflow-y-auto custom-scrollbar">
                                        <div className="space-y-1">
                                            {currentSkill.resources.length > 0 ? (
                                                buildResourceTree(currentSkill.resources).map(node => (
                                                    <ResourceTreeItem
                                                        key={node.relPath}
                                                        node={node}
                                                        onRemove={handleRemoveResource}
                                                    />
                                                ))
                                            ) : (
                                                <div className="flex flex-col items-center justify-center py-12 text-center opacity-40">
                                                    <Layers size={32} className="mb-4" />
                                                    <p className="text-sm font-medium">No resources attached</p>
                                                    <p className="text-[10px] max-w-[200px] mt-2 leading-relaxed">
                                                        Attach scripts, documentation, or templates to provide deeper context or tool definitions for the agent.
                                                    </p>
                                                </div>
                                            )}
                                        </div>
                                    </div>
                                </div>
                            </div>
                        </motion.div>
                    )}
                </AnimatePresence>
            </div>

            {/* Conflict Dialog */}
            <AnimatePresence>
                {showConflictDialog && (
                    <div className="fixed inset-0 z-[100] flex items-center justify-center p-4 bg-black/40 backdrop-blur-sm">
                        <motion.div
                            initial={{ opacity: 0, scale: 0.95, y: 20 }}
                            animate={{ opacity: 1, scale: 1, y: 0 }}
                            exit={{ opacity: 0, scale: 0.95, y: 20 }}
                            className="bg-dc-bg p-8 rounded-3xl border border-dc-border shadow-2xl max-w-md w-full space-y-6"
                        >
                            <div className="flex items-center gap-4 text-amber-400">
                                <AlertTriangle size={32} />
                                <h3 className="text-xl font-bold">Sync Conflict Detected</h3>
                            </div>
                            <p className="text-sm text-black/70 dark:text-white/70">
                                One or more sync targets already contain a skill named <span className="font-bold text-dc-accent">{currentSkill.metadata.name}</span>.
                                Existing files always take precedence.
                            </p>
                            <div className="space-y-2 max-h-40 overflow-y-auto pr-2 custom-scrollbar">
                                {conflicts.map((c, i) => (
                                    <div key={i} className="p-3 bg-dc-accent/5 rounded-xl border border-dc-accent/10 flex items-center justify-between text-xs">
                                        <span className="font-mono text-dc-accent">{c.target}</span>
                                        <Badge variant="outline" className="text-[8px] uppercase">{c.conflict_type}</Badge>
                                    </div>
                                ))}
                            </div>
                            <div className="flex flex-col gap-3 pt-4">
                                <Button onClick={() => setShowConflictDialog(false)} variant="secondary">Cancel & Rename Skill</Button>
                                <Button
                                    onClick={() => executeSave(true)}
                                    className="bg-amber-500 hover:bg-amber-600 text-white"
                                >
                                    Overwrite Existing Skills
                                </Button>
                            </div>
                        </motion.div>
                    </div>
                )}
            </AnimatePresence>
        </motion.div>
    );
}
