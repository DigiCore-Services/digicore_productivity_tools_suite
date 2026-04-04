export interface ParsedKmsGraphWarning {
    code: string | null;
    message: string;
    raw: string;
}

const CODED_WARNING_RE = /^([A-Z0-9_]+)::\s*(.+)$/;

export function parseKmsGraphWarning(raw: string): ParsedKmsGraphWarning {
    const trimmed = (raw ?? "").trim();
    const m = CODED_WARNING_RE.exec(trimmed);
    if (!m) {
        return { code: null, message: trimmed, raw: trimmed };
    }
    return { code: m[1], message: m[2], raw: trimmed };
}

export function normalizeKmsGraphWarnings(rawWarnings: string[]): ParsedKmsGraphWarning[] {
    const out: ParsedKmsGraphWarning[] = [];
    const seen = new Set<string>();
    for (const raw of rawWarnings ?? []) {
        const parsed = parseKmsGraphWarning(raw);
        const key = `${parsed.code ?? ""}::${parsed.message}`;
        if (seen.has(key)) continue;
        seen.add(key);
        out.push(parsed);
    }
    return out;
}

