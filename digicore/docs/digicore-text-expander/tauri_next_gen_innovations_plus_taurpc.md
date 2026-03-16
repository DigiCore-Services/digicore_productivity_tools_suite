# Tauri Next-Gen Innovations & TauRPC Integration
## The "Ultra-Elite" Roadmap for DigiCore Text Expander

The project has reached a high level of maturity, having already implemented Mica effects, SQLite storage, and worker-based fuzzy search. This document defines the "Next-Gen" horizon, leveraging Tauri 2.0 and the upcoming TauRPC refactor.

---

## 1. Security & Privacy at the Core

### 1.1 Windows Hello / Biometric Lock
As the Text Expander becomes a central vault for productivity (and potentially sensitive data), high-end security becomes a differentiator.
*   **Recommendation**: Integrate `tauri-plugin-biometry`.
*   **Use Case**: Lock specific snippet categories (e.g., "Credentials" or "Work") behind Windows Hello. 
*   **User Experience**: When a locked snippet is triggered, a Windows Hello prompt appears. Only after successful authentication is the text expanded.

### 1.2 Snippet "Privacy Modes"
Implement a "Sensitive Mode" that disables telemetry and expansion history for specific applications (like Banking or Password Managers) automatically using the existing `active_window` tracking.

---

## 2. Local AI: The "Intelligent" Expander

### 2.1 Local LLM Sidecar (Privacy-First AI)
Text expanding is fundamentally about predicting and generating text. Integrated local AI takes this to the next level.
*   **Architecture**: Use `llama.cpp` or `candle` as a Tauri **Sidecar**.
*   **Features**:
    *   **Contextual Auto-Complete**: If you've typed "Looking forward to...", the expander suggests a snippet for "meeting you next Tuesday".
    *   **Snippet Creation via Natural Language**: From the Command Palette, type `/ask Write a JS regex for email` and have it instantly converted into a reusable snippet.
    *   **Tone Rewriting**: Trigger a "Professional Rewrite" snippet that takes the current clipboard, runs it through the local LLM, and replaces it with a polished version.

---

## 3. The TauRPC "Power User" Architecture

The current refactor to **TauRPC** enables deep, type-safe optimizations that were previously brittle.

### 3.1 End-to-End Validation with Zod
*   **Strategy**: Use the generated Specta types to drive **Zod schemas** in the React frontend.
*   **Polish**: This ensures that forms in the "Snippet Editor" or "Config Tab" provide real-time validation errors that are *guaranteed* to match the Rust backend's expectations.

### 3.2 Type-Safe Event Hub
While TauRPC handles commands, the app relies heavily on `emit()` and `listen()` for its "Ghost" windows.
*   **Recommendation**: Create a wrapped `EventEmitter` in TypeScript that uses the same `bindings.ts` types. This eliminates the risk of "Magic String" typos in events like `ghost-follower-update`.

---

## 4. Professional Extensibility

### 4.1 "UI Plugins" & The JS Sandbox
Since the app already has a JS global library, elevate it to a full Plugin System.
*   **Innovation**: Allow users to write JS "manifests" that can:
    *   Inject custom CSS for the Overlays.
    *   Add custom items to the System Tray.
    *   Register new Global Shortcuts via the TauRPC bridge.
*   **Benefit**: Users can share "Snippet Packs" that come with their own custom logic and UI enhancements.

---

## 5. Enhanced Desktop Experience (The "Invisible" WOW)

*   **Window Shadows & Borderless Perfection**: Now that Ghost windows are borderless, use `tauri-v1` or `tauri-v2` window shadow plugins to apply deep, lush system shadows to the overlays, making them feel like they are floating above the OS.
*   **Dynamic Tray Animation**: Change the Tray Icon state (using `app_handle.tray_icon().set_icon()`) to show "Expansion Active", "Paused", or "AI Processing" with subtle visual cues.
*   **Input Smoothing**: Optimize the F11 Variable Input transition using Framer Motion's `layout` prop to ensure fields grow and shrink smoothly as users select different choices.
