# Audit & Enhancement Plan: Scripting Engine Library

## 1. Executive Summary
This document provides a comprehensive audit of the DigiCore "Scripting Engine Library" and outlines a strategic plan for transitioning from a collection of loosely coupled executors to a robust, unified, and feature-rich scripting subsystem.

## 2. Technical Audit & Analysis

### 2.1 Current State Assessment
| Feature | Implementation | Strength | Weakness |
| :--- | :--- | :--- | :--- |
| **JavaScript** | `boa_engine` (Native Rust) | Excellent embedding, sandbox, resource limits. | Limited to ES6+ features supported by Boa. |
| **HTTP** | `reqwest` (Port/Adapter) | Robust, configuration-first, domain allowlist. | Blocking by default (though async exists). |
| **Python** | Subprocess (`Command`) | Easy to implement. | Slow startup, no context injection, requires system install. |
| **Lua** | Subprocess (`Command`) | Simple wrapper. | Slow startup, no context injection, no shared state. |
| **Shell (Run)** | Subprocess with Allowlist | Secure by default. | OS-dependent command syntax. |
| **DSL** | `evalexpr` | Fast math evaluation. | Very limited expression set. |

### 2.2 Architectural Review
*   **Hexagonal Architecture:** The project correctly identifies `ScriptEnginePort` and `HttpFetcherPort`. However, compliance is inconsistent: JS and HTTP use the registry, while Python, Lua, and Run are hardcoded in the `dispatch` function.
*   **SOLID/SRP:** The `script_type_registry::dispatch` function is becoming a "God Function" that knows too much about individual engine implementations.
*   **Configuration-First:** Strong adherence. All engines are toggleable and configurable via `scripting.json`.

## 3. Findings & Areas for Improvement

### 3.1 Critical Gaps
1.  **Context Inconsistency:** JavaScript has access to `clipboard`, `clip_history`, and `user_vars`. Python and Lua scripts are currently "blind" to this context, limiting their utility in text expansion.
2.  **Diagnostic Opaqueity:** Errors in subprocess-based engines (Python/Lua/Run) are captured as raw strings. There is no structured mapping back to source lines or specific error types.
3.  **Performance Overhead:** Running `python.exe` for a 1-line snippet expansion creates significant latency and high CPU spikes.
4.  **Inconsistent Port Usage:** Python and Lua bypass the `ScriptEnginePort` trait, making them harder to mock or swap in tests.

### 3.2 Robustness Requirements
*   **Graceful Handling:** Current implementation returns error strings like `"[Python Error: ...]"`. This is good for end-users but lacks internal diagnostic logging.
*   **Diagnostic Logging:** Need to move from `eprintln!` to structured logging (e.g., `tracing`) with correlation IDs for snippet expansion events.

## 4. Alternative Implementation Options

### Option A: Refined Subprocess (Current Path+)
*   **Description:** Keep subprocesses but use persistent "worker" processes (via stdin/stdout pipes) to avoid startup overhead.
*   **Pros:** Low dependency overhead; users use their existing Python/Lua environments.
*   **Cons:** Complex process management (zombie processes, pipe leaks).
*   **SWOT:**
    *   **S:** Easy to debug scripts externally.
    *   **W:** IPC overhead.
    *   **O:** Can support any version of Python installed.
    *   **T:** Security risks of persistent background processes.

### Option B: Deeply Embedded Engines (Recommended)
*   **Description:** Use `pyo3` (Python) and `mlua` (Lua) to embed engines directly into the Rust binary.
*   **Pros:** Zero startup overhead; shared memory for context injection; synchronous state sharing.
*   **Cons:** Significantly larger binary size; `pyo3` requires specific Python dev headers at build time.
*   **SWOT:**
    *   **S:** Maximum performance and integration.
    *   **W:** Compilation complexity.
    *   **O:** Truly "Professional" feel; instant expansions.
    *   **T:** Version mismatch between embedded libs and system libs.

### Option C: WASM-based Polyglot Engine
*   **Description:** Run Python/Lua compiled to WASM inside a Wasmtime sandbox.
*   **Pros:** Maximum security; zero system dependencies.
*   **Cons:** Massive performance hit; limited library support in WASM.

## 5. Implementation Plan (The "Robust" Path)

### Phase 1: Port Standardization (SRP/SOLID)
1.  **Refactor `py_executor` and `lua_executor`** to implement `ScriptEnginePort`.
2.  **Update `ScriptingRegistry`** to hold a `HashMap<String, Arc<dyn ScriptEnginePort>>` instead of a single engine.
3.  **Clean up `dispatch`**: It should simply look up the prefix in the registry and call `.execute()`.

### Phase 2: Context Injection & Diagnostics
1.  **JSON Context Passing**: For subprocess engines, pass the entire `ScriptContext` as a JSON string via stdin instead of base64 argv.
2.  **Error Mapping**: Implement a parser for Python/Lua stderr to populate `ScriptError` fields (line, column, message).
3.  **Tracing Integration**: Replace `eprintln!` calls with `tracing::info!`/`error!` macro blocks.

### Phase 3: Performance (The "Embedded" Decision)
1.  **Decision Point**: User to decide if `pyo3` / `mlua` are acceptable dependencies for the build environment.
2.  **If yes**: Implement `EmbeddedPyEngine` and `EmbeddedLuaEngine`.
3.  **If no**: Implement `PersistentPyWorker` using a long-lived process and pipe protocol.

## 6. Key Decisions Required
1.  **Dependency Strategy:** Do you prefer keeping the binary small (Subprocess) or making it ultra-fast (Embedded)?
2.  **Python Version Policy:** Should we support whatever `python` is in the PATH, or bundle a specific version?
3.  **Logging Destination:** Should diagnostic logs go to a file, the system event log, or a dedicated GUI console?

---
**Status:** Audit Complete | **Next Step:** Implementation of Phase 1 (Standardization) pending review.
