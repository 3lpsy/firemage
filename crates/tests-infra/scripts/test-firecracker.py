#!/usr/bin/env python3
"""Boot the pinned guest through the real Firecracker Unix API before retention."""
import http.client
import json
import os
from pathlib import Path
import shutil
import signal
import socket
import subprocess
import tempfile
import time


class UnixConnection(http.client.HTTPConnection):
    def __init__(self, path):
        super().__init__('localhost', timeout=10)
        self.path = str(path)

    def connect(self):
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.sock.settimeout(self.timeout)
        self.sock.connect(self.path)


def interrupted(number, _frame):
    raise SystemExit(128 + number)


def main():
    signal.signal(signal.SIGTERM, interrupted)
    fixtures = Path(os.environ.get('FIREMAGE_TEST_FIXTURES', '/opt/firemage/fixtures'))
    with tempfile.TemporaryDirectory(prefix='firemage-boot-') as temporary:
        work = Path(temporary)
        shutil.copyfile(fixtures / 'rootfs.ext4', work / 'rootfs.ext4')
        seed = work / 'seed'
        seed.mkdir()
        (seed / 'run.sh').write_text('echo FIREMAGE_BOOT_OK\necho boot-ok > /firemage/output/result\n')
        subprocess.run(['truncate', '-s', '16M', str(work / 'seed.ext4')], check=True)
        subprocess.run(['mkfs.ext4', '-q', '-F', '-d', str(seed), str(work / 'seed.ext4')], check=True)
        api = work / 'firecracker.sock'
        with (work / 'serial.log').open('wb') as log:
            process = subprocess.Popen(['firecracker', '--api-sock', str(api)], stdout=log, stderr=log)
            try:
                for _ in range(100):
                    if api.exists():
                        break
                    if process.poll() is not None:
                        raise RuntimeError('Firecracker exited before creating its API socket')
                    time.sleep(0.05)

                def put(path, value):
                    connection = UnixConnection(api)
                    try:
                        connection.request('PUT', path, json.dumps(value), {'Content-Type': 'application/json'})
                        response = connection.getresponse()
                        body = response.read()
                        assert response.status == 204, (path, response.status, body)
                    finally:
                        connection.close()

                put('/machine-config', {'vcpu_count': 1, 'mem_size_mib': 256})
                put('/boot-source', {'kernel_image_path': str(fixtures / 'kernel'),
                                     'boot_args': 'console=ttyS0 reboot=k panic=1 pci=off root=/dev/vda rw init=/init'})
                for name in ('rootfs', 'seed'):
                    put('/drives/' + name, {'drive_id': name, 'path_on_host': str(work / (name + '.ext4')),
                                           'is_root_device': name == 'rootfs', 'is_read_only': name == 'seed'})
                put('/actions', {'action_type': 'InstanceStart'})
                assert process.wait(timeout=45) == 0
                assert 'FIREMAGE_BOOT_OK' in (work / 'serial.log').read_text()
                output = subprocess.check_output(['debugfs', '-R', 'cat /firemage/output/result',
                                                  str(work / 'rootfs.ext4')], stderr=subprocess.DEVNULL)
                assert output == b'boot-ok\n', output
                print('Real Firecracker guest boot, seed disk, output disk, and shutdown passed')
            finally:
                if process.poll() is None:
                    process.kill()
                    process.wait(timeout=10)
                print((work / 'serial.log').read_text(errors='replace'))
                results = os.environ.get('FIREMAGE_CI_RESULTS_DIR')
                if results:
                    destination = Path(results) / 'firecracker-serial.log'
                    shutil.copyfile(work / 'serial.log', destination)
                    destination.chmod(0o644)


if __name__ == '__main__':
    main()
