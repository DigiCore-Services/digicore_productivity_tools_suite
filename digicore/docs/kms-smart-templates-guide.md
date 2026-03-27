# KMS Smart Template System - User Guide

The Smart Template System brings the power of the DigiCore scripting engine directly into your Knowledge Management Suite. You can now insert dynamic, automated, and interactive content into your notes with a single click.

## 🚀 Quick Start
1. **Type a Placeholder**: Enter a tag like `{date}` or `{js: Math.PI}` in your note.
2. **Select & Expand**: Highlight the tag (or the whole note) and click the **Sparkles (✨)** icon in the editor toolbar.
3. **Interactive Input**: If the template requires information (like a name or choice), a glassmorphic modal will appear for you to fill in the details.

---

## 🛠 Available Placeholders

### 1. Simple Dynamic Tags
These tags are pre-defined and require no calculation.
- `{date}`: Current date in default format (e.g., `2026-03-25`).
- `{time}`: Current time (e.g., `18:30`).
- `{date:FORMAT}`: Custom date formatting. Example: `{date:YYYY/MM/DD}`.
- `{clipboard}`: Inserts the current text from your clipboard.

### 2. Powerful Scripting `{js:...}`
Execute JavaScript directly inside your note. The result of the expression is inserted.
- **Math**: `{js: 125 * 0.8}` -> `100`
- **Text Logic**: `{js: "important note".toUpperCase()}` -> `IMPORTANT NOTE`
- **Formatting**: `{js: new Intl.NumberFormat().format(1000000)}` -> `1,000,000`

### 3. Interactive Variables
Collect input from yourself at the time of expansion. Use these to create "Fill-in-the-blank" templates.

#### 📝 Text Input (`{edit:...}`)
Creates a text field in the popup modal.
- Syntax: `{edit:VariableLabel}`
- Example: `Task for today: {edit:What is the primary goal?}`

#### ✅ Checkbox (`{checkbox:...}`)
Creates a toggle. If checked, it returns the value; if unchecked, it returns nothing.
- Syntax: `{checkbox:Label|Value}`
- Example: `{checkbox:Urgent?|🔥 URGENT}`

#### 🔽 Dropdown Selection (`{choice:...}`)
Provides a list of options.
- Syntax: `{choice:Label|Option1|Option2|...}`
- Example: `Project Status: {choice:Status|Pending|In Progress|Done}`

---

## 💡 Pro-Tip: Multi-Line Templates
You can mix several tags into a single block of text or even a whole file. 

**Example Template for a Daily Meeting:**
```markdown
# Daily Standup - {date}

**Key Focus**: {edit:Focus}
**Priority**: {choice:Priority|Low|Medium|High}
**Done Yesterday**: {edit:Yesterday}

---
*Generated via DigiCore Smart Templates*
```

When you click **✨ Expand**, DigiCore will prompt you for the Focus, Priority, and Yesterday's tasks all at once in a beautiful overlay!

---

## ⚠️ Important Notes
- **Selection Sensitivity**: If you have text selected, DigiCore will *only* evaluate placeholders within that selection. If nothing is selected, it evaluates the *entire* document.
- **Privacy**: JavaScript execution is local to your machine.
