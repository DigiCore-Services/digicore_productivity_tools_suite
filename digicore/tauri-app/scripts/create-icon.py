"""Create a minimal valid icon.ico for Tauri Windows build. No external deps."""
import struct
import os

# ICO: 16x16 32bpp ARGB
W, H = 16, 16
header = struct.pack("<HHH", 0, 1, 1)  # reserved, type=ICO, count=1
entry = struct.pack(
    "BBBBHHII",
    W, H, 0, 0, 1, 32,  # width, height, colors, reserved, planes, bpp
    40 + W * H * 4 + (W * H // 8),  # size
    6 + 16,  # offset
)
# BITMAPINFOHEADER
bmi = struct.pack(
    "<IIIHHIIIIII",
    40, W, H * 2, 1, 32, 0, W * H * 4, 0, 0, 0, 0
)
# 32bpp ARGB image (blue square, bottom-up)
pixels = b""
for y in range(H - 1, -1, -1):
    for x in range(W):
        # Simple blue (#2563eb) with full alpha - BMP/ICO uses BGRA order
        r, g, b, a = 0x25, 0x63, 0xeb, 0xff
        pixels += struct.pack("BBBB", b, g, r, a)
# AND mask (all transparent)
and_mask = b"\x00" * (W * H // 8)

out_dir = os.path.join(os.path.dirname(__file__), "..", "src-tauri", "icons")
os.makedirs(out_dir, exist_ok=True)
path = os.path.join(out_dir, "icon.ico")
with open(path, "wb") as f:
    f.write(header + entry + bmi + pixels + and_mask)
print(f"Created {path}")
