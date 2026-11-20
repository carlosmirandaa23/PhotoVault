from pathlib import Path
from PIL import Image, ImageEnhance, ImageOps
import argparse, csv, hashlib, shutil

EXTS = {'.jpg','.jpeg','.png','.webp','.bmp','.tif','.tiff'}

def sha256(p):
    h=hashlib.sha256()
    with p.open('rb') as f:
        for chunk in iter(lambda:f.read(1024*1024),b''): h.update(chunk)
    return h.hexdigest()

def main():
    parser=argparse.ArgumentParser(description='Genera variantes de fotos para probar PhotoVault sin alterar originales.')
    parser.add_argument('carpeta',type=Path,help='Carpeta con 5 fotografías originales')
    args=parser.parse_args()
    source=args.carpeta.expanduser().resolve()
    if not source.is_dir(): parser.error('La carpeta no existe')
    originals=sorted(p for p in source.iterdir() if p.is_file() and p.suffix.lower() in EXTS)
    if len(originals)!=5: parser.error(f'Se requieren exactamente 5 fotografías compatibles en la carpeta; encontradas: {len(originals)}')
    out=source.parent/(source.name+'_PhotoVault_Test')
    if out.exists(): parser.error(f'La carpeta de salida ya existe: {out}. No se sobrescribirá.')
    out.mkdir()
    rows=[]
    def record(group, kind, path, parent):
        rows.append({'grupo':group,'transformacion':kind,'archivo':path.name,'original':parent.name,'sha256':sha256(path)})
    for i,original in enumerate(originals,1):
        prefix=f'F{i:02d}'
        base=out/f'{prefix}__00_ORIGINAL{original.suffix.lower()}'
        shutil.copy2(original,base);record(prefix,'original',base,original)
        for j,kind in [(1,'COPIA_EXACTA'),(2,'RENOMBRADO_EXACTO')]:
            p=out/f'{prefix}__{j:02d}_{kind}{original.suffix.lower()}'
            shutil.copy2(original,p);record(prefix,kind,p,original)
        with Image.open(original) as im:
            im=ImageOps.exif_transpose(im)
            rgb=im.convert('RGB')
            w,h=rgb.size
            if min(w,h)<40: parser.error(f'{original.name} es demasiado pequeña para recortes fiables')
            def save(j,kind,image,quality=92):
                p=out/f'{prefix}__{j:02d}_{kind}.jpg'
                image.save(p,'JPEG',quality=quality,subsampling=0)
                record(prefix,kind,p,original)
            save(3,'JPEG_Q85',rgb,85)
            save(4,'JPEG_Q40',rgb,40)
            save(5,'ESCALA_50',rgb.resize((max(1,w//2),max(1,h//2)),Image.Resampling.LANCZOS))
            save(6,'ESCALA_25',rgb.resize((max(1,w//4),max(1,h//4)),Image.Resampling.LANCZOS))
            def crop(pct):
                dx=int(w*pct/2);dy=int(h*pct/2)
                return rgb.crop((dx,dy,w-dx,h-dy))
            save(7,'RECORTE_10',crop(.10))
            save(8,'RECORTE_30',crop(.30))
            save(9,'ROTACION_90',rgb.transpose(Image.Transpose.ROTATE_90))
            save(10,'ESPEJO_HORIZONTAL',ImageOps.mirror(rgb))
            save(11,'ESCALA_GRISES',ImageOps.grayscale(rgb).convert('RGB'))
            save(12,'BRILLO_CONTRASTE',ImageEnhance.Contrast(ImageEnhance.Brightness(rgb).enhance(1.25)).enhance(1.2))
    manifest=out/'manifest.csv'
    with manifest.open('w',newline='',encoding='utf-8-sig') as f:
        writer=csv.DictWriter(f,fieldnames=['grupo','transformacion','archivo','original','sha256']);writer.writeheader();writer.writerows(rows)
    print(f'Listo: {len(rows)} imágenes en {out}')
    print(f'Manifiesto: {manifest}')
    print('Resultado esperado con duplicados exactos: 5 grupos de 3 archivos cada uno (original + 2 copias).')
    print('No se modificó ningún archivo de origen.')

if __name__=='__main__': main()