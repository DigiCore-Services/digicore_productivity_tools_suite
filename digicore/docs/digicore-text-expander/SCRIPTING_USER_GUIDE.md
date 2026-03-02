# DigiCore Text Expander - Scripting User Guide

**Version:** 1.1  
**Last Updated:** 2026-02-28  
**Product:** DigiCore Text Expander (Rust)

---

## Table of Contents

1. [Overview](#1-overview)
2. [Configuration](#2-configuration)
3. [JavaScript](#3-javascript-js)
4. [Custom DSL](#4-custom-dsl-dsl)
5. [Python](#5-python-py)
6. [Lua](#6-lua-lua)
7. [HTTP/REST](#7-httprest-http)
8. [Run Command](#8-run-command-run)
9. [Script Library](#9-script-library)
10. [Error Messages](#10-error-messages)
11. [Processing Order](#11-processing-order)
12. [Security](#12-security)

---

## 1. Overview

### 1.1 Supported Script Types

DigiCore Text Expander supports six scripting placeholder types within snippet templates:

| Placeholder | Description | Default | Config File |
|-------------|-------------|---------|-------------|
| `{js:...}` | JavaScript (Boa engine) | Enabled | `scripting.json` |
| `{dsl:...}` | Custom DSL (math expressions) | Enabled | `scripting.json` |
| `{py:...}` | Python script | **Disabled** | `scripting.json` |
| `{lua:...}` | Lua script | **Disabled** | `scripting.json` |
| `{http:url\|path}` | HTTP GET with optional JSON path | Enabled | `scripting.json` |
| `{run:cmd}` | Shell command | **Disabled** | `scripting.json` |

### 1.2 Configuration File Location

**Windows:** `%APPDATA%\DigiCore\config\scripting.json`

Example path: `C:\Users\YourName\AppData\Roaming\DigiCore\config\scripting.json`

### 1.3 Environment Variable Overrides

Set `DIGICORE_ENV` or `RUST_ENV` to override behavior:

| Value | Effect |
|-------|--------|
| `dev` | Longer timeouts (10s), debug logging enabled |
| `test` | Shorter timeouts (2s), JS sandbox disabled |
| `prod` | JS sandbox enabled (default) |

---

## 2. Configuration

### 2.1 scripting.json Structure

```json
{
  "dsl": {
    "enabled": true
  },
  "http": {
    "url_allowlist": [],
    "timeout_secs": 5,
    "retry_count": 3,
    "retry_delay_ms": 500,
    "use_async": false
  },
  "js": {
    "library_path": "scripts/global_library.js",
    "library_paths": [],
    "timeout_secs": 5,
    "fallback_on_error": "[JS Error]",
    "sandbox_enabled": true,
    "debug_execution": false,
    "recursion_limit": 1000,
    "loop_iteration_limit": 1000000
  },
  "py": {
    "enabled": false,
    "path": "",
    "library_path": "scripts/global_library.py"
  },
  "lua": {
    "enabled": false,
    "path": "",
    "library_path": "scripts/global_library.lua"
  },
  "run": {
    "disabled": true,
    "allowlist": ""
  },
  "debug_logging": false
}
```

### 2.2 Config Field Reference

| Section | Field | Default | Description |
|---------|-------|---------|-------------|
| `dsl` | `enabled` | `true` | Enable/disable `{dsl:expr}` |
| `http` | `url_allowlist` | `[]` | Empty = allow all; list domains to restrict |
| `http` | `timeout_secs` | `5` | HTTP request timeout |
| `http` | `retry_count` | `3` | Retries on failure (exponential backoff) |
| `http` | `use_async` | `false` | Use async HTTP (tokio) instead of blocking |
| `js` | `library_path` | `scripts/global_library.js` | Path to global JS library |
| `js` | `library_paths` | `[]` | Multiple paths (overrides library_path when non-empty) |
| `js` | `timeout_secs` | `5` | JS execution timeout (0 = no limit) |
| `js` | `sandbox_enabled` | `true` | Reject `eval`, `Function`, `new Function` |
| `js` | `recursion_limit` | `1000` | Max recursion depth |
| `js` | `loop_iteration_limit` | `1000000` | Max loop iterations |
| `py` | `enabled` | `false` | Enable `{py:code}` |
| `py` | `path` | `""` | Path to Python executable (empty = `python` from PATH) |
| `py` | `library_path` | `scripts/global_library.py` | Path to global Python library (relative to config root); Script Library tab when enabled |
| `lua` | `enabled` | `false` | Enable `{lua:code}` |
| `lua` | `path` | `""` | Path to Lua executable (empty = `lua` from PATH) |
| `lua` | `library_path` | `scripts/global_library.lua` | Path to global Lua library (relative to config root); Script Library tab when enabled |
| `run` | `disabled` | `true` | Disable `{run:cmd}` (recommended) |
| `run` | `allowlist` | `""` | Comma-separated allowlist: `hostname,cmd,python,C:\Scripts\` |

---

## 3. JavaScript (`{js:...}`)

### 3.1 Overview

JavaScript is executed via the embedded Boa engine. It supports full ES5-style JavaScript with Date, String, Math, and other standard objects.

### 3.2 Syntax

```
{js: expression }
```

The expression is evaluated and its result is converted to a string. Use `return` for multi-line blocks or expressions; the last evaluated value is used.

### 3.3 Built-in Globals

| Variable | Description |
|----------|-------------|
| `clipboard` | Current clipboard content (string) |
| `clip1` | Most recent clipboard history entry |
| `clip2` | Second most recent |
| `clip3` | Third most recent |
| ... | Up to `clip10` |
| `var_Label` | User input from `{var:Label}` (e.g. `var_Env`) |
| `choice_Label` | User selection from `{choice:Label\|opt1\|opt2}` |
| `checkbox_Label` | Checkbox value from `{checkbox:Label\|value}` |
| `date_picker_Label` | Date from `{date_picker:Label}` |
| `file_picker_Label` | File path from `{file_picker:Label}` |

### 3.4 Clipboard in JS

Use `"{clipboard}"` (quoted) in your JS code to inject the current clipboard content:

```
{js: "hello".length + " ".length + "{clipboard}".length }
```

The literal `"{clipboard}"` is replaced with the escaped clipboard string before evaluation.

### 3.5 Global Script Library

Functions defined in the Global Script Library (`scripts/global_library.js`) are available in all `{js:...}` tags. Edit via the **Script Library** tab in the app.

### 3.6 Examples

**Simple arithmetic:**
```
Logic Check: 10 + 20 = {js: 10 + 20}
```

**Time-based greeting:**
```
Good {js: new Date().getHours() < 12 ? "Morning" : "Afternoon"}, what can I help you with today?
```

**Clipboard length:**
```
Clipboard has {js: "{clipboard}".length } characters
```

**Using clip history:**
```
First clip: {js: clip1 }
Second clip: {js: clip2 }
```

**User variable (from {var:Env}):**
```
Environment: {js: var_Env }
```

**Global library function:**
```
{js: greet("World") }
```

**Date formatting:**
```
Today is {js: new Date().toLocaleDateString() }
```

### 3.7 Sandbox Restrictions

When `sandbox_enabled` is `true` (default), the following are **rejected**:

- `eval(`
- `Function(`
- `new Function`

### 3.8 Error Messages

| Message | Meaning |
|---------|---------|
| `[JS Error: ...]` | General execution error |
| `[JS Error: execution timeout (Ns)]` | Script exceeded timeout |
| `[JS Error: sandbox violation]` | Code contains forbidden constructs |

---

## 4. Custom DSL (`{dsl:...}`)

### 4.1 Overview

The DSL uses the `meval` crate for math expression evaluation. It supports numbers, operators, and common math functions.

### 4.2 Syntax

```
{dsl: expression }
```

### 4.3 Supported Operators

| Operator | Description |
|----------|-------------|
| `+` | Addition |
| `-` | Subtraction |
| `*` | Multiplication |
| `/` | Division |
| `%` | Remainder |
| `^` | Power |

### 4.4 Built-in Functions

| Function | Description |
|----------|-------------|
| `sqrt(x)` | Square root |
| `abs(x)` | Absolute value |
| `exp(x)` | e^x |
| `ln(x)` | Natural logarithm |
| `sin(x)`, `cos(x)`, `tan(x)` | Trigonometric |
| `asin(x)`, `acos(x)`, `atan(x)`, `atan2(y,x)` | Inverse trig |
| `sinh(x)`, `cosh(x)`, `tanh(x)` | Hyperbolic |
| `floor(x)`, `ceil(x)`, `round(x)` | Rounding |
| `min(x, y, ...)` | Minimum of 1+ numbers |
| `max(x, y, ...)` | Maximum of 1+ numbers |
| `signum(x)` | Sign of x |

### 4.5 Constants

| Constant | Value |
|----------|-------|
| `pi` | 3.14159... |
| `e` | 2.71828... |

### 4.6 Examples

**Basic arithmetic:**
```
Total: {dsl: 10 + 20 * 3 }
```

**Rounding:**
```
Rounded: {dsl: round(3.7) }
```

**Math functions:**
```
sqrt(16) = {dsl: sqrt(16) }
pi * 2 = {dsl: pi * 2 }
```

**Min/Max:**
```
Min of 5, 3, 7: {dsl: min(5, 3, 7) }
```

### 4.7 Error Messages

| Message | Meaning |
|---------|---------|
| `[DSL Error: ...]` | Invalid expression or syntax |
| `[DSL disabled by config]` | Disabled in `scripting.json` |

---

## 5. Python (`{py:...}`)

### 5.1 Overview

Python runs via subprocess. Code is passed as an expression and evaluated with `eval()`. **Must be enabled** in `scripting.json` (`py.enabled: true`).

### 5.2 Syntax

```
{py: expression }
```

The expression is evaluated and `print(eval(expr))` is used. Output is captured from stdout.

### 5.3 Requirements

- Python 3 installed and on PATH (or set `py.path` in config)
- `py.enabled: true` in `scripting.json`

### 5.4 Examples

**Simple expression:**
```
Python: {py: 1 + 2 }
```

**String methods:**
```
Uppercase: {py: "hello".upper() }
```

**List/join:**
```
Joined: {py: ", ".join(["a", "b", "c"]) }
```

**Date:**
```
Today: {py: __import__("datetime").datetime.now().strftime("%Y-%m-%d") }
```

### 5.5 Limitations

- **Expression only:** Code is wrapped as `print(eval(code))`. Multi-line statements are not supported.
- **Subprocess:** Each expansion spawns a new Python process; may be slower than JS.

### 5.6 Error Messages

| Message | Meaning |
|---------|---------|
| `[Python disabled by config]` | `py.enabled` is false |
| `[Python Error: ...]` | Execution error or Python not found |

---

## 6. Lua (`{lua:...}`)

### 6.1 Overview

Lua runs via subprocess. Code is written to a temp file and executed. **Must print to stdout** for output. **Must be enabled** in `scripting.json` (`lua.enabled: true`).

### 6.2 Syntax

```
{lua: code }
```

Code must call `print()` to produce output.

### 6.3 Requirements

- Lua installed and on PATH (or set `lua.path` in config)
- `lua.enabled: true` in `scripting.json`

### 6.4 Examples

**Simple expression:**
```
Lua: {lua: print(1 + 2) }
```

**String:**
```
{lua: print(string.upper("hello")) }
```

**Table:**
```
{lua: print(table.concat({"a","b","c"}, ", ")) }
```

**Date (Lua 5.3+ os.date):**
```
{lua: print(os.date("%Y-%m-%d")) }
```

### 6.5 Limitations

- **Must print:** Code must explicitly call `print()` to produce output.
- **Temp file:** Code is written to a temp file; execution is via `lua script.lua`.

### 6.6 Error Messages

| Message | Meaning |
|---------|---------|
| `[Lua disabled by config]` | `lua.enabled` is false |
| `[Lua Error: ...]` | Execution error or Lua not found |

---

## 7. HTTP/REST (`{http:...}`)

### 7.1 Overview

Fetches a URL via HTTP GET. Optionally extracts a value from JSON using a dot-separated path.

### 7.2 Syntax

```
{http:url}
{http:url|jsonPath}
```

- **url:** Full URL (e.g. `https://api.example.com/data`)
- **jsonPath:** Optional. Dot-separated path (e.g. `ip`, `data.items.0.name`)

### 7.3 JSON Path

- Object keys: `ip`, `slip.advice`, `current_weather.temperature`
- Array indices: `items.0`, `data.1`

### 7.4 URL Allowlist

If `http.url_allowlist` is non-empty, only requests to listed domains (and subdomains) are allowed. Empty list = allow all.

Example:
```json
"url_allowlist": ["api.example.com", "api.ipify.org"]
```

### 7.5 Examples

**Full response:**
```
{http:https://api.ipify.org}
```

**JSON path:**
```
My IP: {http:https://api.ipify.org?format=json|ip}
```

**Advice API:**
```
Motivation: {http:https://api.adviceslip.com/advice|slip.advice}
```

**Weather:**
```
London temp: {http:https://api.open-meteo.com/v1/forecast?latitude=51.5074&longitude=-0.1278&current_weather=true|current_weather.temperature}°C
```

### 7.6 Retry and Timeout

- **Retries:** 3 by default with exponential backoff
- **Timeout:** 5 seconds by default
- **Async:** Set `use_async: true` to use async HTTP (tokio)

### 7.7 Error Messages

| Message | Meaning |
|---------|---------|
| `[HTTP Error: domain not in allowlist]` | URL not in allowlist |
| `[HTTP Timeout]` | Request timed out |
| `[HTTP Error: ...]` | Network or server error |
| `[Path Error: ...]` | Invalid JSON or path not found |

---

## 8. Run Command (`{run:...}`)

### 8.1 Overview

Executes a shell command and captures stdout. **Disabled by default** for security. Requires allowlist when enabled.

### 8.2 Syntax

```
{run: command }
```

**Windows:** `cmd /C` is used  
**Linux/macOS:** `sh -c` is used

### 8.3 Requirements

- `run.disabled: false` in `scripting.json`
- Command must be in `run.allowlist`

### 8.4 Allowlist Rules

| Entry Type | Example | Behavior |
|------------|---------|----------|
| Exec name | `hostname`, `cmd`, `python` | Matches executable name |
| Path prefix | `C:\Scripts\` | Matches if command starts with path |

Examples:
```json
"allowlist": "hostname,cmd,python,C:\\Scripts\\"
```

### 8.5 Examples

**Hostname:**
```
{run:hostname}
```

**With args:**
```
{run:cmd /c echo %date%}
```

**PowerShell script:**
```
{run:C:\Scripts\myscript.ps1}
```

### 8.6 Error Messages

| Message | Meaning |
|---------|---------|
| `[Run disabled by config]` | `run.disabled` is true |
| `[Run blocked: not in allowlist]` | Command not in allowlist |
| `[Run Error: ...]` | Execution failed |

---

## 9. Script Library

The **Script Library** tab provides collapsible sections for each enabled script type. Edit global libraries and save to disk; changes apply on next expansion.

### 9.1 JavaScript Library

**Location:** `%APPDATA%\DigiCore\scripts\global_library.js` (configurable via `js.library_path`)

**Purpose:** Define reusable JavaScript functions available in all `{js:...}` tags.

**Default content** (when file is missing): `greet`, `getTimeGreeting`, `clipClean`, `mathRound`, `guiTest`.

**Example:**
```javascript
function greet(name) {
    return "Hello, " + name + "!";
}
function getTimeGreeting() {
    var hour = new Date().getHours();
    if (hour < 12) return "Good Morning";
    if (hour < 18) return "Good Afternoon";
    return "Good Evening";
}
```

**Usage:** `{js: greet("World") }` | `{js: getTimeGreeting() }`

**Syntax highlighting:** JS keyword highlighting (keywords, strings, comments).

### 9.2 Python Library (when py.enabled)

**Location:** `%APPDATA%\DigiCore\scripts\global_library.py` (configurable via `py.library_path`)

**Visibility:** Collapsible section appears in Script Library tab when `py.enabled: true` in `scripting.json`.

**Purpose:** Define reusable Python functions for `{py:...}` tags. Edit and save via "Save & Reload Python" button.

### 9.3 Lua Library (when lua.enabled)

**Location:** `%APPDATA%\DigiCore\scripts\global_library.lua` (configurable via `lua.library_path`)

**Visibility:** Collapsible section appears in Script Library tab when `lua.enabled: true` in `scripting.json`.

**Purpose:** Define reusable Lua functions for `{lua:...}` tags. Edit and save via "Save & Reload Lua" button.

---

## 10. Error Messages

### 10.1 Quick Reference

| Prefix | Script Type |
|--------|-------------|
| `[JS Error: ...]` | JavaScript |
| `[DSL Error: ...]` | Custom DSL |
| `[Python Error: ...]` | Python |
| `[Lua Error: ...]` | Lua |
| `[HTTP Error: ...]` | HTTP |
| `[Path Error: ...]` | HTTP JSON path |
| `[Run Error: ...]` | Run command |

### 10.2 Disabled by Config

- `[DSL disabled by config]`
- `[Python disabled by config]`
- `[Lua disabled by config]`
- `[Run disabled by config]`, `[Run blocked: not in allowlist]`

---

## 11. Processing Order

Placeholders are processed in this order:

1. Static global replace (`{date}`, `{clipboard}`, `{am/pm}`)
2. `{uuid}`
3. `{random:N}`
4. `{env:VAR}`
5. `{timezone}`, `{tz}`
6. `{time:FORMAT}`
7. `{js:...}`
8. `{http:...}`
9. `{run:...}`
10. `{dsl:...}`, `{py:...}`, `{lua:...}` (with other script types)
11. `{clip:1}`–`{clip:N}`
12. Interactive vars (`{var:}`, `{choice:}`, etc.)

Nested placeholders are resolved recursively (e.g. `{js:{var:x} + 1}`).

---

## 12. Security

### 12.1 Recommendations

| Feature | Recommendation |
|---------|----------------|
| `{run:cmd}` | Keep disabled (`run.disabled: true`) unless needed |
| `{run:cmd}` | Use allowlist when enabled |
| `{http:...}` | Use `url_allowlist` to restrict domains |
| `{js:...}` | Keep `sandbox_enabled: true` |
| `{py:...}`, `{lua:...}` | Enable only when needed |

### 12.2 Sandbox (JS)

- Rejects `eval`, `Function`, `new Function`
- Recursion and loop limits apply
- Execution timeout applies

### 12.3 Run Command

- Disabled by default
- Allowlist required when enabled
- Path prefix or exec name matching only

---

## Appendix A: Quick Reference Card

| Placeholder | Example | Output |
|-------------|---------|--------|
| `{js:expr}` | `{js: 10 + 20}` | `30` |
| `{dsl:expr}` | `{dsl: round(3.7)}` | `4` |
| `{py:expr}` | `{py: "hi".upper()}` | `HI` |
| `{lua:code}` | `{lua: print(1+2)}` | `3` |
| `{http:url}` | `{http:https://api.ipify.org}` | IP string |
| `{http:url\|path}` | `{http:url\|ip}` | JSON value |
| `{run:cmd}` | `{run:hostname}` | stdout |

---

## Appendix B: File Locations

| Item | Path |
|------|------|
| Config | `%APPDATA%\DigiCore\config\scripting.json` |
| Global JavaScript Library | `%APPDATA%\DigiCore\scripts\global_library.js` |
| Global Python Library | `%APPDATA%\DigiCore\scripts\global_library.py` (when py.enabled) |
| Global Lua Library | `%APPDATA%\DigiCore\scripts\global_library.lua` (when lua.enabled) |
| Config root | `%APPDATA%\DigiCore` |

---

*End of Scripting User Guide*
