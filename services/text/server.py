"""Loopback seller endpoint: invoices -> read-only shielded payment -> result."""
import argparse
import json
import os
import stat
import ssl
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from commerce import Commerce
from observer import WalletObserver


def self_test_journal(path, config):
    """Operator-only opt-in: pin a single already-broadcast local test payment."""
    if path is None: return None
    fd=os.open(path,os.O_RDONLY|os.O_NOFOLLOW)
    with os.fdopen(fd,'rb') as file:
        info=os.fstat(file.fileno())
        if not stat.S_ISREG(info.st_mode) or info.st_mode & 0o077 or info.st_uid!=os.getuid():
            raise ValueError('Self-payment journal must be a protected regular file')
        data=file.read(4097)
    if len(data)>4096: raise ValueError('Self-payment journal too large')
    journal=json.loads(data)
    if (journal.get('stage')!='broadcast_accepted' or journal.get('self_payment') is not True
        or journal.get('network')!=config['network']
        or type(journal.get('amount_zat')) is not int or journal['amount_zat']!=config['price_zat']):
        raise ValueError('Self-payment journal mismatch')
    for key,size in [('invoice_id',32),('txid',64)]:
        value=journal.get(key)
        if not isinstance(value,str) or len(value)!=size or any(c not in '0123456789abcdef' for c in value):
            raise ValueError('Invalid self-payment journal identifier')
    return {key:journal[key] for key in ('invoice_id','txid','amount_zat')}


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass  # Never log tokens, request inputs, addresses or wallet responses.

    def respond(self, status, value):
        data=json.dumps(value,ensure_ascii=False).encode('utf-8')
        self.send_response(status)
        self.send_header('Content-Type','application/json')
        self.send_header('Content-Length',str(len(data)))
        self.send_header('Cache-Control','no-store')
        self.send_header('X-Content-Type-Options','nosniff')
        self.end_headers();self.wfile.write(data)

    def do_GET(self):
        if self.path != '/healthz': return self.respond(404,{'error':'not_found'})
        self.respond(200,{'service':'deterministic-text','network':self.server.network,
                         'signs_or_broadcasts':False})

    def do_POST(self):
        if self.path not in ('/v1/invoices','/v1/fulfill','/v1/recover'):
            return self.respond(404,{'error':'not_found'})
        if self.headers.get('Host') != f'127.0.0.1:{self.server.server_port}':
            return self.respond(403,{'error':'host_rejected'})
        origin=self.headers.get('Origin')
        scheme='https' if isinstance(self.connection,ssl.SSLSocket) else 'http'
        if origin is not None and origin != f'{scheme}://127.0.0.1:{self.server.server_port}':
            return self.respond(403,{'error':'origin_rejected'})
        try:
            length=int(self.headers.get('Content-Length','0'))
            if self.headers.get('Transfer-Encoding') or not 0 < length <= 16384:
                return self.respond(413,{'error':'body_too_large'})
            if self.headers.get('Content-Type','').split(';')[0] != 'application/json':
                return self.respond(415,{'error':'json_required'})
            self.connection.settimeout(10)
            request=json.loads(self.rfile.read(length))
            if not isinstance(request,dict): raise ValueError()
            if self.path == '/v1/invoices':
                if set(request) not in ({'offer_id','text'},{'offer_id','text','request_key'}): raise ValueError()
                result=self.server.commerce.create(request['offer_id'],request['text'],request_key=request.get('request_key'))
                return self.respond(201,result)
            if self.path == '/v1/recover':
                if set(request) != {'offer_id','request_key'}: raise ValueError()
                return self.respond(200,self.server.commerce.recover(request['offer_id'],request['request_key']))
            if set(request) != {'invoice_id','access_token'}: raise ValueError()
            if not all(isinstance(v,str) and len(v)<=128 for v in request.values()): raise ValueError()
            # No client-provided payment evidence is ever accepted.
            transactions,tip,synced=self.server.observer.snapshot()
            result=self.server.commerce.fulfill(request['invoice_id'],request['access_token'],transactions,tip,synced)
            return self.respond(200,result)
        except PermissionError:
            return self.respond(403,{'error':'invalid_invoice_access'})
        except (ValueError,TypeError,UnicodeError):
            return self.respond(400,{'error':'invalid_request'})
        except Exception:
            return self.respond(503,{'error':'service_unavailable'})


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--config',required=True,help='Protected local seller config; never commit it')
    parser.add_argument('--database',required=True,help='Private persistent invoice database')
    parser.add_argument('--tls-cert',required=True)
    parser.add_argument('--tls-key',required=True)
    parser.add_argument('--port',type=int,default=8092)
    parser.add_argument('--self-payment-journal',help='Opt-in local self-payment test: pin one protected broadcast journal')
    args=parser.parse_args()
    os.umask(0o077)
    path=Path(args.config)
    if not stat.S_ISREG(path.lstat().st_mode) or path.stat().st_mode & 0o077 or path.stat().st_uid != os.getuid():
        raise SystemExit('Config must be an owner-only regular file')
    config=json.loads(path.read_text())
    if config['network'] not in ('testnet','mainnet'): raise SystemExit('Choose an explicit network')
    prefix='utest1' if config['network']=='testnet' else 'u1'
    if not config['receive_address'].startswith(prefix): raise SystemExit('Receiver/network mismatch')
    server=ThreadingHTTPServer(('127.0.0.1',args.port),Handler)
    server.commerce=Commerce(args.database,config['offer_id'],config['account_uuid'],
                            config['receive_address'],config['price_zat'],
                            self_test_payment=self_test_journal(args.self_payment_journal,config))
    server.observer=WalletObserver(config['wallet_rpc'],config['wallet_auth_file'],config['account_uuid'])
    server.network=config['network']
    context=ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.minimum_version=ssl.TLSVersion.TLSv1_2
    key=Path(args.tls_key)
    if not stat.S_ISREG(key.lstat().st_mode) or key.stat().st_mode & 0o077 or key.stat().st_uid != os.getuid():
        raise SystemExit('TLS key must be an owner-only regular file')
    context.load_cert_chain(args.tls_cert,args.tls_key)
    server.socket=context.wrap_socket(server.socket,server_side=True)
    print(f'Local seller service listening on port {args.port}',flush=True)
    server.serve_forever()

if __name__=='__main__': main()
