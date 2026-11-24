"""Bounded local ZIP/folder/manifest selection. No hardware or network access."""
import json, shutil, stat, zipfile, os
from pathlib import Path, PurePosixPath
import current_bundle as bundle
import updater as u
import arcade_bundle as arcade
MAX_BYTES=20*1024*1024

def find_manifest(root):
    candidates=[];visited=0
    for directory, dirs, files in os.walk(root,followlinks=False):
        visited+=len(dirs)+len(files)
        u.require(visited<=512 and len(Path(directory).relative_to(root).parts)<=5,'Select the extracted firmware folder or its exact manifest, not a source-code folder')
        for name in files:
            if name not in ('manifest.json','development-menu.json','release.json'):continue
            path=Path(directory)/name
            u.require(not path.is_symlink(),'Release manifest symlinks are not supported')
            m=bundle.read_json(path)
            if isinstance(m,dict) and (('firmware' in m and 'assets' in m) or m.get('base')=='solid-loading-v0.2.0' or m.get('format')==arcade.FORMAT):candidates.append(path)
    u.require(len(candidates)==1,'Select the firmware release ZIP, or the exact release manifest; one matching manifest is required')
    return candidates[0]

def select(source, output, release):
    source=Path(source).resolve();output=Path(output)
    u.require(source.exists() and not output.exists(),'Release not found or cache directory already exists')
    output.mkdir(parents=True)
    try:
        if source.is_file() and source.suffix.lower()=='.zip':
            u.require(source.stat().st_size<=1024*1024*1024,'Release ZIP is too large')
            expanded=output/'unpacked';expanded.mkdir()
            with zipfile.ZipFile(source) as z:
                entries=z.infolist();u.require(len(entries)<=8192,'Too many files in release ZIP')
                seen=set();manifests=[]
                for entry in entries:
                    name=entry.filename.rstrip('/')
                    u.require(name and name.casefold() not in seen,'Duplicate release ZIP entry');seen.add(name.casefold())
                    bundle.inside(expanded,name)
                    u.require(not stat.S_ISLNK(entry.external_attr>>16),'ZIP symlinks are not supported')
                    if Path(name).name in ('manifest.json','development-menu.json','release.json') and not entry.is_dir():
                        u.require(entry.file_size<=65536,'Release metadata is too large')
                        m=json.loads(z.read(entry))
                        if isinstance(m,dict) and (('firmware' in m and 'assets' in m) or m.get('base')=='solid-loading-v0.2.0' or m.get('format')==arcade.FORMAT):
                            manifests.append((entry,m))
                u.require(len(manifests)==1,'Select the firmware release ZIP, or the exact release manifest; one matching manifest is required')
                entry,m=manifests[0];prefix=PurePosixPath(entry.filename).parent
                if m.get('format')==arcade.FORMAT:
                    records=[m.get('firmware')]+m.get('video_images',[])
                elif 'assets' in m:records=[dict(m['firmware'],file=m['firmware'].get('file','nanoblast-firmware.bin'))]+m['assets']
                else:
                    companion=z.getinfo(str(prefix/'core-manifest.json'))
                    u.require(companion.file_size<=65536,'Release metadata is too large')
                    records=[m]+json.loads(z.read(companion))['assets']
                needed={entry.filename}
                if m.get('base')=='solid-loading-v0.2.0':needed.add(str(prefix/'core-manifest.json'))
                u.require(len(records)<=33 and all(isinstance(r,dict) for r in records),'Invalid release file records')
                for record in records:
                    bundle.inside(expanded/prefix,record.get('file'))
                    needed.add(str(prefix/record['file']))
                total=0
                for name in needed:
                    item=z.getinfo(name)
                    total+=item.file_size
                    u.require(total<=40*1024*1024 and item.file_size<=1024*1024 and not item.is_dir(),'Selected release files exceed size limits')
                    path=bundle.inside(expanded,name);path.parent.mkdir(parents=True,exist_ok=True)
                    with z.open(item) as f:
                        data=f.read(1024*1024+1);u.require(len(data)==item.file_size,'Invalid ZIP entry length')
                    path.write_bytes(data)
                manifest=expanded/entry.filename
        elif source.is_dir():manifest=find_manifest(source)
        else:
            u.require(source.suffix.lower()=='.json','Choose a release ZIP, extracted release folder or JSON manifest')
            manifest=source
        root=manifest.parent.resolve()
        metadata=bundle.read_json(manifest)
        if metadata.get('format')==arcade.FORMAT:
            m=arcade.describe(root,manifest)
            arcade.load(root,release,manifest=manifest)
            target=output/'package';target.mkdir()
            (target/'release.json').write_text(json.dumps(m,indent=2)+'\n')
            for record in [m['firmware']]+m['video_images']:
                src=bundle.inside(root,record['file']);dst=bundle.inside(target,record['file'])
                dst.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(src,dst)
            arcade.load(target,release)
            return dict(version=m['version'],path=str(target.resolve()),source=source.name,bytes=m['firmware']['bytes'],
                        sha256=m['firmware']['sha256'],cores=0,kind='arcade',layout='Arcade V1 / application only',
                        video_images=len(m['video_images']),hardware_tested=m.get('hardware_tested') is True)
        version,menu,assets=bundle.describe(root,manifest)
        # Validate the source before copying, then validate the private copy too.
        bundle.load(root,release,manifest=manifest)
        target=output/'package';target.mkdir()
        normalized=dict(version=version,core_format='NBCRXZ01',firmware=menu,assets=assets)
        (target/'manifest.json').write_text(json.dumps(normalized,indent=2)+'\n')
        for record in [menu]+assets:
            src=bundle.inside(root,record['file']);dst=bundle.inside(target,record['file'])
            dst.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(src,dst)
        changes,_=bundle.load(target,release)
        return dict(version=version,path=str(target.resolve()),source=source.name,bytes=menu['bytes'],sha256=menu['sha256'],cores=len(assets),layout='BL616 / NBCRXZ01')
    except Exception:
        shutil.rmtree(output)
        raise
