import { useEffect, useState } from "react";
import type { PendingVariableInput } from "../../types";

interface VariableInputProps {
  visible: boolean;
  data: PendingVariableInput | null;
  onOk: (values: Record<string, string>) => void;
  onCancel: () => void;
}

export function VariableInput({
  visible,
  data,
  onOk,
  onCancel,
}: VariableInputProps) {
  const [values, setValues] = useState<Record<string, string>>({});

  useEffect(() => {
    if (visible && data) {
      const initial: Record<string, string> = {};
      for (const v of data.vars) {
        if (v.var_type === "checkbox") {
          initial[v.tag] = data.checkbox_checked[v.tag]
            ? (v.options?.[0] || "yes")
            : "";
        } else if (v.var_type === "choice") {
          const idx = data.choice_indices[v.tag] ?? 0;
          initial[v.tag] = v.options[idx] ?? data.values[v.tag] ?? "";
        } else {
          initial[v.tag] = data.values[v.tag] ?? "";
        }
      }
      setValues(initial);
    }
  }, [visible, data]);

  const handleChange = (tag: string, value: string) => {
    setValues((prev) => ({ ...prev, [tag]: value }));
  };

  const handleOk = () => {
    onOk(values);
  };

  if (!visible || !data) return null;

  return (
    <div className="fixed inset-0 bg-black/50 z-[1000] flex items-center justify-center">
      <div className="bg-[var(--dc-bg-elevated)] text-[var(--dc-text)] p-5 rounded-lg max-w-[600px] w-[90%] border border-[var(--dc-border)]">
        <h3 className="mt-0 mb-4 text-lg font-semibold">
          Snippet Input Required (F11)
        </h3>
        <p className="mb-4">Enter values for placeholders:</p>
        <div className="space-y-3">
          {data.vars.map((v) => (
            <label key={v.tag} className="block">
              {v.label}:{" "}
              {v.var_type === "edit" ||
              v.var_type === "date_picker" ||
              v.var_type === "file_picker" ? (
                <input
                  type="text"
                  value={values[v.tag] ?? ""}
                  onChange={(e) => handleChange(v.tag, e.target.value)}
                  placeholder={
                    v.var_type === "date_picker" ? "YYYYMMDD" : ""
                  }
                  className="w-full max-w-[300px] p-1.5 mt-1 bg-[var(--dc-bg)] text-[var(--dc-text)] border border-[var(--dc-border)] rounded"
                />
              ) : v.var_type === "choice" ? (
                <select
                  value={values[v.tag] ?? v.options[0]}
                  onChange={(e) => handleChange(v.tag, e.target.value)}
                  className="w-full max-w-[300px] p-1.5 mt-1 bg-[var(--dc-bg)] text-[var(--dc-text)] border border-[var(--dc-border)] rounded"
                >
                  {v.options.map((opt, i) => (
                    <option key={i} value={opt}>
                      {opt}
                    </option>
                  ))}
                </select>
              ) : v.var_type === "checkbox" ? (
                <input
                  type="checkbox"
                  checked={!!values[v.tag]}
                  onChange={(e) =>
                    handleChange(
                      v.tag,
                      e.target.checked ? (v.options?.[0] || "yes") : ""
                    )
                  }
                  className="ml-2"
                />
              ) : null}
            </label>
          ))}
        </div>
        <div className="mt-4 flex gap-2">
          <button
            onClick={handleOk}
            className="px-2.5 py-1 text-sm bg-[var(--dc-accent)] text-white rounded"
          >
            OK
          </button>
          <button
            onClick={onCancel}
            className="px-2.5 py-1 text-sm bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded"
          >
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
}
