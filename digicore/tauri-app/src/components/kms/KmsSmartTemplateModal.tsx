import React, { useState, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { Sparkles, X, Check, Calendar, FileText, ChevronDown } from "lucide-react";
import { Button } from "../ui/button";
import { Input } from "../ui/input";
import { SnippetLogicTestResultDto } from "../../bindings";
import { cn } from "../../lib/utils";

interface KmsSmartTemplateModalProps {
    visible: boolean;
    data: SnippetLogicTestResultDto | null;
    onOk: (values: Record<string, string>) => void;
    onCancel: () => void;
}

export default function KmsSmartTemplateModal({
    visible,
    data,
    onOk,
    onCancel,
}: KmsSmartTemplateModalProps) {
    const [values, setValues] = useState<Record<string, string>>({});

    useEffect(() => {
        if (visible && data) {
            const initial: Record<string, string> = {};
            for (const v of data.vars) {
                if (v.var_type === "checkbox") {
                    initial[v.tag] = v.options?.[0] || "yes";
                } else if (v.var_type === "choice") {
                    initial[v.tag] = v.options[0] ?? "";
                } else {
                    initial[v.tag] = "";
                }
            }
            setValues(initial);
        }
    }, [visible, data]);

    const handleChange = (tag: string, value: string) => {
        setValues((prev) => ({ ...prev, [tag]: value }));
    };

    if (!visible || !data) return null;

    return (
        <AnimatePresence>
            <div className="fixed inset-0 z-[100] flex items-center justify-center p-4">
                {/* Backdrop */}
                <motion.div
                    initial={{ opacity: 0 }}
                    animate={{ opacity: 1 }}
                    exit={{ opacity: 0 }}
                    onClick={onCancel}
                    className="fixed inset-0 bg-black/60 backdrop-blur-sm"
                />

                {/* Modal Container */}
                <motion.div
                    initial={{ opacity: 0, scale: 0.95, y: 20 }}
                    animate={{ opacity: 1, scale: 1, y: 0 }}
                    exit={{ opacity: 0, scale: 0.95, y: 20 }}
                    className="relative w-full max-w-lg overflow-hidden rounded-2xl border border-dc-border bg-dc-bg-elevated/90 backdrop-blur-xl shadow-2xl flex flex-col"
                >
                    {/* Header */}
                    <div className="p-6 border-b border-dc-border flex items-center justify-between bg-gradient-to-r from-dc-accent/5 to-transparent">
                        <div className="flex items-center gap-3">
                            <div className="h-10 w-10 rounded-xl bg-dc-accent/10 flex items-center justify-center text-dc-accent shadow-inner">
                                <Sparkles size={20} />
                            </div>
                            <div>
                                <h3 className="text-lg font-semibold text-dc-text leading-tight">
                                    Smart Template Input
                                </h3>
                                <p className="text-xs text-dc-text-muted mt-0.5">
                                    Configure dynamic variables for your note
                                </p>
                            </div>
                        </div>
                        <Button
                            variant="ghost"
                            size="sm"
                            className="h-8 w-8 p-0 rounded-full hover:bg-dc-bg-alt"
                            onClick={onCancel}
                        >
                            <X size={16} />
                        </Button>
                    </div>

                    {/* Content */}
                    <div className="p-6 space-y-5 max-h-[60vh] overflow-y-auto custom-scrollbar">
                        {data.vars.map((v) => (
                            <div key={v.tag} className="space-y-1.5 group">
                                <label className="text-xs font-medium text-dc-text-muted transition-colors group-focus-within:text-dc-accent">
                                    {v.label}
                                </label>

                                {v.var_type === "edit" || v.var_type === "date_picker" || v.var_type === "file_picker" ? (
                                    <div className="relative">
                                        <Input
                                            type="text"
                                            value={values[v.tag] ?? ""}
                                            onChange={(e) => handleChange(v.tag, e.target.value)}
                                            placeholder={v.var_type === "date_picker" ? "YYYYMMDD" : `Enter ${v.label.toLowerCase()}...`}
                                            className="bg-dc-bg/50 border-dc-border focus:border-dc-accent transition-all pl-9"
                                        />
                                        <div className="absolute left-3 top-1/2 -translate-y-1/2 text-dc-text-muted">
                                            {v.var_type === "date_picker" ? <Calendar size={14} /> : <FileText size={14} />}
                                        </div>
                                    </div>
                                ) : v.var_type === "choice" ? (
                                    <div className="relative">
                                        <select
                                            value={values[v.tag] ?? v.options[0]}
                                            onChange={(e) => handleChange(v.tag, e.target.value)}
                                            className="w-full h-10 px-3 pr-10 bg-dc-bg/50 border border-dc-border rounded-md text-sm text-dc-text focus:outline-none focus:ring-1 focus:ring-dc-accent focus:border-dc-accent appearance-none transition-all"
                                        >
                                            {v.options.map((opt, i) => (
                                                <option key={i} value={opt}>
                                                    {opt}
                                                </option>
                                            ))}
                                        </select>
                                        <div className="absolute right-3 top-1/2 -translate-y-1/2 text-dc-text-muted pointer-events-none">
                                            <ChevronDown size={14} />
                                        </div>
                                    </div>
                                ) : v.var_type === "checkbox" ? (
                                    <div
                                        className="flex items-center gap-3 p-3 rounded-lg bg-dc-bg/40 border border-dc-border cursor-pointer hover:bg-dc-bg/60 transition-colors"
                                        onClick={() => handleChange(v.tag, values[v.tag] ? "" : (v.options?.[0] || "yes"))}
                                    >
                                        <div className={cn(
                                            "h-5 w-5 rounded border flex items-center justify-center transition-all",
                                            values[v.tag] ? "bg-dc-accent border-dc-accent" : "border-dc-border bg-dc-bg"
                                        )}>
                                            {values[v.tag] && <Check size={12} className="text-white" />}
                                        </div>
                                        <span className="text-sm text-dc-text font-medium">{v.label}</span>
                                    </div>
                                ) : null}
                            </div>
                        ))}
                    </div>

                    {/* Footer */}
                    <div className="p-4 bg-dc-bg/30 border-t border-dc-border flex justify-end gap-3">
                        <Button
                            variant="secondary"
                            onClick={onCancel}
                            className="bg-dc-bg-alt hover:bg-dc-bg border-dc-border"
                        >
                            Cancel
                        </Button>
                        <Button
                            variant="default"
                            onClick={() => onOk(values)}
                            className="bg-dc-accent hover:bg-dc-accent/90 text-white shadow-lg shadow-dc-accent/20 px-6"
                        >
                            Apply Selection
                        </Button>
                    </div>
                </motion.div>
            </div>
        </AnimatePresence>
    );
}
