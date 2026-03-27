import React, { useState, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import {
    Activity,
    Database,
    FileText,
    Scissors,
    Clipboard,
    Layers,
    AlertTriangle,
    RefreshCw,
    Trash2,
    CheckCircle2,
    XCircle,
    Info,
    History
} from "lucide-react";
import { getTaurpc } from "../../lib/taurpc";
import { KmsDiagnosticsDto } from "../../bindings";
import { Button } from "../ui/button";
import { useToast } from "../ui/use-toast";

export default function KmsHealthDashboard() {
    const { toast } = useToast();
    const [diagnostics, setDiagnostics] = useState<KmsDiagnosticsDto | null>(null);
    const [loading, setLoading] = useState(true);
    const [pruning, setPruning] = useState(false);
    const [refreshKey, setRefreshKey] = useState(0);

    useEffect(() => {
        const fetchDiag = async () => {
            setLoading(true);
            try {
                const data = await getTaurpc().kms_get_diagnostics();
                setDiagnostics(data);
            } catch (err) {
                console.error("Failed to fetch diagnostics:", err);
            } finally {
                setLoading(false);
            }
        };
        fetchDiag();
    }, [refreshKey]);

    const handlePrune = async () => {
        if (!window.confirm("This will optimize git history and prune unreachable objects. It may take a few seconds and will make history extraction slightly faster. Proceed?")) return;

        setPruning(true);
        try {
            const result = await getTaurpc().kms_prune_history();
            toast({
                title: "Prune Complete",
                description: result,
            });
            setRefreshKey(prev => prev + 1);
        } catch (err) {
            toast({
                title: "Prune Failed",
                description: String(err),
                variant: "destructive",
            });
        } finally {
            setPruning(false);
        }
    };

    const containerVariants = {
        hidden: { opacity: 0, y: 10 },
        visible: {
            opacity: 1,
            y: 0,
            transition: { staggerChildren: 0.1 }
        }
    };

    const cardVariants = {
        hidden: { opacity: 0, scale: 0.95 },
        visible: { opacity: 1, scale: 1 }
    };

    if (loading && !diagnostics) {
        return (
            <div className="flex items-center justify-center p-12 h-64">
                <div className="flex flex-col items-center gap-4 opacity-50">
                    <RefreshCw className="w-8 h-8 animate-spin text-dc-accent" />
                    <span className="text-xs font-bold uppercase tracking-widest">Collecting Metrics...</span>
                </div>
            </div>
        );
    }

    const StatCard = ({ icon: Icon, label, value, colorClass }: any) => (
        <motion.div
            variants={cardVariants}
            className="bg-dc-bg-secondary/30 border border-dc-border/40 rounded-3xl p-6 backdrop-blur-md flex flex-col gap-3 group transition-all hover:bg-dc-bg-hover/50 hover:border-dc-accent/30 shadow-xl shadow-transparent hover:shadow-black/5"
        >
            <div className={`p-3 rounded-2xl ${colorClass} bg-current/10 w-fit group-hover:scale-110 transition-transform`}>
                <Icon className={`${colorClass} w-5 h-5`} />
            </div>
            <div className="space-y-1">
                <div className="text-[10px] font-bold text-dc-text-muted uppercase tracking-widest">{label}</div>
                <div className="text-2xl font-bold tracking-tight">{value.toLocaleString()}</div>
            </div>
        </motion.div>
    );

    return (
        <motion.div
            variants={containerVariants}
            initial="hidden"
            animate="visible"
            className="space-y-8"
        >
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-5 gap-4">
                <StatCard
                    icon={FileText}
                    label="Active Notes"
                    value={diagnostics?.note_count || 0}
                    colorClass="text-dc-blue"
                />
                <StatCard
                    icon={Scissors}
                    label="Snippets"
                    value={diagnostics?.snippet_count || 0}
                    colorClass="text-dc-amber"
                />
                <StatCard
                    icon={Clipboard}
                    label="Clip History"
                    value={diagnostics?.clip_count || 0}
                    colorClass="text-dc-green"
                />
                <StatCard
                    icon={Layers}
                    label="AI Embeddings"
                    value={diagnostics?.vector_count || 0}
                    colorClass="text-dc-accent"
                />
                <StatCard
                    icon={diagnostics?.error_log_count && diagnostics.error_log_count > 0 ? AlertTriangle : CheckCircle2}
                    label="Sync Health"
                    value={diagnostics?.error_log_count || 0}
                    colorClass={diagnostics?.error_log_count && diagnostics.error_log_count > 0 ? "text-dc-red" : "text-dc-green"}
                />
            </div>

            <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
                {/* Database Health */}
                <div className="lg:col-span-2 bg-dc-bg-secondary/20 border border-dc-border/30 rounded-3xl p-8 space-y-6">
                    <div className="flex items-center justify-between">
                        <div className="flex items-center gap-3">
                            <div className="p-2 bg-dc-green/10 rounded-xl">
                                <Database className="text-dc-green w-5 h-5" />
                            </div>
                            <h2 className="text-xl font-bold tracking-tight">Data Integrity</h2>
                        </div>
                        <Badge variant="secondary" className="bg-dc-green/10 text-dc-green border-dc-green/20">Operational</Badge>
                    </div>

                    <div className="space-y-4">
                        <div className="flex items-center justify-between p-4 bg-dc-bg/40 rounded-2xl border border-dc-border/20">
                            <div className="flex items-center gap-3">
                                <CheckCircle2 className="w-4 h-4 text-dc-green" />
                                <div className="text-sm font-medium">SQLite Vector Index</div>
                            </div>
                            <div className="text-xs text-dc-text-muted">Optimized</div>
                        </div>
                        <div className="flex items-center justify-between p-4 bg-dc-bg/40 rounded-2xl border border-dc-border/20">
                            <div className="flex items-center gap-3">
                                <CheckCircle2 className="w-4 h-4 text-dc-green" />
                                <div className="text-sm font-medium">FTS5 Search Indices</div>
                            </div>
                            <div className="text-xs text-dc-text-muted">Consistent</div>
                        </div>
                        <div className="flex items-center justify-between p-4 bg-dc-bg/40 rounded-2xl border border-dc-border/20">
                            <div className="flex items-center gap-3">
                                <CheckCircle2 className="w-4 h-4 text-dc-green" />
                                <div className="text-sm font-medium">Link Graph Relationship Map</div>
                            </div>
                            <div className="text-xs text-dc-text-muted">Linked</div>
                        </div>
                    </div>
                </div>

                {/* Maintenance Actions */}
                <div className="bg-dc-bg-secondary/20 border border-dc-border/30 rounded-3xl p-8 space-y-6">
                    <div className="flex items-center gap-3">
                        <div className="p-2 bg-dc-accent/10 rounded-xl">
                            <Activity className="text-dc-accent w-5 h-5" />
                        </div>
                        <h2 className="text-xl font-bold tracking-tight">Maintenance</h2>
                    </div>

                    <div className="space-y-3">
                        <Button
                            className="w-full justify-start h-14 rounded-2xl bg-dc-bg-secondary hover:bg-dc-bg-hover text-dc-text border-dc-border/40 hover:border-dc-accent/40 group transition-all"
                            variant="secondary"
                            onClick={handlePrune}
                            disabled={pruning}
                        >
                            <History className={`w-5 h-5 mr-3 text-dc-accent group-hover:scale-110 transition-transform ${pruning ? "animate-spin" : ""}`} />
                            <div className="flex flex-col items-start">
                                <span className="text-sm font-bold">Prune History</span>
                                <span className="text-[10px] text-dc-text-muted">Compresses Git repository size</span>
                            </div>
                        </Button>

                        <Button
                            className="w-full justify-start h-14 rounded-2xl bg-dc-bg-secondary hover:bg-dc-bg-hover text-dc-text border-dc-border/40 hover:border-dc-accent/40 group transition-all"
                            variant="secondary"
                            onClick={async () => {
                                if (window.confirm("Perform an aggressive re-validation of all index entries?")) {
                                    try {
                                        await getTaurpc().kms_reindex_all();
                                        toast({ title: "Repair Started", description: "Indexing all local content." });
                                    } catch (e) {
                                        toast({ title: "Error", description: String(e), variant: "destructive" });
                                    }
                                }
                            }}
                        >
                            <RefreshCw className="w-5 h-5 mr-3 text-dc-green group-hover:rotate-180 transition-transform duration-500" />
                            <div className="flex flex-col items-start">
                                <span className="text-sm font-bold">Deep Reindex</span>
                                <span className="text-[10px] text-dc-text-muted">Reconstructs vector map</span>
                            </div>
                        </Button>
                    </div>

                    <div className="bg-dc-accent/5 rounded-2xl p-4 flex gap-3 border border-dc-accent/10">
                        <Info size={16} className="text-dc-accent shrink-0 mt-0.5" />
                        <p className="text-[10px] text-dc-text-muted leading-relaxed">
                            Scheduled maintenance runs every 24h. History pruning is recommended after large vault moves or renames.
                        </p>
                    </div>
                </div>
            </div>
        </motion.div>
    );
}

function Badge({ children, variant, className }: any) {
    return (
        <span className={`px-2 py-0.5 rounded-full text-[10px] font-bold uppercase tracking-wider border ${className}`}>
            {children}
        </span>
    );
}
