import json
import os

library_path = r"C:\Users\pinea\Scripts\AHK_AutoHotKey\AHK_-_PROD-MAIN_STARTUP-SCRIPTZ\ACTIVE-Prod-LIVE-Apps\Text-Expansion\text_expansion_library.json"

if not os.path.exists(library_path):
    print(f"Error: {library_path} not found.")
    exit(1)

with open(library_path, 'r', encoding='utf-8') as f:
    data = json.load(f)

# Update all entries in all categories
for category, snippets in data.get("Categories", {}).items():
    for snippet in snippets:
        if "profile" not in snippet:
            # Set DevOps snippets to 'Work', others to 'Default'
            if category == "DevOps / Docker":
                snippet["profile"] = "Work"
            else:
                snippet["profile"] = "Default"

# Write back to file
with open(library_path, 'w', encoding='utf-8') as f:
    json.dump(data, f, indent=2, ensure_ascii=False)

print("Successfully updated all snippets with 'profile' field.")
