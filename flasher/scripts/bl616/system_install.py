"""Explicit three-step Nano 20K installation. Never erases chips or writes efuses."""
import hashlib,json,lzma,os,re,shutil,struct,subprocess,sys,uuid,zlib
from pathlib import Path
import updater as u
import current_bundle as console
import arcade_bundle as arcade
import release_files
PRIMARY_SHA='bc5b1f733a13562c898ff66b540038c116c3d77ca202d0afebd1e7af1c3b3203'
PRIMARY_BYTES=90672
FPGA_SIZE=0x800000
BOOT_BYTES=907418
BOOT_SPAN=0xe0000
PROTECTED=[(0,0x40000),(0x1df000,0x201000),(0x3ff000,u.SIZE)]


def save(path,record):
    temp=path.with_name(path.name+'.'+uuid.uuid4().hex+'.tmp')
    u.durable(temp,(json.dumps(record,indent=2)+'\n').encode());os.replace(temp,path)


def footer(data):
    u.require(len(data)==BOOT_BYTES,'Unsupported Nano 20K boot image size')
    p=data[-30:]
    u.require(p[0]==10 and p[1:6]==bytes(5) and p[8:16]==b'\xff'*8 and p[16]==8 and p[17:20]==bytes(3) and p[20:]==b'\xff'*10,'Invalid Nano 20K configuration footer')


def primary(resources):
    data=(resources/'installer/bl616_fpga_partner_20kNano.bin').read_bytes()
    u.require(len(data)==PRIMARY_BYTES and u.sha(data)==PRIMARY_SHA,'Vendor programmer integrity check failed')
    return data+b'\xff'*(0x40000-len(data))


def profile(folder,release):
    folder=Path(folder)
    if (folder/'release.json').exists():
        m=arcade.describe(folder);arcade.load(folder,release)
        image=(folder/'SD-card/cps1/video/cps1/480p60.bin').read_bytes();footer(image)
        cache=bytearray(b'\xff'*0xdf000);crc=zlib.crc32(image)
        cache[:8]=b'NBCACHE1';struct.pack_into('<II',cache,8,len(image),crc)
        marker=bytearray(b'S16SAVE1'+bytes(24));struct.pack_into('<5I',marker,8,28,crc,0x43505331,0x56494400,BOOT_BYTES)
        struct.pack_into('<I',marker,28,zlib.crc32(marker[:28]));cache[64:96]=marker;cache[4096:4096+len(image)]=image
        fw=(folder/m['firmware']['file']).read_bytes()
        changes=[(0x201000,b'\xff'*0x1fe000),(0x100000,bytes(cache)),(0xe8000,b'\xff'*0x18000),(0x40000,fw+b'\xff'*(0xa8000-len(fw)))]
        return 'arcade',m['version'],image,changes
    version,menu,assets=console.describe(folder)
    checked,_=console.load(folder,release)
    record=next(a for a in assets if a['core_id']==1)
    data=(folder/record['file']).read_bytes();u.require(u.sha(data)==record['sha256'],'NES boot image checksum mismatch')
    count=struct.unpack_from('<I',data,28)[0];image=bytearray()
    for i in range(count):
        off,size=struct.unpack_from('<II',data,64+i*8);stored=bool(size&0x80000000);size&=0x7fffffff
        chunk=data[off:off+size]
        if stored:image+=chunk
        else:
            decoder=lzma.LZMADecompressor(memlimit=2*1024*1024)
            decoded=decoder.decompress(chunk,max_length=65537)
            u.require(decoder.eof and not decoder.unused_data and len(decoded)<=65536,'Invalid NES boot block');image+=decoded
    image=bytes(image);footer(image)
    u.require(u.sha(image)==record.get('raw_sha256',record.get('image_sha256')),'Invalid NES boot image')
    return 'console',version,image,[checked[0],(0x100000,b'\xff'*0xdf000),(0xe8000,b'\xff'*0x18000),checked[1]]


class PrimaryTransport(u.Transport):
    def write(self,start,data,path):
        u.require(0<=start<start+len(data)<=0x40000 and start%4096==0 and len(data)%4096==0,'Primary write outside confirmed programmer slot')
        path.write_bytes(data)
        self.perform(['--write','--flash','--addr',hex(start),'--file',str(path)],True)


