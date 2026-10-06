"""Loopback-only TLS test proxy. Never use its ephemeral certificate in production.

Only seller order routes are forwarded to a fixed loopback target. No access
logging, discovery proxy, payments, signing or broadcast operations exist here.
"""
import argparse,ssl,re
from http.server import BaseHTTPRequestHandler,ThreadingHTTPServer
from urllib.request import Request,urlopen
from urllib.error import HTTPError
class Handler(BaseHTTPRequestHandler):
    def forward(self):
        if not (self.path in ('/orders','/healthz') or re.fullmatch(r'/orders/[a-f0-9]{32}',self.path)):
            self.send_error(404);return
        try:
            length=int(self.headers.get('Content-Length','0'))
            if self.headers.get('Transfer-Encoding') or not 0<=length<=8192:self.send_error(413);return
            headers={key:self.headers[key] for key in ('Content-Type','Authorization','Idempotency-Key','Origin','Access-Control-Request-Method','Access-Control-Request-Headers') if key in self.headers}
            request=Request(self.server.upstream+self.path,data=self.rfile.read(length) if length else None,method=self.command,headers=headers)
            try:response=urlopen(request,timeout=25)
            except HTTPError as error:response=error
            with response:
                data=response.read(200001)
                if len(data)>200000:raise ValueError()
                self.send_response(response.status)
                for key in ('Content-Type','Cache-Control','Access-Control-Allow-Origin','Access-Control-Allow-Methods','Access-Control-Allow-Headers','Access-Control-Max-Age','Vary','Referrer-Policy'):
                    if response.headers.get(key):self.send_header(key,response.headers[key])
                self.send_header('Content-Length',str(len(data)));self.end_headers();self.wfile.write(data)
        except Exception:self.send_error(502,'Seller unavailable')
    do_GET=forward
    do_POST=forward
    do_DELETE=forward
    do_OPTIONS=forward
    def log_message(self,*args):pass
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--port',type=int,required=True);p.add_argument('--upstream-port',type=int,required=True);p.add_argument('--certificate',required=True);p.add_argument('--key',required=True);args=p.parse_args()
    context=ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER);context.load_cert_chain(args.certificate,args.key)
    server=ThreadingHTTPServer(('127.0.0.1',args.port),Handler);server.upstream=f'http://127.0.0.1:{args.upstream_port}'
    server.socket=context.wrap_socket(server.socket,server_side=True);server.serve_forever()
