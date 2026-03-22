/**
 * Normalize TauRPC DTOs to frontend types (Record instead of Partial).
 */
import type { AppState, Snippet } from "../types";
import type { AppStateDto, PendingVariableInputDto } from "../bindings";
import type { PendingVariableInput } from "../types";

export function normalizeAppState(dto: AppStateDto): AppState {
  const lib = dto.library ?? {};
  const library: Record<string, Snippet[]> = {};
  for (const [k, v] of Object.entries(lib)) {
    if (v) {
      library[k] = v.map((s) => ({
        trigger: s.trigger,
        trigger_type: (s.trigger_type ?? 'suffix') as 'suffix' | 'regex',
        content: s.content,
        htmlContent: s.htmlContent ?? null,
        rtfContent: s.rtfContent ?? null,
        options: s.options ?? '',
        category: s.category ?? '',
        profile: s.profile ?? 'Default',
        appLock: s.appLock ?? '',
        pinned: s.pinned ?? 'false',
        case_adaptive: s.case_adaptive ?? true,
        case_sensitive: s.case_sensitive ?? false,
        smart_suffix: s.smart_suffix ?? true,
        is_sensitive: s.is_sensitive ?? false,
        lastModified: s.lastModified ?? '',
      }));
    }
  }
  return { ...dto, library } as AppState;
}

export function normalizePendingInput(
  dto: PendingVariableInputDto
): PendingVariableInput {
  return {
    ...dto,
    values: (dto.values ?? {}) as Record<string, string>,
    choice_indices: (dto.choice_indices ?? {}) as Record<string, number>,
    checkbox_checked: (dto.checkbox_checked ?? {}) as Record<string, boolean>,
  };
}