def request(path):
    path=Path(path).resolve();m=console.read_json(path)
    u.require(m.get('format')=='TANGFPGA_INSTALL_V1' and m.get('target') in ('arcade','console'),'Invalid installation request')
    u.require(path.name=='system-install.json','Invalid installation record path')
    image=(path.parent/'boot-image.bin').read_bytes();footer(image)
    u.require(u.sha(image)==m['boot_sha256'],'Installation boot image changed')
    return path,m,image


def prepare(args,transport_class=PrimaryTransport,run_command=None):
    u.require(args.release and args.current_bundle and args.output and args.port and args.identity,'Missing first-install selection')
    kind,version,image,_=profile(args.current_bundle,args.release)
    u.require(args.confirmation==f'INSTALL {kind.upper()} NANO20K {args.port}','Explicit Nano 20K installation confirmation required')
    u.check_port(args.port,args.identity)
    bootloader=primary(args.release.parent)
    executable=loader(args.release.parent)
    info=(run_command or command)(executable,['-V'])
    u.require('openFPGALoader' in info,'Bundled FPGA programmer is unavailable; no flash was changed')
    args.output.mkdir(parents=True,exist_ok=False)
    selected=release_files.select(args.current_bundle,args.output/'selected',args.release)
    kind,version,image,_=profile(Path(selected['path']),args.release)
    u.durable(args.output/'boot-image.bin',image)
    record=dict(format='TANGFPGA_INSTALL_V1',phase='preparing',target=kind,version=version,boot_sha256=u.sha(image),update_identity=args.identity)
    path=args.output/'system-install.json';save(path,record)
    changes=[(0,bootloader)]
    def plan(before,requested):
        u.require(len(before)==u.SIZE and requested==changes,'Invalid primary installation request')
        return bootloader+before[0x40000:]
    u.install(transport_class(args.port,args.identity,args.output),changes,args.output,plan,[(0x40000,u.SIZE)])
    result=console.read_json(args.output/'result.json')
    record.update(phase='primary_verified',controller_before_sha256=result['before_sha256'],controller_prepared_sha256=result['after_sha256'])
    save(path,record);return record


def loader(resources):
    root=resources/'programmer';exe=root/('openFPGALoader.exe' if sys.platform=='win32' else 'openFPGALoader')
    u.require(u.sha(exe.read_bytes())==(root/'sha256.txt').read_text().strip(),'Bundled FPGA programmer integrity failed')
    return exe


def command(exe,args):
    options={'creationflags':subprocess.CREATE_NO_WINDOW} if sys.platform=='win32' else {}
    result=subprocess.run([str(exe)]+args,capture_output=True,text=True,**options)
    print(result.stdout,flush=True);print(result.stderr,file=sys.stderr,flush=True)
    u.require(result.returncode==0,'FPGA programmer failed. Retain both backups and reconnect the same board before retrying.')
    return result.stdout+'\n'+result.stderr


def fpga(args,run_command=command):
    u.require(args.request and args.release and re.fullmatch(r'[0-9]+:[0-9]+',args.busdev or ''),'Select a Nano 20K programmer')
    u.require(args.confirmation==f'FPGA NANO20K {args.busdev}','Explicit FPGA installation confirmation required')
    path,m,image=request(args.request)
    u.require(m['phase'] in ('primary_verified','fpga_pending'),'Installation is not waiting for FPGA programming')
    exe=loader(args.release.parent)
    base=['-b','tangnano20k','--busdev-num',args.busdev,'--external-flash']
    if args.serial:base+=['--ftdi-serial',args.serial]
    detected=run_command(exe,base+['--detect','-f'])
    u.require(any(n in detected for n in ('GW2A(R)-18','GW2AR-18','GW2A-18')) and re.search(r'JEDEC ID:\s*0xef4017\b',detected,re.I),
              'Expected Nano 20K GW2A(R)-18 and Winbond 8 MiB flash; no programming performed')
    root=path.parent;capture=root/('fpga-read-'+uuid.uuid4().hex+'.bin')
    run_command(exe,base+['--dump-flash','--file-size',str(FPGA_SIZE),str(capture)])
    current=capture.read_bytes();u.require(len(current)==FPGA_SIZE,'Incomplete FPGA flash backup')
    original=root/'fpga-before-full-flash.bin'
    if original.exists():
        before=original.read_bytes();u.require(len(before)==FPGA_SIZE and u.sha(before)==m.get('fpga_before_sha256'),'Original FPGA backup changed')
        u.require(current[BOOT_SPAN:]==before[BOOT_SPAN:],'FPGA protected region changed since preparation')
    else:
        before=current;u.durable(original,before);m['fpga_before_sha256']=u.sha(before)
    padded=image+b'\xff'*(BOOT_SPAN-len(image));expected=padded+before[BOOT_SPAN:]
    m['phase']='fpga_pending';save(path,m)
    staged=root/('boot-write-'+uuid.uuid4().hex+'.bin');u.durable(staged,padded)
    if current!=expected:run_command(exe,base+['-f','--verify','--file-type','bin',str(staged)])
    after_path=root/('fpga-after-'+uuid.uuid4().hex+'.bin')
    run_command(exe,base+['--dump-flash','--file-size',str(FPGA_SIZE),str(after_path)])
    after=after_path.read_bytes();u.require(after==expected,'FPGA full-flash readback differs from plan; retain backups')
    m.update(phase='fpga_verified',fpga_after_sha256=u.sha(after));save(path,m);return m


