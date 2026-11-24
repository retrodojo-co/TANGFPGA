"""Arcade V1 application-only updates. No release-supplied Python is executed."""
import json
import os
import re
import struct
import uuid
import zlib
from pathlib import Path
import current_bundle as console
import updater as u

START, END = 0x40000, 0xe8000
FORMAT = 'TANGFPGA_ARCADE_V1'
# Audited applications accepted by the Arcade r16 updater, plus r16 itself.
KNOWN_APPLICATIONS = {
 'da3cbdd5fe598b61739f68a4c899a2bb2378de393d15d4fcf45065166e5f3685',
 '43c191c9878ce12947ab01c21a81f7b367004e0581bb0c1d1bad3440e068ad27',
 'c127327a13c12052b492847c3a2911bf53db5bdf5f5ed19ea4b4b43c76e23a3b',
 'fd7d2abb88658cb62b9bbc8ce6736b98d4ad52e8e4efc69f1de8c8b58834313c',
 '94c6b33a60a4543b0f93cd97416dd4e97a7a2de979a000298684d72b818fbc42',
 'bf2a87b74b810145c3db76f3016167af62dd0c70b27e0b06523339b357e9c3bf',
}
VIDEO_PATHS = {'SD-card/cps1/video/cps1/480p60.bin'} | {
 f'SD-card/system16/video/{system}/{mode}.bin'
 for system in ('system16a', 'system16b')
 for mode in ('480p60', '720p60', 'direct15', 'direct31')
}

def describe(folder, manifest=None):
    m = console.read_json(manifest or folder/'release.json')
    u.require(isinstance(m, dict) and m.get('format') == FORMAT, 'Unsupported Arcade release format')
    u.require(m.get('systems') == ['system16a', 'system16b', 'cps1'] and m.get('boot_marker_protocol') == 28,
              'Unsupported Arcade systems or recovery protocol')
    version = m.get('version')
    u.require(isinstance(version, str) and 0 < len(version) <= 100, 'Missing Arcade release version')
    fw, videos = m.get('firmware'), m.get('video_images')
    u.require(isinstance(fw, dict) and type(fw.get('offset')) is int and fw['offset'] == START,
              'Arcade application offset must be 0x40000')
    u.require(isinstance(videos, list) and 9 <= len(videos) <= 32, 'Invalid Arcade video image list')
    names = set()
    for record in [fw] + videos:
        u.require(isinstance(record, dict), 'Invalid Arcade file record')
        name = record.get('file')
        console.inside(folder, name)
        u.require(name.casefold() not in names, 'Duplicate Arcade file record')
        names.add(name.casefold())
        u.require(type(record.get('bytes')) is int and
                  (4096 < record['bytes'] <= END-START if record is fw else record['bytes'] == 907418),
                  'Invalid Arcade file size')
        u.require(isinstance(record.get('sha256'), str) and re.fullmatch('[0-9a-f]{64}', record['sha256']),
                  'Invalid Arcade checksum')
    u.require(VIDEO_PATHS <= {r['file'] for r in videos}, 'Missing required Arcade video images')
    return m

def receipt_path(path):
    return Path(path).with_name('verified-arcade-layouts.json') if path else None

def receipts(path):
    path = receipt_path(path)
    if not path or not path.exists(): return []
    values = console.read_json(path)
    u.require(isinstance(values, list) and len(values) <= 256 and all(
        isinstance(v, str) and re.fullmatch('[0-9a-f]{64}', v) for v in values), 'Invalid Arcade update history')
    return values

def remember(path, changes):
    if not path: return
    u.require(len(changes) == 1 and changes[0][0] == START and len(changes[0][1]) == END-START,
              'Invalid Arcade receipt')
    data = changes[0][1]
    length = 4096 + struct.unpack_from('<I', data, 132)[0]
    u.firmware(data[:length])
    digest = u.sha(data[:length])
    values = [v for v in receipts(path) if v != digest][-255:] + [digest]
    path = receipt_path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(path.name+'.'+uuid.uuid4().hex+'.tmp')
    u.durable(temporary, (json.dumps(values)+'\n').encode())
    os.replace(temporary, path)

def load(folder, release=None, trusted_receipts=None, manifest=None):
    folder = Path(folder).resolve()
    m = describe(folder, manifest)
    def checked(record):
        path = console.inside(folder, record['file'])
        u.require(path.is_file() and path.stat().st_size == record['bytes'], 'Arcade file is missing or has the wrong size')
        data = path.read_bytes()
        u.require(u.sha(data) == record['sha256'], 'Arcade file checksum mismatch: '+record['file'])
        return data
    fw = checked(m['firmware'])
    u.firmware(fw)
    for record in m['video_images']: checked(record)
    changes = [(START, fw + b'\xff'*(END-START-len(fw)))]
    trusted = KNOWN_APPLICATIONS | {m['firmware']['sha256']} | set(receipts(trusted_receipts))
    def plan(before, requested):
        u.require(requested == changes and len(before) == u.SIZE, 'Invalid Arcade update request')
        length = 4096 + struct.unpack_from('<I', before, START+132)[0]
        u.require(4096 < length <= END-START, 'Unrecognized installed Arcade application; backup retained, no writes')
        old = before[START:START+length]
        u.firmware(old)
        u.require(u.sha(old) in trusted, 'Unrecognized Arcade application; backup retained, no writes')
        u.require(before[0x100000:0x100008] == b'NBCACHE1', 'Missing compatible Arcade recovery cache')
        size, crc = struct.unpack_from('<II', before, 0x100008)
        u.require(size == 907418, 'Invalid Arcade recovery size')
        core = before[0x101000:0x101000+size]
        u.require(zlib.crc32(core) == crc and u.sha(core) in {r['sha256'] for r in m['video_images']},
                  'Unrecognized or damaged Arcade recovery image')
        marker = before[0x100040:0x100060]
        u.require(marker[:8] == b'S16SAVE1' and struct.unpack_from('<II', marker, 8) == (28, crc)
                  and zlib.crc32(marker[:28]) == struct.unpack_from('<I', marker, 28)[0],
                  'Arcade recovery marker must use protocol 28')
        after = bytearray(before)
        after[START:END] = changes[0][1]
        u.require(after[:START] == before[:START] and after[END:] == before[END:], 'Protected Arcade bytes changed')
        return bytes(after)
    return changes, plan
