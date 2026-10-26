"""Loopback HTTP/untrusted-TLS fixture for native mobile Loader acceptance.

Uses an explicitly supplied self-signed certificate. Never installs trust roots
or disables certificate verification. Writes raw request/disconnect evidence.
"""
import argparse
import json
from pathlib import Path
import signal
import socket
import ssl
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlsplit

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--out', type=Path, required=True)
parser.add_argument('--cert', type=Path, required=True)
parser.add_argument('--key', type=Path, required=True)
parser.add_argument('--http-port', type=int, default=8791)
parser.add_argument('--tls-port', type=int, default=8792)
args = parser.parse_args()
root = args.out.resolve()
if not root.is_relative_to('/Volumes/Data/Caches/glory'):
    parser.error('use a data-volume output directory')
root.mkdir(parents=True, exist_ok=True)
assets = Path(__file__).parent / 'assets'
lock = threading.Lock()
states = {}
ended = threading.Event()


def record(event, **fields):
    with lock:
        with (root / 'server.jsonl').open('a') as stream:
            stream.write(json.dumps(dict(event=event, time=time.time(), **fields)) + '\n')


class Handler(BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'

    def log_message(self, *_):
        pass

    def reply(self, body, status=200, **headers):
        self.send_response(status)
        self.send_header('Content-Length', str(len(body)))
        self.send_header('Connection', 'close')
        for name, value in headers.items():
            self.send_header(name.replace('_', '-'), value)
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        parsed = urlsplit(self.path)
        token = parse_qs(parsed.query).get('token', [''])[0][:80]
        record('request', path=parsed.path, token=token,
               encoding=self.headers.get('Accept-Encoding'))
        try:
            if parsed.path in ('/theme.css', '/badge.png'):
                self.reply((assets / parsed.path[1:]).read_bytes())
            elif parsed.path == '/ok':
                self.reply(b'glory network')
            elif parsed.path == '/redirect':
                self.reply(b'', 302, Location='/ok')
            elif parsed.path == '/redirect-outside':
                self.reply(b'', 302, Location='http://outside.invalid/denied')
            elif parsed.path == '/redirect-loop':
                self.reply(b'', 302, Location='/redirect-loop')
            elif parsed.path == '/large':
                self.reply(b'x' * (128 * 1024))
            elif parsed.path == '/chunked':
                self.send_response(200)
                self.send_header('Transfer-Encoding', 'chunked')
                self.send_header('Connection', 'close')
                self.end_headers()
                for _ in range(2):
                    self.wfile.write(b'10000\r\n' + b'x' * 65536 + b'\r\n')
                self.wfile.write(b'0\r\n\r\n')
            elif parsed.path == '/encoded':
                self.reply(b'not decoded', Content_Encoding='gzip')
            elif parsed.path == '/status':
                with lock:
                    state = states.get(token, 'absent')
                self.reply(state.encode())
            elif parsed.path == '/slow':
                self.send_response(200)
                self.send_header('Content-Length', '2')
                self.send_header('Connection', 'close')
                self.end_headers()
                self.wfile.write(b'a')
                self.wfile.flush()
                with lock:
                    if len(states) >= 256:
                        raise RuntimeError('fixture token budget')
                    states[token] = 'entered'
                record('entered', token=token)
                self.connection.settimeout(.1)
                end = time.monotonic() + 20
                while not ended.is_set() and time.monotonic() < end:
                    try:
                        data = self.connection.recv(1)
                        if not data:
                            break
                    except socket.timeout:
                        continue
                    except ConnectionError:
                        break
                else:
                    record('hold_timeout', token=token)
                    return
                with lock:
                    states[token] = 'disconnected'
                record('disconnected', token=token)
            else:
                self.reply(b'missing', 404)
        except (BrokenPipeError, ConnectionError):
            record('body_disconnected', path=parsed.path, token=token)


class Server(ThreadingHTTPServer):
    daemon_threads = True

    def __init__(self, port, tls=None):
        self.tls = tls
        self.slots = threading.Semaphore(32)
        super().__init__(('127.0.0.1', port), Handler)

    def get_request(self):
        connection, address = super().get_request()
        connection.settimeout(5)
        if self.tls:
            try:
                connection = self.tls.wrap_socket(connection, server_side=True)
            except ssl.SSLError as error:
                record('tls_rejected', error=str(error))
                connection.close()
                raise
        return connection, address

    def process_request(self, request, address):
        self.slots.acquire()
        try:
            super().process_request(request, address)
        except BaseException:
            self.slots.release()
            raise

    def process_request_thread(self, request, address):
        try:
            super().process_request_thread(request, address)
        finally:
            self.slots.release()


context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.load_cert_chain(args.cert, args.key)
servers = [Server(args.http_port), Server(args.tls_port, context)]
for server in servers:
    threading.Thread(target=server.serve_forever, daemon=True).start()
record('ready', http_port=args.http_port, tls_port=args.tls_port)
print('Mobile network fixture ready on loopback', flush=True)
signal.signal(signal.SIGTERM, lambda *_: ended.set())
signal.signal(signal.SIGINT, lambda *_: ended.set())
ended.wait()
for server in servers:
    server.shutdown()
    server.server_close()
