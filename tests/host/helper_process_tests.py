#!/usr/bin/env python3
"""Actual target helper process. Never print terminal captures or capabilities."""
import errno
import fcntl
import os
import select
import signal
import socket
import struct
import subprocess
import sys
import termios
import time
import re
import threading
from http.server import BaseHTTPRequestHandler, HTTPServer


def check(ok, message):
    if not ok:
        raise AssertionError(message)


def redirected(binary):
    master, slave = os.openpty()
    try:
        for stdin in (subprocess.DEVNULL, slave):
            result = subprocess.run([binary, '--bind', '127.0.0.1:0', '--target', 'localhost:3002'],
                                    stdin=stdin, stdout=subprocess.PIPE,
                                    stderr=subprocess.PIPE, timeout=10)
            check(result.returncode != 0, 'redirected helper unexpectedly succeeded')
            check(result.stdout == b'', 'redirected helper wrote output (omitted)')
            check(b'helper-terminal' in result.stderr, 'missing terminal rejection (output omitted)')
        print('redirected-output=refused')
    finally:
        os.close(slave)
        os.close(master)


def rejected_arguments(binary):
    master, slave = os.openpty()
    try:
        for args in [
            ['--qr-test'], ['--masque-app', '127.0.0.1', '127.0.0.2', '3002'],
            ['--target-port', '3002'],
            ['--target', '192.0.2.1:3002'],
            ['--target', 'http://localhost:3002'],
            ['--target', 'localhost:3002/path'],
            ['--bind', '0.0.0.0:0', '--target', 'localhost:3002'],
            ['--bind', '127.0.0.1:0', '--target', 'localhost:0'],
            ['--bind', '127.0.0.1:0', '--target', 'localhost:3002', '--relay', '127.0.0.1:0'],
            ['--bind', '127.0.0.1:0', '--target', 'localhost:3002', '--observer', 'example.invalid:10000'],
            ['--bind', '127.0.0.1:0', '--bind', '127.0.0.1:0', '--target', 'localhost:3002'],
        ]:
            result = subprocess.run([binary, *args], stdin=slave, stdout=slave,
                                    stderr=subprocess.PIPE, timeout=10)
            check(result.returncode != 0 and b'Helper failed: helper-' in result.stderr,
                  'invalid or legacy CLI arguments were not rejected (output omitted)')
        print('legacy-and-invalid-arguments=refused')
    finally:
        os.close(slave)
        os.close(master)


def controlling_terminal():
    os.setsid()
    fcntl.ioctl(0, termios.TIOCSCTTY, 0)


def qr_pixels(capture):
    try:
        frame = capture.rsplit(b'\x1b[2J', 1)[-1].decode('utf-8')
    except UnicodeDecodeError:
        return None
    rows = []
    for row, text in re.findall(r'\x1b\[(\d+);1H([^\x1b]*)', frame):
        if int(row) >= 4 and text and set(text) <= set(' ▀▄█'):
            rows.append(text)
    if not rows:
        return None
    width = len(rows[0])
    if len(rows) != (width + 1) // 2 or any(len(row) != width for row in rows):
        return None
    pixels = bytearray()
    for row in rows:
        pixels.extend(int(ch in '▀█') for ch in row)
        pixels.extend(int(ch in '▄█') for ch in row)
    return struct.pack('!II', width, len(rows) * 2) + pixels


class Target(BaseHTTPRequestHandler):
    def do_GET(self):
        check(self.path == '/helper', 'unexpected target path')
        self.send_response(200)
        self.send_header('Content-Length', '11')
        self.end_headers()
        self.wfile.write(b'from-target')

    def log_message(self, *args):
        pass


def wait_for_terminal(master, child, capture, predicate):
    def read_ready():
        try:
            chunk = os.read(master, 65536)
        except OSError as error:
            if error.errno == errno.EIO:  # Linux PTY EOF after the slave closes.
                return False
            raise
        capture.extend(chunk)
        check(len(capture) < 2_000_000, 'unbounded terminal output (omitted)')
        return bool(chunk)

    deadline = time.monotonic() + 25
    while not predicate() and time.monotonic() < deadline:
        if select.select([master], [], [], 0.05)[0] and not read_ready():
            # The slave can close just before the process becomes waitable.
            time.sleep(0.01)
        if child.poll() is not None:
            break
    if child.poll() is not None:
        # Exit does not mean the PTY has been drained. In particular, phone.wait()
        # can return after the helper exits but before we read its restoration.
        while select.select([master], [], [], 0)[0]:
            if not read_ready():
                break
    check(predicate(), 'terminal expectation failed (output omitted)')


class IPv6HTTPServer(HTTPServer):
    address_family = socket.AF_INET6


