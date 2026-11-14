# -*- coding: utf-8 -*-
"""LPasistant APK 安全自检:解包 Manifest(二进制 AXML)并核对关键安全标志。

用法: python scripts/verify_apk.py <apk路径>
核对项:debuggable / allowBackup / 明文配置 / 导出组件 / 权限清单。
"""
import struct
import sys
import tempfile
import zipfile
from pathlib import Path

if sys.platform == 'win32':
    try:
        sys.stdout.reconfigure(encoding='utf-8')
    except Exception:
        pass

RES_STRING_POOL = 0x0001
RES_XML_START_ELEMENT = 0x0102
RES_XML_END_ELEMENT = 0x0103


def parse_string_pool(data, off):
    typ, hdr_size, size = struct.unpack_from('<HHI', data, off)
    str_count, style_count, flags = struct.unpack_from('<III', data, off + 8)
    strings_start = struct.unpack_from('<I', data, off + 20)[0]
    utf8 = bool(flags & 0x100)
    ids_off = off + 28
    base = off + strings_start
    strings = []
    for i in range(str_count):
        s_off = struct.unpack_from('<I', data, ids_off + 4 * i)[0]
        p = base + s_off
        if utf8:
            n1 = data[p]; p += 1
            if n1 & 0x80: n1 = ((n1 & 0x7F) << 8) | data[p]; p += 1
            n2 = data[p]; p += 1
            if n2 & 0x80: n2 = ((n2 & 0x7F) << 8) | data[p]; p += 1
            strings.append(data[p:p + n1].decode('utf-8', 'replace'))
        else:
            n = struct.unpack_from('<H', data, p)[0]; p += 2
            if n & 0x8000: n = ((n & 0x7FFF) << 16) | struct.unpack_from('<H', data, p)[0]; p += 2
            strings.append(data[p:p + n * 2].decode('utf-16-le', 'replace'))
    return strings


def parse_axml(data):
    off = 8
    strings = []
    elements = []  # (tag, [(name, value)])
    while off + 8 <= len(data):
        typ, hdr_size, size = struct.unpack_from('<HHI', data, off)
        if typ == RES_STRING_POOL:
            strings = parse_string_pool(data, off)
        elif typ == RES_XML_START_ELEMENT and strings:
            ext = off + 16
            _ns, name = struct.unpack_from('<iI', data, ext)
            attr_start, attr_size, attr_count = struct.unpack_from('<HHH', data, ext + 8)
            attrs = []
            for i in range(attr_count):
                a = ext + attr_start + i * attr_size
                _ans, aname, _raw, _asize, _res0, atype, adata = struct.unpack_from('<iIiHBBI', data, a)
                if atype == 0x12:
                    val = 'true' if (adata & 0xFFFFFFFF) else 'false'
                elif atype == 0x10:
                    val = str(adata if adata < 0x80000000 else adata - 0x100000000)
                elif atype == 0x03:
                    val = strings[adata] if adata < len(strings) else '?'
                else:
                    val = 'res:%d' % adata
                attrs.append((strings[aname] if aname < len(strings) else '?', val))
            elements.append((strings[name] if name < len(strings) else '?', attrs))
        off += size if size else 8
    return elements


def main():
    if len(sys.argv) != 2:
        sys.exit('用法: python verify_apk.py <apk路径>')
    apk = Path(sys.argv[1])
    with tempfile.TemporaryDirectory() as td:
        with zipfile.ZipFile(apk) as z:
            z.extract('AndroidManifest.xml', td)
        elements = parse_axml((Path(td) / 'AndroidManifest.xml').read_bytes())

    d = dict()
    for tag, attrs in elements:
        if tag == 'application':
            d.update(attrs)
        if tag == 'uses-permission':
            d.setdefault('permissions', []).append(dict(attrs)['name'])

    print('包名/版本:', [v for t, a in elements if t == 'manifest' for k, v in a if k == 'package'])
    print('权限清单:')
    for p in d.get('permissions', []):
        print('  -', p)
    for k in ('debuggable', 'allowBackup', 'usesCleartextTraffic', 'networkSecurityConfig'):
        print(f'{k}: {d.get(k, "(未设置)")}')

    print('\n导出组件(exported=true):')
    for tag, attrs in elements:
        if tag in ('activity', 'receiver', 'service', 'provider') and dict(attrs).get('exported') == 'true':
            print(f'  - {dict(attrs).get("name")}  [{tag}]')

    print('\n判定:')
    checks = [
        ('debuggable', d.get('debuggable') in (None, 'false', '(未设置)'), 'release 包必须不可调试'),
        ('allowBackup', d.get('allowBackup') in (None, 'false', '(未设置)'), '关闭 adb 备份提取'),
        ('usesCleartextTraffic', d.get('usesCleartextTraffic') in (None, 'false', '(未设置)'), '禁止全局明文'),
    ]
    ok = True
    for name, passed, why in checks:
        print(f'  {"✅" if passed else "❌"} {name}: {"通过" if passed else why}')
        ok &= passed
    exported = [dict(a).get('name') for t, a in elements
                if t in ('activity', 'receiver', 'service', 'provider')
                and dict(a).get('exported') == 'true']
    expect = {
        'com.lenovo.lpasistant.MainActivity',
        'com.lenovo.lpasistant.sys.BootReceiver',
        # androidx.profileinstaller 标准组件:受 android.permission.DUMP(签名级)保护,普通 App 无法触发
        'androidx.profileinstaller.ProfileInstallReceiver',
    }
    extra = [e for e in exported if e not in expect]
    print(f'  {"✅" if not extra else "❌"} 导出组件白名单外无多余: {extra or "无"}')
    ok &= not extra
    sys.exit(0 if ok else 1)


if __name__ == '__main__':
    main()
