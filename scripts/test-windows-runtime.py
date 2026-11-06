"""Exercise the runtime guard with real PE structures, including delay imports."""
import importlib.util
from pathlib import Path
import struct
import sys
import tempfile
import unittest


sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location(
    'windows_runtime', Path(__file__).with_name('check-windows-runtime.py'))
runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime)


def executable(machine=0x8664, regular=('KERNEL32.dll',), delayed=()):
    data = bytearray(2048)
    data[:2] = b'MZ'
    struct.pack_into('<I', data, 0x3C, 0x80)
    data[0x80:0x84] = b'PE\0\0'
    struct.pack_into('<HH', data, 0x84, machine, 1)
    struct.pack_into('<H', data, 0x94, 240)
    optional = 0x98
    struct.pack_into('<H', data, optional, 0x20B)
    struct.pack_into('<Q', data, optional + 24, 0x140000000)
    struct.pack_into('<I', data, optional + 60, 512)
    struct.pack_into('<I', data, optional + 108, 16)
    section = optional + 240
    struct.pack_into('<IIII', data, section + 8, 1536, 0x1000, 1536, 512)
    name_offset = 0x500
    for directory, table_offset, size, names in [(1, 0x200, 20, regular), (13, 0x380, 32, delayed)]:
        if not names:
            continue
        struct.pack_into('<II', data, optional + 112 + directory * 8,
                         table_offset - 512 + 0x1000, size * (len(names) + 1))
        for index, name in enumerate(names):
            descriptor = table_offset + index * size
            name_rva = name_offset - 512 + 0x1000
            if directory == 1:
                struct.pack_into('<I', data, descriptor + 12, name_rva)
            else:
                struct.pack_into('<II', data, descriptor, 1, name_rva)
            encoded = name.encode('ascii') + b'\0'
            data[name_offset:name_offset + len(encoded)] = encoded
            name_offset += len(encoded)
    return data


class RuntimeTests(unittest.TestCase):
    def check(self, data, architecture='x64'):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'client.exe'
            path.write_bytes(data)
            return runtime.check_runtime(path, architecture)

    def test_system_imports_pass_for_both_architectures(self):
        for architecture, machine in runtime.MACHINES.items():
            with self.subTest(architecture=architecture):
                self.assertEqual(self.check(executable(machine, delayed=('WS2_32.dll',)), architecture),
                                 ['KERNEL32.dll', 'WS2_32.dll'])

    def test_runtime_imports_fail_in_regular_and_delay_tables(self):
        for name in ['VCRUNTIME140.dll', 'vcruntime140_1.dll', 'MSVCP140.dll',
                     'msvcp140_atomic_wait.dll', 'CONCRT140.dll', 'msvcr120.dll',
                     'ucrtbase.dll', 'api-ms-win-crt-heap-l1-1-0.dll',
                     'libcrypto-3-x64.dll', 'libssl-3-arm64.dll']:
            for delayed in [False, True]:
                with self.subTest(name=name, delayed=delayed):
                    data = executable(delayed=(name,)) if delayed else executable(regular=(name,))
                    with self.assertRaisesRegex(ValueError, 'External runtime DLL dependencies'):
                        self.check(data)

    def test_wrong_architecture_fails(self):
        with self.assertRaisesRegex(ValueError, 'Expected arm64'):
            self.check(executable(), 'arm64')

    def test_malformed_or_missing_imports_fail_closed(self):
        missing_name = executable()
        struct.pack_into('<I', missing_name, 0x20C, 0x9000)
        unterminated = executable()
        struct.pack_into('<I', unterminated, 0x98 + 112 + 8 + 4, 20)
        for data in [b'not a PE', executable()[:1024], executable(regular=()),
                     missing_name, unterminated]:
            with self.subTest(length=len(data)):
                with self.assertRaises(ValueError):
                    self.check(data)


if __name__ == '__main__':
    unittest.main()
