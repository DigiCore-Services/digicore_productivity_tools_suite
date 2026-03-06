import CodeMirror from "@uiw/react-codemirror";
import { javascript } from "@codemirror/lang-javascript";
import { python } from "@codemirror/lang-python";
import { StreamLanguage } from "@codemirror/language";
import { lua } from "@codemirror/legacy-modes/mode/lua";
import { shell } from "@codemirror/legacy-modes/mode/shell";
import type { Extension } from "@codemirror/state";

type ScriptEditorLanguage =
  | "javascript"
  | "python"
  | "lua"
  | "http"
  | "dsl"
  | "run";

interface ScriptCodeEditorProps {
  value: string;
  onChange: (next: string) => void;
  minHeight?: string;
  language: ScriptEditorLanguage;
}

function extensionsForLanguage(language: ScriptEditorLanguage): Extension[] {
  switch (language) {
    case "javascript":
      return [javascript({ jsx: false })];
    case "python":
      return [python()];
    case "lua":
      return [StreamLanguage.define(lua)];
    case "run":
      return [StreamLanguage.define(shell)];
    case "http":
    case "dsl":
    default:
      return [];
  }
}

function fallbackPlaceholder(language: ScriptEditorLanguage): string {
  switch (language) {
    case "javascript":
      return "function greet(name) { return `Hello ${name}`; }";
    case "python":
      return "def greet(name: str) -> str:\n    return f\"Hello {name}\"";
    case "lua":
      return "function greet(name)\n  return \"Hello \" .. tostring(name)\nend";
    case "http":
      return "{http:https://api.example.com/data|path.to.value}";
    case "dsl":
      return "{dsl:(2 + 3) * 4}";
    case "run":
      return "{run:hostname}";
    default:
      return "";
  }
}

export function ScriptCodeEditor({
  value,
  onChange,
  minHeight = "280px",
  language,
}: ScriptCodeEditorProps) {
  if (import.meta.env.MODE === "test") {
    return (
      <textarea
        value={value}
        onChange={(e) => onChange(e.target.value)}
        rows={16}
        className="w-full p-2 font-mono text-sm bg-[var(--dc-bg)] text-[var(--dc-text)] border border-[var(--dc-border)] rounded"
      />
    );
  }

  return (
    <div className="rounded border border-[var(--dc-border)] overflow-hidden">
      <CodeMirror
        value={value}
        onChange={onChange}
        basicSetup={{
          lineNumbers: true,
          highlightActiveLine: true,
          foldGutter: true,
          bracketMatching: true,
          closeBrackets: true,
        }}
        extensions={extensionsForLanguage(language)}
        theme="dark"
        height={minHeight}
        placeholder={fallbackPlaceholder(language)}
      />
    </div>
  );
}

