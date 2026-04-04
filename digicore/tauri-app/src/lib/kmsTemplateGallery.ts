export type KmsTemplateGalleryEntry = {
    id: string;
    title: string;
    description: string;
    body: string;
    /** When set, the gallery suggests creating under this vault-relative path pattern. */
    suggestDailyPath?: boolean;
};

/**
 * Curated starter bodies for KMS; saved as normal markdown files via `kms_save_note`.
 */
export const KMS_TEMPLATE_GALLERY: KmsTemplateGalleryEntry[] = [
    {
        id: "blank",
        title: "Blank note",
        description: "Minimal heading and body.",
        body: "# New note\n\n",
    },
    {
        id: "daily",
        title: "Daily note",
        description: "Frontmatter tags + sections for priorities and log.",
        body: "---\ntags: [daily]\n---\n\n# Daily\n\n## Priorities\n\n- \n\n## Log\n\n",
        suggestDailyPath: true,
    },
    {
        id: "meeting",
        title: "Meeting",
        description: "Attendees, agenda, notes, actions.",
        body: "---\ntags: [meeting]\n---\n\n# Meeting\n\n**When:** \n**Attendees:** \n\n## Agenda\n\n1. \n\n## Notes\n\n\n## Actions\n\n- [ ] \n",
    },
    {
        id: "project",
        title: "Project hub",
        description: "Links, milestones, risks.",
        body: "---\ntags: [project]\n---\n\n# Project\n\n## Goal\n\n\n## Links\n\n- \n\n## Milestones\n\n- [ ] \n\n## Risks\n\n- \n",
    },
];

/** Vault-relative path like `notes/daily/YYYY-MM-DD.md` (local date). */
export function kmsDailyNoteRelPath(d = new Date()): string {
    const y = d.getFullYear();
    const m = String(d.getMonth() + 1).padStart(2, "0");
    const day = String(d.getDate()).padStart(2, "0");
    return `notes/daily/${y}-${m}-${day}.md`;
}
