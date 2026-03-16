"""Convert PNG to ICO with multiple sizes for Windows app icon.

Usage: Place icon.png in tauri-app/ and run: python scripts/png-to-ico.py
Or: Copy your PNG to tauri-app/icon-temp.png and run the script.
"""
import os
import sys

try:
    from PIL import Image
except ImportError:
    print("Install Pillow: pip install Pillow")
    sys.exit(1)

SIZES = [(16, 16), (32, 32), (48, 48), (256, 256)]

def main():
    script_dir = os.path.dirname(os.path.abspath(__file__))
    tauri_root = os.path.dirname(script_dir)
    png_path = os.path.join(tauri_root, "icon-temp.png")
    if not os.path.exists(png_path):
        png_path = os.path.join(tauri_root, "icon.png")
    if not os.path.exists(png_path):
        print(f"PNG not found: {png_path}")
        sys.exit(1)

    out_dir = os.path.join(script_dir, "..", "src-tauri", "icons")
    os.makedirs(out_dir, exist_ok=True)
    ico_path = os.path.join(out_dir, "icon.ico")

    img = Image.open(png_path).convert("RGBA")
    # Composite onto white - transparent areas can render as dark on Windows
    bg = Image.new("RGBA", img.size, (255, 255, 255, 255))
    img = Image.alpha_composite(bg, img)
    img.save(ico_path, format="ICO", sizes=SIZES)
    print(f"Created {ico_path}")

if __name__ == "__main__":
    main()
