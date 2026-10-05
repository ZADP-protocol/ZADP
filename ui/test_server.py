import threading
import unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.error import HTTPError
from urllib.request import Request, urlopen
from server import Handler

class Upstream(BaseHTTPRequestHandler):
    paths = []
    def do_GET(self):
        self.paths.append(self.path)
        self.send_response(200)
        self.end_headers()
        self.wfile.write(b'{"offers":[]}' if self.path == '/offers' else b'ok')
    def log_message(self, *args): pass

class ServerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.upstream = ThreadingHTTPServer(('127.0.0.1', 0), Upstream)
        cls.server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        cls.server.upstream = f'http://127.0.0.1:{cls.upstream.server_port}'
        cls.url = f'http://127.0.0.1:{cls.server.server_port}'
        for server in (cls.upstream, cls.server):
            threading.Thread(target=server.serve_forever, daemon=True).start()
    @classmethod
    def tearDownClass(cls):
        for server in (cls.server, cls.upstream):
            server.shutdown(); server.server_close()
    def test_public_offers_forwarded(self):
        with urlopen(self.url + '/api/offers') as response:
            self.assertEqual(response.read(), b'{"offers":[]}')
        self.assertEqual(Upstream.paths[-1], '/offers')
    def test_no_wallet_routes_traversal_or_posts(self):
        for route in ('/api/wallet', '/api/offers?url=http://other', '/../server.py', '/server.py'):
            with self.assertRaises(HTTPError) as error: urlopen(self.url + route)
            self.assertEqual(error.exception.code, 404)
        with self.assertRaises(HTTPError) as error:
            urlopen(Request(self.url + '/api/offers', data=b'{}'))
        self.assertEqual(error.exception.code, 501)
    def test_asset_has_security_headers(self):
        with urlopen(self.url) as response:
            self.assertIn(b'ZADP', response.read())
            self.assertIn("script-src 'self'", response.headers['Content-Security-Policy'])
    def test_unavailable_index_returns_error(self):
        previous = self.server.upstream
        self.server.upstream = 'http://127.0.0.1:1'
        try:
            with self.assertRaises(HTTPError) as error: urlopen(self.url + '/api/offers')
            self.assertEqual(error.exception.code, 502)
        finally: self.server.upstream = previous

if __name__ == '__main__': unittest.main()
