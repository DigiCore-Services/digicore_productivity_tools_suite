import re
import json
import os

def parse_ahk_file(file_path):
    snippets = {}
    
    with open(file_path, 'r', encoding='utf-8') as f:
        content = f.read()

    # Pattern for simple hotstrings: :options:trigger::expansion
    # This handles :*:trigger::expansion
    # We use a non-greedy .*? for the trigger and handle newlines in expansion if escaped with `n
    pattern = re.compile(r'^:(?P<options>[^:]*):(?P<trigger>[^:]+)::(?P<expansion>.*)$', re.MULTILINE)
    
    for match in pattern.finditer(content):
        trigger = match.group('trigger').strip()
        options = match.group('options').strip()
        expansion = match.group('expansion').strip()
        
        # Basic comment cleaning if any exist at the end of the line (simple check)
        if ' ; ' in expansion:
            expansion = expansion.split(' ; ')[0].strip()
            
        snippets[trigger] = {
            'content': expansion,
            'options': options,
            'source': os.path.basename(file_path)
        }

    # Handle multi-line variable pattern (simplified)
    # Variable assignment: varName := "content"
    # Followed by: :*:trigger::SendRaw, %varName%
    var_pattern = re.compile(r'^(?P<varName>\w+)\s*:=\s*"(?P<varContent>.*?)"', re.DOTALL | re.MULTILINE)
    vars = {}
    for match in var_pattern.finditer(content):
        vars[match.group('varName')] = match.group('varContent').replace('`n', '\n')

    # Look for hotstrings that use these variables
    send_pattern = re.compile(r'^:(?P<options>[^:]*):(?P<trigger>[^:]+)::\s*Send(?:Raw|Input|Play)?,\s*%(?P<varName>\w+)%', re.MULTILINE)
    for match in send_pattern.finditer(content):
        trigger = match.group('trigger').strip()
        var_name = match.group('varName')
        if var_name in vars:
            snippets[trigger] = {
                'content': vars[var_name],
                'options': match.group('options').strip(),
                'source': os.path.basename(file_path)
            }

    return snippets

def merge_libraries(lib1, lib2):
    merged = {}
    duplicates = []
    
    all_triggers = set(lib1.keys()) | set(lib2.keys())
    
    for trigger in all_triggers:
        item1 = lib1.get(trigger)
        item2 = lib2.get(trigger)
        
        if item1 and item2:
            if item1['content'] == item2['content'] and item1['options'] == item2['options']:
                # Exact duplicate, take either
                merged[trigger] = item1
            else:
                # Conflict
                duplicates.append({
                    'trigger': trigger,
                    'file1': item1,
                    'file2': item2
                })
                # Add to merged with a conflict marker for now
                merged[trigger] = item1 # Default to file 1 for the merged list
        elif item1:
            merged[trigger] = item1
        else:
            merged[trigger] = item2
            
    return merged, duplicates

def get_category(trigger, content):
    trigger = trigger.lower()
    
    # Archiving & Versioning
    if trigger.startswith('/dt'):
        return "Archiving & Versioning"
    
    # API & Cloud Services
    if trigger.startswith('/api'):
        return "API & Cloud Services"
    
    # Documentation & Metadata
    if any(trigger.startswith(x) for x in ['/fm', '/com', '/fpb', '/td', '/ws', '/bob', '/rfc', '/rfz', '/trnc', '/ztoc', '/ohi']):
        return "Documentation & Metadata"
    
    # System & File Utilities
    if any(trigger.startswith(x) for x in ['/utz', '/fahk', '/bws', '/mt', '/of', '/usr', '/scr', '/srv', '/psr', '/pipi']):
        return "System & File Utilities"
    
    # Formatting & Decoration
    if any(trigger.startswith(x) for x in ['/--', '/==', '/sep', '/fs', '/vp', '/.v', '/zsep']):
        return "Formatting & Decoration"
        
    # AI Engineering Prompts (heuristic: long tech-heavy content)
    ai_keywords = ['refactor', 'modular', 'robust', 'solid', 'dry', 'srp', 'microservice', 'prompt', 'codebase', 'syntax']
    if len(content) > 100 and any(kw in content.lower() for kw in ai_keywords):
        return "AI Engineering Prompts"
        
    # Tokens
    if trigger.startswith('/p') and len(trigger) <= 3:
        return "Expansion Tokens"

    return "General Snippets"

def main():
    file1_path = r'C:\Users\pinea\Scripts\AHK_AutoHotKey\AHK_-_PROD-MAIN_STARTUP-SCRIPTZ\ACTIVE-Prod-LIVE-Scripts\2-VERIFY_-_AHK_TE_P-LIVE_TextExpansion_Rev1.3a_2024-04-26_18-06-18pm_PST-PDT_AutoHotKey-AHK-SCRIPT.txt'
    file2_path = r'C:\Users\pinea\Scripts\AHK_AutoHotKey\AHK_-_PROD-MAIN_STARTUP-SCRIPTZ\ACTIVE-Prod-LIVE-Scripts\AHK_TE_P-LIVE_TextExpansion_Rev1.3_2024-04-26_18-06-18pm_PST-PDT_AutoHotKey-AHK-SCRIPT.ahk'
    existing_lib_path = r'C:\Users\pinea\Scripts\AHK_AutoHotKey\AHK_-_PROD-MAIN_STARTUP-SCRIPTZ\ACTIVE-Prod-LIVE-Scripts\text_expansion_library.json'
    
    lib1 = parse_ahk_file(file1_path)
    lib2 = parse_ahk_file(file2_path)
    
    # Resolution strategy: File 1 (Rev1.3a) wins conflicts
    merged_legacy, conflicts = merge_libraries(lib1, lib2)
    
    # Load existing library
    try:
        with open(existing_lib_path, 'r', encoding='utf-8-sig') as f:
            existing_data = json.load(f)
    except Exception as e:
        print(f"Error loading existing library: {e}")
        # Initialize empty if file is missing or corrupt
        existing_data = {"Categories": {}}
    
    # Flatten existing snippets for comparison
    existing_snippets = {}
    for cat, items in existing_data.get("Categories", {}).items():
        for item in items:
            existing_snippets[item['trigger']] = item

    # Final Categories Map
    final_library = {} # category -> list of items
    
    all_combined_triggers = set(merged_legacy.keys()) | set(existing_snippets.keys())
    
    for trigger in sorted(all_combined_triggers):
        # Prefer existing snippets if they exist (they are "live" and potentially edited)
        # But for the legacy merge, if it's new, it goes into the new categorization
        
        item = None
        if trigger in existing_snippets:
            item = existing_snippets[trigger]
            # Content might be updated in the new system, keep it
        else:
            legacy_item = merged_legacy[trigger]
            item = {
                "trigger": trigger,
                "content": legacy_item['content'],
                "options": "*:", # Standardizing options
                "category": get_category(trigger, legacy_item['content'])
            }
            
        cat = item['category']
        if cat not in final_library:
            final_library[cat] = []
        
        # Avoid duplicate objects in the final list
        final_library[cat].append({
            "trigger": item['trigger'],
            "content": item['content'],
            "options": item['options'],
            "category": item['category']
        })

    # Wrap in expected JSON structure
    output_data = {"Categories": final_library}
    
    # Write finalized library
    with open(existing_lib_path, 'w', encoding='utf-8') as f:
        json.dump(output_data, f, indent=2)
        
    print(f"Migration complete!")
    print(f"Total snippets in unified library: {len(all_combined_triggers)}")
    print(f"Categorized across {len(final_library)} categories.")

if __name__ == "__main__":
    main()