def finish(args,transport_class=u.Transport):
    u.require(args.request and args.release and args.port and args.identity,'Missing controller connection')
    path,m,image=request(args.request)
    u.require(m['phase']=='fpga_verified','FPGA verification is required before installing the application')
    u.require(args.confirmation==f'FINISH {m["target"].upper()} NANO20K {args.port}','Explicit final installation confirmation required')
    u.check_port(args.port,args.identity)
    previous=json.loads(m['update_identity']);current=json.loads(args.identity)
    u.require(not previous[3] or previous[3]==current[3],'UPDATE device serial changed; reconnect the original board')
    kind,version,boot,changes=profile(path.parent/'selected/package',args.release)
    u.require(kind==m['target'] and u.sha(boot)==m['boot_sha256'],'Selected system package changed')
    initial=(path.parent/'before-full-flash.bin').read_bytes()
    u.require(len(initial)==u.SIZE and u.sha(initial)==m['controller_before_sha256'],'Original controller backup missing or changed')
    fpga_before=(path.parent/'fpga-before-full-flash.bin').read_bytes()
    u.require(len(fpga_before)==FPGA_SIZE and u.sha(fpga_before)==m['fpga_before_sha256'],'Original FPGA backup missing or changed')
    primary_image=primary(args.release.parent)
    def plan(before,requested):
        u.require(len(before)==u.SIZE and requested==changes,'Invalid system installation request')
        u.require(before[:0x40000]==primary_image,'Primary programmer changed; restart preparation')
        for start,end in PROTECTED[1:]:u.require(before[start:end]==initial[start:end],'Reserved controller region changed; reconnect the original board')
        after=bytearray(before)
        for start,data in changes:after[start:start+len(data)]=data
        for start,end in PROTECTED:u.require(after[start:end]==before[start:end],'Protected controller bytes changed')
        return bytes(after)
    out=path.parent/('controller-final-'+uuid.uuid4().hex);out.mkdir()
    u.install(transport_class(args.port,args.identity,out),changes,out,plan,PROTECTED)
    result=console.read_json(out/'result.json')
    receipts=getattr(args,'trusted_receipts',None)
    if receipts:
        try:
            if kind=='arcade':arcade.remember(receipts,[changes[-1]])
            else:console.remember(receipts,changes)
        except Exception as error:print(f'WARNING: Installation verified but history could not be saved: {error}',flush=True)
    m.update(phase='complete',controller_after_sha256=result['after_sha256'],final_backup=out.name)
    save(path,m);return m


def run(args):
    if args.system_stage=='inspect':
        u.require(args.release and args.current_bundle and not args.port and not args.identity and not args.output,'Offline package inspection requires no device arguments')
        kind,version,image,changes=profile(args.current_bundle,args.release)
        primary(args.release.parent);exe=loader(args.release.parent)
        info=command(exe,['-V']);u.require('openFPGALoader' in info,'Bundled FPGA programmer identity failed')
        return dict(status='OFFLINE_SYSTEM_PROFILE_PASS',target=kind,version=version,boot_sha256=u.sha(image),primary_sha256=PRIMARY_SHA,
                    controller_write_regions=[[start,start+len(data)] for start,data in changes],protected_regions=PROTECTED)
    if args.system_stage=='prepare':return prepare(args)
    if args.system_stage=='fpga':return fpga(args)
    return finish(args)
