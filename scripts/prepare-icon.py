from PIL import Image
from pathlib import Path

src = Path(r"C:\Users\EDY\.cursor\projects\e-WorkSpace-mihomo\assets\aclash-icon-v1.jpg")
out = Path(r"E:\WorkSpace\mihomo\src-tauri\app-icon.png")

img = Image.open(src).convert("RGBA")
w, h = img.size
side = max(w, h)
canvas = Image.new("RGBA", (side, side), (245, 241, 222, 255))
canvas.paste(img, ((side - w) // 2, (side - h) // 2))
canvas = canvas.resize((1024, 1024), Image.Resampling.LANCZOS)
canvas.save(out, "PNG")
print(f"{out} {canvas.size}")
