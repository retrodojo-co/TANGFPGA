"""Run in an isolated Python 3.9–3.12 environment with requirements-bl616.txt installed."""
import hashlib
import importlib.metadata
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import bflb_mcu_tool

root=Path(__file__).resolve().parents[1]
out=root/'src-tauri/resources/bl616'
work=Path(tempfile.mkdtemp(prefix='nanoblast-bl616-build-'))
env=dict(os.environ,PYINSTALLER_CONFIG_DIR=str(work/'cache'))
subprocess.run([sys.executable,'-m','PyInstaller','--noconfirm','--onedir','--name','nanoblast-bl616','--distpath',str(out),'--workpath',str(work/'build'),'--specpath',str(work),'--collect-all','bflb_mcu_tool','--collect-all','serial','--paths',str(Path(bflb_mcu_tool.__file__).parent),'--collect-submodules','libs',str(root/'scripts/bl616/updater.py')],env=env,check=True)
exe=out/'nanoblast-bl616'/('nanoblast-bl616.exe' if sys.platform=='win32' else 'nanoblast-bl616')
subprocess.run([str(exe),'--self-check'],check=True)
(out/'sha256.txt').write_text(hashlib.sha256(exe.read_bytes()).hexdigest()+'\n')
licenses=out/'licenses';licenses.mkdir(exist_ok=True)
versions={}
for dist in importlib.metadata.distributions():
    name=dist.metadata['Name'];versions[name]=dist.version
    for file in dist.files or []:
        if any(term in str(file).lower() for term in ['license','copying']) and dist.locate_file(file).is_file():
            dest=licenses/name/str(file).replace('..','_');dest.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(dist.locate_file(file),dest)
(out/'dependencies.json').write_text(json.dumps(versions,indent=2)+'\n')
shutil.copyfile(root/'scripts/bl616/updater.py',out/'updater-source.py')
print('Bundled BL616 helper verified:',exe)