def terminal_quit(binary, stop, actor=None, preparing=False, auto_bind=False, ipv6_target=False):
    master, slave = os.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 24, 80, 0, 0))
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as reserve:
        reserve.bind(('127.0.0.1', 0))
        address = reserve.getsockname()
    capture = bytearray()
    server = IPv6HTTPServer if ipv6_target else HTTPServer
    target = server(('::1' if ipv6_target else '127.0.0.1', 0), Target) if actor else None
    target_port = target.server_port if target else 3002
    phone = None
    quiet = socket.socket(socket.AF_INET, socket.SOCK_DGRAM) if preparing else None
    args = [binary, '--bind', f'127.0.0.1:{address[1]}', '--target', f'localhost:{target_port}']
    if auto_bind:
        check(preparing, 'automatic-bind fixture must use a loopback participant')
        args = [binary, '--target', f'localhost:{target_port}']
    if quiet:
        quiet.bind(('127.0.0.1', 0))
        args += ['--observer', f'127.0.0.1:{quiet.getsockname()[1]}']
    child = subprocess.Popen(args,
                             stdin=slave, stdout=slave, stderr=slave,
                             env={**os.environ, 'TERM': 'xterm-256color'},
                             preexec_fn=controlling_terminal)
    os.close(slave)
    if target:
        threading.Thread(target=target.serve_forever, daemon=True).start()

    def until(predicate):
        wait_for_terminal(master, child, capture, predicate)

    try:
        if quiet:
            until(lambda: b'Preparing' in capture)
            check(select.select([quiet], [], [], 10)[0], 'no real bootstrap packet observed')
            _, source = quiet.recvfrom(65536)
            if auto_bind:
                check(source[0] == '127.0.0.1', 'automatic bind selected wrong interface')
                address = source  # Real transport port, not the discarded lookup socket.
        else:
            until(lambda: b'QR hidden: terminal too small' in capture)
            fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack('HHHH', 60, 140, 0, 0))
            until(lambda: qr_pixels(capture) is not None)
        check(not (termios.tcgetattr(master)[3] & termios.ICANON), 'terminal is not raw')
        check(b'wpa2:' not in capture, 'plaintext offer was printed')
        if actor:
            phone_binary, java, classpath = actor
            decoded = subprocess.run([java, '-cp', classpath, 'HelperQrDecode'],
                                     input=qr_pixels(capture), stdout=subprocess.PIPE,
                                     stderr=subprocess.PIPE, timeout=10)
            check(decoded.returncode == 0, 'independent QR decoding failed (contents omitted)')
            encoded = bytearray(decoded.stdout)
            del decoded
            check(encoded.startswith(b'wpa2:') and len(encoded) <= 195, 'wrong offer format')
            phone = subprocess.Popen([phone_binary], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            phone.stdin.write(struct.pack('!H', len(encoded)) + encoded)
            phone.stdin.flush()
            encoded[:] = b'\0' * len(encoded)
            check(select.select([phone.stdout], [], [], 15)[0], 'browser response timed out')
            check(phone.stdout.readline() == b'HTTP_OK\n', 'browser HTTP failed (contents omitted)')
            until(lambda: b'Connected' in capture)
            # QR must be removed rather than left reusable-looking during a session.
            check(qr_pixels(capture) is None, 'QR stayed visible after authorization')
            phone.stdin.write(b'x')
            phone.stdin.flush()
            phone.wait(timeout=20)
            check(phone.returncode == 0, 'phone Stop failed (output omitted)')
        elif stop in ('term', 'int'):
            child.send_signal(signal.SIGTERM if stop == 'term' else signal.SIGINT)
        else:
            os.write(master, stop)
        until(lambda: child.poll() is not None)
        check(child.returncode == 0, 'helper did not exit cleanly (output omitted)')
        check(b'\x1b[?1049h' in capture and b'\x1b[?1049l' in capture, 'alternate screen not restored')
        with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as released:
            released.bind(address)
        print('qr-transfer=http-ok session-end=clean' if actor else
              'auto-bind=ok preparation-stop=clean socket=released' if auto_bind else
              'preparation-stop=clean socket=released' if preparing else 'terminal-quit=clean socket=released')
    finally:
        os.close(master)
        if quiet:
            quiet.close()
        if phone:
            phone.stdin.close()
            try:
                phone.wait(timeout=20)
            except subprocess.TimeoutExpired:
                phone.kill()
                phone.wait(timeout=5)
        if target:
            target.shutdown()
            target.server_close()
        if child.poll() is None:
            child.send_signal(signal.SIGTERM)
            child.send_signal(signal.SIGCONT)
            try:
                child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                child.kill()  # This test's own child only; cleanup has already failed.
                child.wait(timeout=5)
        capture[:] = b'\0' * len(capture)


if __name__ == '__main__':
    redirected(sys.argv[1])
    rejected_arguments(sys.argv[1])
    for stop in (b'q', b'\x1b', b'\x03', 'term', 'int'):
        terminal_quit(sys.argv[1], stop)
    terminal_quit(sys.argv[1], b'q', preparing=True)
    terminal_quit(sys.argv[1], b'q', preparing=True, auto_bind=True)
    if len(sys.argv) == 5:
        terminal_quit(sys.argv[1], None, tuple(sys.argv[2:]))
        terminal_quit(sys.argv[1], None, tuple(sys.argv[2:]), ipv6_target=True)
