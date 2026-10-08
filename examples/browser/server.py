"""Minimal reference client and fixed loopback discovery proxy, not hosted product UX."""
import argparse,json
from pathlib import Path
from http.server import BaseHTTPRequestHandler,ThreadingHTTPServer
from urllib.request import urlopen
ROOT=Path(__file__).resolve().parents[2]
ASSETS={'/':('examples/browser/index.html','text/html'),'/offers.js':('client/browser/offers.js','text/javascript')}
class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path in ASSETS:
            name,mime=ASSETS[self.path];body=(ROOT/name).read_bytes()
        elif self.path in ('/api/offers','/api/healthz'):
            try:
                with urlopen(self.server.upstream+self.path[4:],timeout=5) as r:body=r.read(4*1024*1024+1)
                if len(body)>4*1024*1024:raise ValueError()
                mime='application/json' if self.path.endswith('offers') else 'text/plain'
            except Exception:self.send_error(502);return
        else:self.send_error(404);return
        self.send_response(200);self.send_header('Content-Type',mime);self.send_header('Cache-Control','no-store');self.end_headers();self.wfile.write(body)
    def log_message(self,*args):pass # Buyer requests/coordinates are never logged.
def main():
    p=argparse.ArgumentParser();p.add_argument('--port',type=int,default=8090);p.add_argument('--indexer-port',type=int,default=8080);a=p.parse_args()
    s=ThreadingHTTPServer(('127.0.0.1',a.port),Handler);s.upstream=f'http://127.0.0.1:{a.indexer_port}';s.serve_forever()
if __name__=='__main__':main()
