"""Complete first-install/conversion models for both flash chips; no hardware."""
from pathlib import Path
from types import SimpleNamespace
import json,struct,tempfile,unittest
from unittest.mock import patch
import system_install as s
import updater as u
import test_arcade_bundle as fixtures
ROOT=Path(__file__).resolve().parents[2]
RELEASE=ROOT/'src-tauri/resources/nanoblast-v0.2.0'

class Install(unittest.TestCase):
    save_manifest=fixtures.Arcade.save_manifest
    def setUp(self):
        fixtures.Arcade.setUp(self)
        footer=b'\x0a'+bytes(5)+b'\x12\x34'+b'\xff'*8+b'\x08'+bytes(3)+b'\xff'*10
        for record in self.videos:
            p=self.bundle/record['file'];data=p.read_bytes()[:-30]+footer;p.write_bytes(data);record['sha256']=u.sha(data)
        self.save_manifest()
        self.mcu=bytearray(b'\xff'*u.SIZE);self.mcu[0x1df000:0x201000]=b'R'*0x22000;self.mcu[0x3ff000:]=b'C'*4096
        self.fpga=bytearray(b'\x5a'*s.FPGA_SIZE);self.commands=[];self.port='TEST';self.identity=json.dumps(['TEST',13467,24928,'board-one','slot'])
        owner=self
        class Primary:
            def __init__(self,*args):pass
            def read(self,start,end,path):return bytes(owner.mcu[start:end])
            def write(self,start,data,path):
                assert 0<=start<start+len(data)<=0x40000
                owner.mcu[start:start+len(data)]=data
        class Controller(Primary):
            def write(self,start,data,path):
                assert any(a<=start<start+len(data)<=b for a,b in u.RANGES)
                owner.mcu[start:start+len(data)]=data
        self.Primary,self.Controller=Primary,Controller
        self.port_patch=patch.object(u,'check_port',return_value=None);self.port_patch.start();self.addCleanup(self.port_patch.stop)
        self.loader_patch=patch.object(s,'loader',return_value=Path('TRUSTED_PROGRAMMER'));self.loader_patch.start();self.addCleanup(self.loader_patch.stop)

    def args(self,folder,kind):
        return SimpleNamespace(release=RELEASE,current_bundle=folder,output=self.root/'install',port=self.port,identity=self.identity,
            confirmation=f'INSTALL {kind.upper()} NANO20K {self.port}',request=None,busdev='1:2',serial='board-one',trusted_receipts=self.root/'verified-release-layouts.json')

    def cli(self,exe,args):
        self.commands.append(args)
        assert '--busdev-num' in args and args[args.index('--busdev-num')+1]=='1:2' and '--external-flash' in args
        assert '--bulk-erase' not in args and '--unprotect-flash' not in args
        if '--detect' in args:return 'Gowin GW2A(R)-18(C)\nJEDEC ID: 0xef4017'
        if '--dump-flash' in args:Path(args[-1]).write_bytes(self.fpga)
        else:
            assert '--verify' in args and '-f' in args
            data=Path(args[-1]).read_bytes();assert len(data)==s.BOOT_SPAN
            self.fpga[:len(data)]=data
        return 'OK'

    def stages(self,args):
        original_mcu=bytes(self.mcu);original_fpga=bytes(self.fpga)
        prepared=s.prepare(args,self.Primary)
        self.assertEqual(self.mcu[0x40000:],original_mcu[0x40000:])
        self.assertEqual((args.output/'before-full-flash.bin').read_bytes(),original_mcu)
        args.request=args.output/'system-install.json';args.confirmation='FPGA NANO20K 1:2'
        programmed=s.fpga(args,self.cli);self.assertEqual(programmed['phase'],'fpga_verified')
        self.assertEqual((args.output/'fpga-before-full-flash.bin').read_bytes(),original_fpga)
        self.assertEqual(self.fpga[s.BOOT_SPAN:],original_fpga[s.BOOT_SPAN:])
        args.confirmation=f'FINISH {prepared["target"].upper()} NANO20K TEST'
        finished=s.finish(args,self.Controller);self.assertEqual(finished['phase'],'complete')
        self.assertEqual(self.mcu[0x1df000:0x201000],original_mcu[0x1df000:0x201000])
        self.assertEqual(self.mcu[0x3ff000:],original_mcu[0x3ff000:])
        return finished

    def test_blank_to_arcade_and_back_to_console(self):
        a=self.args(self.bundle,'arcade');self.stages(a)
        self.assertEqual(self.mcu[0x100000:0x100008],b'NBCACHE1')
        marker=self.mcu[0x100040:0x100060]
        self.assertEqual(marker[:8],b'S16SAVE1')
        self.assertEqual(struct.unpack_from('<III',marker,16),(0x43505331,0x56494400,s.BOOT_BYTES))
        a=self.args(RELEASE,'console');a.output=self.root/'console-install';self.stages(a)
        self.assertEqual(self.mcu[0x201000:0x201008],b'NBCRXZ01')
        self.assertEqual(self.mcu[0x100000:0x1df000],b'\xff'*0xdf000)
        self.assertTrue(a.trusted_receipts.exists())
        a=self.args(self.bundle,'arcade');a.output=self.root/'arcade-again';self.stages(a)

    def test_blank_to_console(self):self.stages(self.args(RELEASE,'console'))

    def test_confirmation_prevents_first_mutation(self):
        a=self.args(self.bundle,'arcade');a.confirmation='UPDATE TEST';before=bytes(self.mcu)
        with self.assertRaises(ValueError):s.prepare(a,self.Primary)
        self.assertEqual(self.mcu,before);self.assertFalse(a.output.exists())

    def test_wrong_fpga_and_incomplete_backup_prevent_programming(self):
        a=self.args(self.bundle,'arcade');s.prepare(a,self.Primary);a.request=a.output/'system-install.json';a.confirmation='FPGA NANO20K 1:2'
        with self.assertRaises(ValueError):s.fpga(a,lambda exe,args:'GW5A-60\nJEDEC ID: 0xef4017')
        self.assertEqual(self.fpga,b'\x5a'*s.FPGA_SIZE)
        def short(exe,args):
            if '--detect' in args:return self.cli(exe,args)
            Path(args[-1]).write_bytes(b'short');return ''
        with self.assertRaises(ValueError):s.fpga(a,short)
        self.assertEqual(self.fpga,b'\x5a'*s.FPGA_SIZE)

    def test_final_step_requires_verified_fpga_and_same_board(self):
        a=self.args(self.bundle,'arcade');s.prepare(a,self.Primary);a.request=a.output/'system-install.json';a.confirmation='FINISH ARCADE NANO20K TEST'
        with self.assertRaises(ValueError):s.finish(a,self.Controller)
        a.confirmation='FPGA NANO20K 1:2';s.fpga(a,self.cli)
        a.confirmation='FINISH ARCADE NANO20K TEST';a.identity=json.dumps(['TEST',13467,24928,'other-board','slot'])
        with self.assertRaises(ValueError):s.finish(a,self.Controller)

    def test_corrupt_readback_does_not_advance_installation(self):
        a=self.args(self.bundle,'arcade');s.prepare(a,self.Primary);a.request=a.output/'system-install.json';a.confirmation='FPGA NANO20K 1:2'
        reads=0
        def corrupt(exe,args):
            nonlocal reads
            result=self.cli(exe,args)
            if '--dump-flash' in args:
                reads+=1
                if reads==2:
                    p=Path(args[-1]);data=bytearray(p.read_bytes());data[0]^=1;p.write_bytes(data)
            return result
        with self.assertRaises(ValueError):s.fpga(a,corrupt)
        self.assertEqual(json.loads(a.request.read_text())['phase'],'fpga_pending')
        s.fpga(a,self.cli)
        self.assertEqual(json.loads(a.request.read_text())['phase'],'fpga_verified')

if __name__=='__main__':unittest.main()
