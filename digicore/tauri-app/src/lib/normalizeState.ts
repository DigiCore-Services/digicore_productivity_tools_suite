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
    if (v) library[k] = v;
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
