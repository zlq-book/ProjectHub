"""从已选定的品牌源图生成桌面和网页图标，不调用远程服务。"""
from pathlib import Path
from PIL import Image

workspace = Path(__file__).resolve().parents[1]
source = workspace / 'assets' / 'branding' / 'projecthub-icon-concept.png'
root = workspace / 'src-tauri' / 'icons'
public = workspace / 'public'
root.mkdir(parents=True, exist_ok=True)
public.mkdir(parents=True, exist_ok=True)
with Image.open(source) as original:
    if original.width != original.height:
        raise ValueError('品牌源图必须为正方形')
    image = original.convert('RGBA')
image.save(root / 'icon.png')
for size, name in [(32, '32x32.png'), (128, '128x128.png'), (256, '128x128@2x.png')]:
    image.resize((size, size), Image.Resampling.LANCZOS).save(root / name)
image.save(root / 'icon.ico', sizes=[(16,16), (32,32), (48,48), (64,64), (128,128), (256,256)])
image.resize((128, 128), Image.Resampling.LANCZOS).save(public / 'projecthub-icon.png')
image.resize((32, 32), Image.Resampling.LANCZOS).save(public / 'favicon.png')
image.save(public / 'favicon.ico', sizes=[(16,16), (32,32), (48,48)])
print(f'已生成桌面图标：{root}；网页图标：{public}')
