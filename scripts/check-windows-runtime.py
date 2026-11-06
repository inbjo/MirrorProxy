"""Check PE32+ imports without relying on a runner's installed VC++ runtime.

Inspect both regular and delay imports. Windows system DLLs remain expected;
redistributable CRT and OpenSSL DLLs must be statically linked instead.
"""
import argparse
from pathlib import Path
import re
import struct
import sys


MACHINES = {'x64': 0x8664, 'arm64': 0xAA64}
EXTERNAL_RUNTIME = re.compile(
    r'(?i)^(?:vcruntime.*|msvcp.*|msvcr\d.*|concrt.*|vcomp.*|ucrtbase.*|'
    r'api-ms-win-crt-.*|libcrypto.*|libssl.*|libgcc.*|libstdc\+\+.*|libwinpthread.*)\.dll$'
)


def imported_dlls(data, architecture):
    def unpack(fmt, offset):
        if offset < 0 or offset + struct.calcsize(fmt) > len(data):
            raise ValueError('Truncated PE structure')
        return struct.unpack_from(fmt, data, offset)

    if data[:2] != b'MZ':
        raise ValueError('Missing DOS signature')
    pe, = unpack('<I', 0x3C)
    if data[pe:pe + 4] != b'PE\0\0':
        raise ValueError('Missing PE signature')
    machine, sections = unpack('<HH', pe + 4)
    if machine != MACHINES[architecture]:
        raise ValueError(f'Expected {architecture}, found PE machine 0x{machine:04x}')
    optional_size, = unpack('<H', pe + 20)
    optional = pe + 24
    magic, = unpack('<H', optional)
    if magic != 0x20B or optional_size < 112:
        raise ValueError('Expected a PE32+ optional header')
    directory_count, = unpack('<I', optional + 108)
    if directory_count > (optional_size - 112) // 8:
        raise ValueError('Invalid PE directory count')
    unpack(f'<{optional_size}s', optional)
    image_base, = unpack('<Q', optional + 24)
    headers_size, = unpack('<I', optional + 60)
    mappings = []
    for index in range(sections):
        section = optional + optional_size + index * 40
        unpack('<40s', section)
        _, virtual_address, raw_size, raw_offset = unpack('<IIII', section + 8)
        mappings.append((virtual_address, raw_size, raw_offset))

    def offset_of(rva, size=1):
        if 0 <= rva < headers_size and rva + size <= min(headers_size, len(data)):
            return rva
        for address, raw_size, raw_offset in mappings:
            if address <= rva and rva + size <= address + raw_size:
                offset = raw_offset + rva - address
                if offset + size <= len(data):
                    return offset
        raise ValueError(f'Unmapped PE RVA 0x{rva:x}')

    imports = set()
    for directory, descriptor_size, name_index in [(1, 20, 3), (13, 32, 1)]:
        if directory >= directory_count:
            continue
        rva, size = unpack('<II', optional + 112 + directory * 8)
        if not rva and not size:
            continue
        if not rva or size < descriptor_size:
            raise ValueError('Invalid import directory')
        terminated = False
        for relative in range(0, size - descriptor_size + 1, descriptor_size):
            descriptor = unpack(f'<{descriptor_size // 4}I', offset_of(rva + relative, descriptor_size))
            if not any(descriptor):
                terminated = True
                break
            name_rva = descriptor[name_index]
            if directory == 13 and not descriptor[0] & 1:
                name_rva -= image_base
            name = bytearray()
            for index in range(260):
                byte = data[offset_of(name_rva + index)]
                if byte == 0:
                    break
                name.append(byte)
            else:
                raise ValueError('Unterminated imported DLL name')
            if not name:
                raise ValueError('Empty imported DLL name')
            imports.add(name.decode('ascii'))
        if not terminated:
            raise ValueError('Unterminated import directory')
    if not imports:
        raise ValueError('No DLL imports found; refusing an unverified executable')
    return imports


def check_runtime(path, architecture):
    imports = imported_dlls(Path(path).read_bytes(), architecture)
    forbidden = sorted(name for name in imports if EXTERNAL_RUNTIME.fullmatch(name))
    if forbidden:
        raise ValueError('External runtime DLL dependencies: ' + ', '.join(forbidden))
    return sorted(imports)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--architecture', required=True, choices=MACHINES)
    parser.add_argument('executable', type=Path)
    args = parser.parse_args()
    try:
        imports = check_runtime(args.executable, args.architecture)
    except (OSError, ValueError, struct.error) as error:
        print(f'{args.executable}: {error}', file=sys.stderr)
        return 1
    print(f'{args.executable}: runtime dependency check passed; imports: {", ".join(imports)}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
