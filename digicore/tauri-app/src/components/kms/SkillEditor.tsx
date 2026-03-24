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
    CheckCircle2
} from "lucide-react";
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

const TEMPLATES = [
    {
        id: "code-expert",
        name: "Code Expert",
        description: "Specialized in a specific language or framework with deep technical knowledge.",
        icon: <FileCode className="w-6 h-6 text-blue-400" />,
        defaultMetadata: {
            name: "New Code Skill",
            description: "Deep expertise in [Language/Framework]",
            version: "1.0.0",
            author: "",
            tags: ["coding", "development"]
        },
        defaultInstructions: "# Instructions\n\n- Follow [Style Guide]\n- Prioritize [Performance/Readability]\n- Always include [Error Handling]"
    },
    {
        id: "test-gen",
        name: "Test Generator",
        description: "Focuses on generating robust unit, integration, and end-to-end tests.",
        icon: <Terminal className="w-6 h-6 text-green-400" />,
        defaultMetadata: {
            name: "Testing Authority",
            description: "Automated test generation using [Library]",
            version: "1.0.0",
            author: "",
            tags: ["testing", "qa"]
        },
        defaultInstructions: "# Testing Strategy\n\n- Mock external dependencies\n- Cover edge cases\n- Use [Test Framework] patterns"
    },
    {
        id: "doc-arch",
        name: "Doc Architect",
        description: "For technical writing, system design documentation, and architectural overviews.",
        icon: <Book className="w-6 h-6 text-purple-400" />,
        defaultMetadata: {
            name: "Documentation Engine",
            description: "Technical writing for [Project/Domain]",
            version: "1.0.0",
            author: "",
            tags: ["docs", "architecture"]
        },
        defaultInstructions: "# Documentation Standards\n\n- Use Mermaid diagrams for architecture\n- Keep language concise and professional\n- Cross-link related components"
    }
];

export default function SkillEditor({ skill, onClose, onSaved }: SkillEditorProps) {
    const { toast } = useToast();
    const [step, setStep] = useState<"template" | "edit">(skill ? "edit" : "template");
    const [currentSkill, setCurrentSkill] = useState<SkillDto>(skill || {
        metadata: { name: "", description: "", version: "1.0.0", author: null, tags: [] },
        instructions: null,
        resources: [],
        path: null
    });
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
            metadata: { ...template.defaultMetadata, author: currentSkill.metadata.author },
            instructions: template.defaultInstructions
        });
        setStep("edit");
    };

    const handleSave = async () => {
        if (!currentSkill.metadata.name) {
            toast({ title: "Name is required", variant: "destructive" });
            return;
        }

        setSaving(true);
        try {
            await getTaurpc().kms_save_skill(currentSkill);
            toast({ title: "Skill Saved", description: `${currentSkill.metadata.name} has been persisted.` });
            onSaved();
        } catch (error) {
            toast({ title: "Save Failed", description: String(error), variant: "destructive" });
        } finally {
            setSaving(false);
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
                                                onChange={e => setCurrentSkill({ ...currentSkill, metadata: { ...currentSkill.metadata, name: e.target.value } })}
                                                placeholder="e.g. React Architecture Master"
                                                className="bg-white text-black dark:bg-[#1e293b] dark:text-white border-dc-border/30 h-10 rounded-xl"
                                                disabled={!!skill}
                                            />
                                        </div>
                                        <div className="space-y-2">
                                            <Label className="text-[10px] font-bold uppercase tracking-widest text-black/80 dark:text-white/80">Version</Label>
                                            <div className="relative">
                                                <Input
                                                    list="version-suggestions"
                                                    value={currentSkill.metadata.version}
                                                    onChange={e => setCurrentSkill({ ...currentSkill, metadata: { ...currentSkill.metadata, version: e.target.value } })}
                                                    className="bg-white text-black dark:bg-[#1e293b] dark:text-white border-dc-border/30 h-10 rounded-xl font-mono"
                                                />
                                                <datalist id="version-suggestions">
                                                    {SUGGESTED_VERSIONS.map(v => <option key={v} value={v} />)}
                                                </datalist>
                                            </div>
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
                                            {[".cursor/skills", ".claude/skills"].map(target => (
                                                <label key={target} className="flex items-center gap-2 p-2 bg-dc-bg-secondary rounded-lg border border-dc-border/30 cursor-pointer hover:bg-dc-bg-hover transition-colors">
                                                    <input type="checkbox" className="w-4 h-4 rounded-md border-dc-accent/30 text-dc-accent" />
                                                    <span className="text-xs font-mono">{target}</span>
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
                            </div>
                        </motion.div>
                    )}
                </AnimatePresence>
            </div>
        </motion.div>
    );
}
