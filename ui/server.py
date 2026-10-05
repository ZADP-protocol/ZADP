"""Local read-only UI. Fixed allowlist proxy; no wallet or publication access."""
import argparse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.request import urlopen

ROOT = Path(__file__).resolve().parent
ASSETS = {'/': ('index.html', 'text/html'), '/app.js': ('app.js', 'text/javascript'), '/style.css': ('style.css', 'text/css')}

class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path in ASSETS:
            name, mime = ASSETS[self.path]
            data = (ROOT / name).read_bytes()
            status = 200
        elif self.path in ('/api/offers', '/api/healthz'):
            try:
                with urlopen(self.server.upstream + self.path[4:], timeout=5) as response:
                    data = response.read(2_000_001)
                    if len(data) > 2_000_000:
                        raise ValueError('Response too large')
                mime, status = 'application/json' if self.path.endswith('offers') else 'text/plain', 200
            except Exception:
                data, mime, status = b'{"error":"Indexer unavailable"}', 'application/json', 502
        else:
            self.send_error(404)
            return
        self.send_response(status)
        self.send_header('Content-Type', mime + '; charset=utf-8')
        self.send_header('Content-Length', str(len(data)))
        self.send_header('Cache-Control', 'no-store')
        self.send_header('X-Content-Type-Options', 'nosniff')
        self.send_header('Content-Security-Policy', "default-src 'self'; script-src 'self'; style-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'")
        self.end_headers()
        self.wfile.write(data)

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--port', type=int, default=8090)
    parser.add_argument('--indexer-port', type=int, default=8080)
    args = parser.parse_args()
    server = ThreadingHTTPServer(('127.0.0.1', args.port), Handler)
    server.upstream = f'http://127.0.0.1:{args.indexer_port}'
    print(f'ZADP UI: http://127.0.0.1:{args.port}', flush=True)
    server.serve_forever()
