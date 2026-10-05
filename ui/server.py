"""Local UI and free deterministic test service; no wallet or publication access."""
import argparse
import json
import re
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.request import urlopen

ROOT = Path(__file__).resolve().parent
ASSETS = {'/': ('index.html', 'text/html'), '/app.js': ('app.js', 'text/javascript'), '/style.css': ('style.css', 'text/css'), '/memo-budget.js': ('memo-budget.js', 'text/javascript')}

def summarize(text):
    """Extract up to three leading sentences. This is not an AI model."""
    sentences = re.split(r'(?<=[.!?])\s+', text.strip())
    return ' '.join(sentences[:3])

class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        if self.path != '/api/test-service/summarize':
            self.send_error(404)
            return
        # Reject cross-origin browser submissions and unbounded/chunked bodies.
        origin = self.headers.get('Origin')
        expected = f'http://127.0.0.1:{self.server.server_port}'
        if origin is not None and origin != expected:
            self.send_error(403)
            return
        try:
            length = int(self.headers.get('Content-Length', '0'))
            if self.headers.get('Transfer-Encoding') or not 0 < length <= 16384:
                self.send_error(413)
                return
            if self.headers.get('Content-Type', '').split(';')[0] != 'application/json':
                self.send_error(415)
                return
            body = json.loads(self.rfile.read(length))
            text = body.get('text') if isinstance(body, dict) else None
            if not isinstance(text, str) or not text.strip() or len(text) > 4000:
                raise ValueError()
        except (ValueError, UnicodeError):
            self.send_error(400)
            return
        data = json.dumps({'summary': summarize(text), 'method': 'first-three-sentences',
                           'payment_required': False, 'published_on_chain': False}).encode()
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(data)))
        self.send_header('Cache-Control', 'no-store')
        self.send_header('X-Content-Type-Options', 'nosniff')
        self.end_headers()
        self.wfile.write(data)


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
