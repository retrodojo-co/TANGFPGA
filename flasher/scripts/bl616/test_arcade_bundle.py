"""Synthetic Arcade packages and flash captures; no board, game data or network."""
import hashlib
import json
from pathlib import Path
import struct
import tempfile
import unittest
from unittest.mock import patch
import zipfile
import zlib
import arcade_bundle as a
import release_files
import updater as u


def firmware(payload):
    data = bytearray(4096) + payload
    data[:4] = b'BFNP'
    struct.pack_into('<I', data, 124, 4096)
    struct.pack_into('<I', data, 132, len(payload))
    data[136:168] = hashlib.sha256(payload).digest()
    for offset, start, end in [(96,12,96),(116,104,116),(252,0,252)]:
        struct.pack_into('<I', data, offset, zlib.crc32(data[start:end]))
    return bytes(data)


class Arcade(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.bundle = self.root/'release'
        self.bundle.mkdir()
        self.fw = firmware(b'Arcade target application' * 16)
        self.old = firmware(b'Arcade previous application' * 16)
        self.videos = []
        for i, name in enumerate(sorted(a.VIDEO_PATHS)):
            data = bytes([i])*907418
            path = self.bundle/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(data)
            self.videos.append(dict(file=name,bytes=len(data),sha256=u.sha(data)))
        self.core = (self.bundle/self.videos[0]['file']).read_bytes()
        path=self.bundle/'Firmware/app.bin';path.parent.mkdir();path.write_bytes(self.fw)
        self.manifest = dict(format=a.FORMAT,version='synthetic-r1',systems=['system16a','system16b','cps1'],
            boot_marker_protocol=28,hardware_tested=False,
            firmware=dict(file='Firmware/app.bin',bytes=len(self.fw),sha256=u.sha(self.fw),offset=a.START),video_images=self.videos)
        self.save_manifest()
        self.before = bytearray(b'\x55'*u.SIZE)
        self.before[a.START:a.END] = self.old+b'\xff'*(a.END-a.START-len(self.old))
        self.before[0x100000:0x100008] = b'NBCACHE1'
        crc=zlib.crc32(self.core)
        struct.pack_into('<II',self.before,0x100008,len(self.core),crc)
        self.before[0x101000:0x101000+len(self.core)] = self.core
        marker=bytearray(32);marker[:8]=b'S16SAVE1';struct.pack_into('<II',marker,8,28,crc)
        struct.pack_into('<I',marker,28,zlib.crc32(marker[:28]))
        self.before[0x100040:0x100060] = marker
        self.known = patch.object(a,'KNOWN_APPLICATIONS',{u.sha(self.old)})
        self.known.start();self.addCleanup(self.known.stop)

    def save_manifest(self):
        (self.bundle/'release.json').write_text(json.dumps(self.manifest))

    def test_application_only_preservation_and_idempotence(self):
        changes, plan = a.load(self.bundle)
        after = plan(bytes(self.before),changes)
        self.assertEqual([(p,len(d)) for p,d in changes],[(a.START,a.END-a.START)])
        self.assertEqual(after[:a.START],self.before[:a.START])
        self.assertEqual(after[a.END:],self.before[a.END:])
        self.assertEqual(after[a.START:a.START+len(self.fw)],self.fw)
        self.assertEqual(plan(after,changes),after)
        with self.assertRaisesRegex(ValueError,'request'):plan(bytes(self.before),changes+[(0,b'x')])

    def test_unknown_application_blank_board_and_invalid_caches_refused(self):
        changes, plan = a.load(self.bundle)
        captures=[bytes(u.SIZE)]
        bad=bytearray(self.before);bad[a.START:a.START+len(self.old)]=firmware(b'unknown application'.ljust(len(self.old)-4096,b'!'));captures.append(bytes(bad))
        for offset in (0x100000,0x101111,0x100040):
            bad=bytearray(self.before);bad[offset]^=1;captures.append(bytes(bad))
        bad=bytearray(self.before);struct.pack_into('<I',bad,0x100048,27)
        struct.pack_into('<I',bad,0x10005c,zlib.crc32(bad[0x100040:0x10005c]));captures.append(bytes(bad))
        for before in captures:
            with self.subTest(capture=u.sha(before)):
                with self.assertRaises(ValueError):plan(before,changes)

    def test_target_validation_rejects_wrong_offset_protocol_and_corrupt_files(self):
        for key,value in [('offset',0),('bytes',True)]:
            original=self.manifest['firmware'][key];self.manifest['firmware'][key]=value;self.save_manifest()
            with self.assertRaises(ValueError):a.load(self.bundle)
            self.manifest['firmware'][key]=original
        self.manifest['boot_marker_protocol']=27;self.save_manifest()
        with self.assertRaises(ValueError):a.load(self.bundle)
        self.manifest['boot_marker_protocol']=28;self.save_manifest()
        for name in ['Firmware/app.bin',self.videos[0]['file']]:
            p=self.bundle/name;original=p.read_bytes();p.write_bytes(b'x'+original[1:])
            with self.assertRaises(ValueError):a.load(self.bundle)
            p.write_bytes(original)

    def test_receipts_allow_later_updates_without_trusting_console_history(self):
        receipt=self.root/'verified-release-layouts.json'
        receipt.write_text(json.dumps([{'menu_sha256':'0'*64,'bank_sha256':'1'*64}]))
        changes,plan=a.load(self.bundle)
        installed=plan(bytes(self.before),changes)
        a.remember(receipt,changes)
        self.assertIn(u.sha(self.fw),a.receipts(receipt))
        target=firmware(b'future Arcade application'*16)
        (self.bundle/'Firmware/app.bin').write_bytes(target)
        self.manifest['firmware'].update(bytes=len(target),sha256=u.sha(target));self.save_manifest()
        changes,next_plan=a.load(self.bundle,trusted_receipts=receipt)
        self.assertEqual(next_plan(installed,changes)[a.START:a.START+len(target)],target)
        with self.assertRaises(ValueError):a.load(self.bundle)[1](installed,changes)
        self.assertIsInstance(json.loads(receipt.read_text())[0],dict)

    def test_zip_selects_only_application_and_recovery_reference_images(self):
        archive=self.root/'private.zip'
        with zipfile.ZipFile(archive,'w',zipfile.ZIP_DEFLATED) as z:
            for p in self.bundle.rglob('*'):
                if p.is_file():z.write(p,'Arcade/'+str(p.relative_to(self.bundle)))
            z.writestr('Arcade/SD-card/cps1/irrelevant-game.zip',b'not needed'*1024*1024)
            z.writestr('Arcade/Source.zip',b'not executable'*1024*1024)
            z.writestr('Arcade/Updater/flash.py','raise Exception("must never execute")')
        result=release_files.select(archive,self.root/'cache',self.bundle)
        self.assertEqual(result['kind'],'arcade');self.assertEqual(result['cores'],0)
        selected=Path(result['path'])
        self.assertEqual(len(list(selected.rglob('*.bin'))),10)
        self.assertFalse(list(selected.rglob('*.zip')));self.assertFalse(list(selected.rglob('*.py')))
        (self.bundle/'Firmware/app.bin').write_bytes(b'changed')
        a.load(selected)

    def test_wrong_and_unsafe_zip_is_rejected_without_leaving_cache(self):
        for name in ('../release.json','C:/release.json','Arcade/../release.json'):
            archive=self.root/'bad.zip'
            with zipfile.ZipFile(archive,'w') as z:z.writestr(name,json.dumps(self.manifest))
            with self.assertRaises(ValueError):release_files.select(archive,self.root/'cache',self.bundle)
            self.assertFalse((self.root/'cache').exists())

    def test_full_readback_and_failed_plan_never_write_outside_application(self):
        class MemoryTransport:
            def __init__(self,data):self.data=bytearray(data);self.writes=[]
            def read(self,start,end,path):return bytes(self.data[start:end])
            def write(self,start,data,path):
                if not a.START<=start<start+len(data)<=a.END:raise AssertionError('protected write')
                self.writes.append((start,len(data)));self.data[start:start+len(data)]=data
        changes,plan=a.load(self.bundle)
        transport=MemoryTransport(self.before);out=self.root/'backup';out.mkdir()
        u.install(transport,changes,out,plan)
        self.assertTrue(transport.writes)
        self.assertEqual(json.loads((out/'result.json').read_text())['status'],'FULL_FLASH_READBACK_PASS')
        self.assertEqual((out/'before-full-flash.bin').read_bytes(),self.before)
        bad=MemoryTransport(bytes(u.SIZE));out=self.root/'bad-backup';out.mkdir()
        with self.assertRaises(ValueError):u.install(bad,changes,out,plan)
        self.assertFalse(bad.writes);self.assertFalse((out/'result.json').exists())
        self.assertTrue((out/'before-full-flash.bin').exists())

if __name__=='__main__':unittest.main()
