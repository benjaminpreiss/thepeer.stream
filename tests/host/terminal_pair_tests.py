#!/usr/bin/env python3
"""Real two-CLI localhost browser flow. Never print captured terminals or offers."""
import fcntl
import http.client
import os
import re
import signal
import socket
import struct
import subprocess
import sys
import termios
import threading
from http.server import BaseHTTPRequestHandler, HTTPServer

from helper_process_tests import controlling_terminal, wait_for_terminal


class App(BaseHTTPRequestHandler):
    def do_GET(self):
        assert self.path == '/restart'
        self.send_response(200)
        self.send_header('Content-Length', '2')
        self.end_headers()
        self.wfile.write(b'ok')

    def log_message(self, *args):
        pass


def start_app(port):
    server = HTTPServer(('127.0.0.1', port), App)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    return server, thread


def terminal(binary, *args):
    master, slave = os.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 40, 220, 0, 0))
    child = subprocess.Popen([binary, *args], stdin=slave, stdout=slave, stderr=slave,
                             env={**os.environ, 'TERM': 'xterm-256color'},
                             preexec_fn=controlling_terminal)
    os.close(slave)
    return child, master, bytearray()


def stop(child, master):
    if child.poll() is None:
        child.send_signal(signal.SIGTERM)
        try:
            child.wait(timeout=15)
        except subprocess.TimeoutExpired:
            child.kill()
            child.wait(timeout=5)
    os.close(master)


def request(port, hostname):
    client = http.client.HTTPConnection('127.0.0.1', port, timeout=10)
    try:
        client.request('GET', '/restart', headers={'Host': f'{hostname}:{port}'})
        response = client.getresponse()
        return response.status, response.read()
    finally:
        client.close()


def main(binary):
    with socket.socket() as reserve:
        reserve.bind(('127.0.0.1', 0))
        port = reserve.getsockname()[1]
    server, thread = start_app(port)
    host = client = None
    try:
        host = terminal(binary, 'share', f'localhost:{port}', '--bind', '127.0.0.1:0')
        host_child, host_pty, host_capture = host
        wait_for_terminal(host_pty, host_child, host_capture,
                          lambda: re.search(rb'wpa2:[A-Za-z0-9_-]{98,190}', host_capture) is not None)
        offer = bytearray(re.search(rb'wpa2:[A-Za-z0-9_-]{98,190}', host_capture).group())
        client = terminal(binary, 'connect')
        client_child, client_pty, client_capture = client
        wait_for_terminal(client_pty, client_child, client_capture,
                          lambda: b'Paste connection string' in client_capture)
        os.write(client_pty, offer + b'\r')
        wait_for_terminal(client_pty, client_child, client_capture,
                          lambda: re.search(rb'http://(thepeer-[0-9a-f]{32}\.localhost):(\d+)', client_capture) is not None)
        match = re.search(rb'http://(thepeer-[0-9a-f]{32}\.localhost):(\d+)', client_capture)
        hostname, local_port = match.group(1).decode('ascii'), int(match.group(2))
        assert offer not in client_capture, 'prompt echoed the capability'
        offer[:] = b'\0' * len(offer)
        assert request(local_port, hostname) == (200, b'ok')
        server.shutdown()
        thread.join(timeout=5)
        server.server_close()
        server = None
        assert request(local_port, hostname)[0] in (502, 503), 'target outage not reported'
        assert client_child.poll() is None and host_child.poll() is None, 'tunnel ended with target'
        server, thread = start_app(port)
        assert request(local_port, hostname) == (200, b'ok'), 'restart required re-pairing'
        assert client_child.poll() is None and host_child.poll() is None, 'tunnel ended after restart'
        client_child.send_signal(signal.SIGINT)
        wait_for_terminal(client_pty, client_child, client_capture, lambda: client_child.poll() is not None)
        assert client_child.returncode == 0, 'client Stop failed'
        wait_for_terminal(host_pty, host_child, host_capture, lambda: host_child.poll() is not None)
        assert host_child.returncode == 0, 'host did not clean up after peer Stop'
        print('terminal-pair=authenticated target-restart=persistent stop=clean')
    finally:
        if server:
            server.shutdown()
            thread.join(timeout=5)
            server.server_close()
        for session in (client, host):
            if session:
                child, master, capture = session
                stop(child, master)
                capture[:] = b'\0' * len(capture)


if __name__ == '__main__':
    main(sys.argv[1])
