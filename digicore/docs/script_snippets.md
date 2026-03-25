# Productivity Script Snippets

These snippets are designed to be used within `{js:...}`, `{py:...}`, and `{lua:...}` placeholders in your text expander.

## 🟨 JavaScript (Boa)
*Best for: Regex, simple string transformations, and light logic.*

### 1. URL Cleaner (Strip UTM parameters)
**Trigger Content:**
`{js: result = cleanUrl(clipboard)}`

**Scripting Engine Library content:**
```javascript
/**
 * Removes tracking parameters (UTM) from a URL.
 * @param {string} url - The raw URL string.
 * @returns {string} The cleaned URL.
 */
function cleanUrl(url) {
    return url.split('?')[0];
}
```

### 2. Title Case Converter
**Trigger Content:**
`{js: result = toTitleCase(clipboard)}`

**Scripting Engine Library content:**
```javascript
/**
 * Converts a string to Title Case.
 */
function toTitleCase(str) {
    return str.toLowerCase().split(' ').map(w => w.charAt(0).toUpperCase() + w.substr(1)).join(' ');
}
```

### 3. JSON Prettifier
**Trigger Content:**
`{js: result = prettifyJson(clipboard)}`

**Scripting Engine Library content:**
```javascript
/**
 * Prettifies a JSON string with 2-space indentation.
 */
function prettifyJson(raw) {
    try {
        return JSON.stringify(JSON.parse(raw), null, 2);
    } catch (e) {
        return "[Invalid JSON: " + e.message + "]";
    }
}
```

### 4. Relative Time Formatter
**Trigger Content:**
`{js: result = getRelativeTime(60)}`

**Scripting Engine Library content:**
```javascript
/**
 * Simple relative time (minutes/hours).
 */
function getRelativeTime(minutes) {
    if (minutes < 60) return minutes + " minutes ago";
    let hours = Math.floor(minutes / 60);
    return hours + (hours === 1 ? " hour" : " hours") + " ago";
}
```

---

## 🟦 Python (pyo3)
*Best for: Advanced string manipulation, data processing, and complex logic.*

### 1. Robust Password Generator
**Trigger Content:**
`{py: result = generate_robust_password(16)}`

**Scripting Engine Library content:**
```python
import random
import string

def generate_robust_password(length=16):
    """Generates a secure-style password with letters, digits, and symbols."""
    chars = string.ascii_letters + string.digits + "!@#$%^&*"
    return "".join(random.choice(chars) for _ in range(length))
```

### 2. Clipboard Word/Char Counter
**Trigger Content:**
`{py: result = count_text(clipboard)}`

**Scripting Engine Library content:**
```python
def count_text(text):
    """Analyzes text for word and character counts."""
    words = len(text.split())
    chars = len(text)
    return f"Analysis: {words} words | {chars} characters"
```

### 3. "Leetspeak" Fun
**Trigger Content:**
`{py: result = to_leetspeak(clipboard)}`

**Scripting Engine Library content:**
```python
def to_leetspeak(text):
    """Converts basic vowels/consonants to numbers."""
    mapping = {'a': '4', 'e': '3', 'i': '1', 'o': '0', 's': '5'}
    return "".join(mapping.get(c.lower(), c) for c in text)
```

### 4. Date Calculator (Days from Now)
**Trigger Content:**
`{py: result = days_from_now(7)}`

**Scripting Engine Library content:**
```python
from datetime import datetime, timedelta

def days_from_now(days):
    """Calculates a future date string."""
    future = datetime.now() + timedelta(days=int(days))
    return future.strftime("%Y-%m-%d")
```

### 5. CamelCase to snake_case
**Trigger Content:**
`{py: result = to_snake_case(clipboard)}`

**Scripting Engine Library content:**
```python
import re

def to_snake_case(text):
    """Converts CamelCase or PascalCase to snake_case."""
    s1 = re.sub('(.)([A-Z][a-z]+)', r'\1_\2', text)
    return re.sub('([a-z0-9])([A-Z])', r'\1_\2', s1).lower()
```

---

## 🌙 Lua (mlua)
*Best for: High-speed transformations and isolation.*

### 1. Reverse Text
**Trigger Content:**
`{lua: result = reverse_text(clipboard)}`

**Scripting Engine Library content:**
```lua
function reverse_text(txt)
    -- Simply reverses the string
    return txt:reverse()
end
```

### 2. Clipboard History Formatter
**Trigger Content:**
`{lua: result = format_history(clip_history)}`

**Scripting Engine Library content:**
```lua
function format_history(history)
    -- Formats the last 5 items in clipboard history
    local out = "Recent Clips:\n"
    for i, clip in ipairs(history) do
        if i > 5 then break end
        out = out .. "  [" .. i .. "] " .. clip:sub(1, 40) .. "...\n"
    end
    return out
end
```

### 3. Random Choice Picker
**Trigger Content:**
`{lua: result = pick_random(clipboard)}`

**Scripting Engine Library content:**
```lua
function pick_random(list_str)
    -- Picks a random item from a comma-separated list
    local items = {}
    for item in list_str:gmatch("([^,]+)") do
        table.insert(items, item:trim())
    end
    if #items == 0 then return "[No items found]" end
    math.randomseed(os.time())
    return items[math.random(#items)]
end

-- Helper for trimming
function string:trim()
  return (self:gsub("^%s*(.-)%s*$", "%1"))
end
```

### 4. Simple Lua Template
**Trigger Content:**
`{lua: result = personal_greet("User")}`

**Scripting Engine Library content:**
```lua
function personal_greet(name)
    local hour = tonumber(os.date("%H"))
    local greeting = "Good morning"
    if hour >= 12 and hour < 18 then greeting = "Good afternoon"
    elseif hour >= 18 then greeting = "Good evening" end
    return greeting .. ", " .. name .. "! The time is " .. os.date("%H:%M") .. "."
end
```

---

## 💡 Pro Tips for Testing
1. **Variable Assignment**: In our engines, always ensure you assign the final string to a variable named `result` for JavaScript and Python to capture the output correctly.
2. **Clipboard Access**: Use the `clipboard` variable to get the current system clipboard.
3. **History Access**: Use the `clip_history` array (index 0 is most recent) to build snippets that merge multiple past copies.
