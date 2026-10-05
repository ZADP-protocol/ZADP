"""Protected local read-only status and unsigned CLI preview boundary."""
import base64
import json
import os
import stat
import ssl
import subprocess
import tempfile
import threading
import urllib.request
from pathlib import Path
from urllib.parse import urlsplit


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, file, code, message, headers, new_url):
        raise ValueError('Redirects are not allowed in the local boundary')


class Operator:
    def __init__(self, config_path):
        path=Path(config_path)
        info=path.lstat()
        if not stat.S_ISREG(info.st_mode) or info.st_mode & 0o077:
            raise ValueError('Operator config must be a protected regular file')
        self.config=json.loads(path.read_text())
        self.lock=threading.Lock()
        url=urlsplit(self.config['wallet_rpc'])
        if url.scheme!='http' or url.hostname not in ('127.0.0.1','::1') or url.username or url.password or url.query or url.fragment:
            raise ValueError('Wallet RPC must use loopback')
        self.client=urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())

    def rpc(self, method, params):
        if method not in ('getwalletstatus','z_getbalances'): raise ValueError()
        fd=os.open(self.config['wallet_auth_file'],os.O_RDONLY|os.O_NOFOLLOW)
        with os.fdopen(fd,'rb') as file:
            info=os.fstat(file.fileno())
            if not stat.S_ISREG(info.st_mode) or info.st_mode & 0o077: raise ValueError()
            auth=file.read(4097).strip()
        if len(auth)>4096: raise ValueError()
        request=urllib.request.Request(self.config['wallet_rpc'],
            data=json.dumps({'jsonrpc':'2.0','id':'ui-status','method':method,'params':params}).encode(),
            headers={'Content-Type':'application/json','Authorization':'Basic '+base64.b64encode(auth).decode()})
        result=json.load(self.client.open(request,timeout=10))
        if result.get('error'): raise ValueError()
        return result['result']

    def status(self):
        try:
            status=self.rpc('getwalletstatus',[])
            balances=self.rpc('z_getbalances',[3])
            account=next(a for a in balances['accounts'] if a['account_uuid']==self.config['account_uuid'])
            fields=('spendable','pending','locked','dust')
            total={field:account.get('total',{}).get(field,{}).get('valueZat') for field in fields}
            return {'available':True,'network':self.config['network'],
                    'tip':status.get('node_tip',{}).get('height'),
                    'wallet_scanned_height':status.get('fully_synced_height'),
                    'sync_locked':status.get('locked'), 'seller_total_zat':total,
                    'receive_address':self.config['receive_address']}
        except Exception: return {'available':False}

    def preview(self, draft):
        if not self.lock.acquire(blocking=False): raise ValueError('Preview is already running')
        try:
            if self.config['network'] not in ('mainnet','testnet'): raise ValueError()
            board=json.loads(Path(self.config['board']).read_text())
            if draft.get('board_id') != board['board']['board_id']: raise ValueError('Board mismatch')
            for name in ('title','capability','endpoint','price'):
                if not isinstance(draft.get(name),str) or len(draft[name])>2048: raise ValueError()
            description=draft.get('description','')
            if not isinstance(description,str) or len(description)>504: raise ValueError()
            with tempfile.TemporaryDirectory(prefix='zadp-preview-') as temp:
                output=Path(temp)/'preview.json'
                command=[self.config['seller_binary'],'publish','--dry-run','--preview-output',str(output),
                    '--network',self.config['network'],'--board',self.config['board'],
                    '--identity',self.config['seller_identity'],'--wallet-rpc',self.config['wallet_rpc'],
                    '--wallet-auth-file',self.config['wallet_auth_file'],'--wallet-account',self.config['account_uuid'],
                    '--lightwalletd',self.config['lightwalletd'],'--title',draft['title'],
                    '--capability',draft['capability'],'--endpoint',draft['endpoint'],
                    '--price-zec',draft['price'],'--ttl-seconds','86400']
                if description: command+=['--description',description]
                result=subprocess.run(command,stdin=subprocess.DEVNULL,capture_output=True,timeout=90)
                if result.returncode!=0 or not output.exists(): raise ValueError('Unsigned preview unavailable')
                preview=json.loads(output.read_text())
                if preview.get('broadcast') is not False or preview.get('zcash_transaction_signed') is not False:
                    raise ValueError('Invalid preview')
                return preview
        finally: self.lock.release()

    def service_request(self, operation, body):
        """Fixed local HTTPS endpoint only; no offer-controlled network requests."""
        config=self.config['text_service']
        url=urlsplit(config['endpoint'])
        if (url.scheme!='https' or url.hostname!='127.0.0.1' or url.username
            or url.password or url.query or url.fragment or url.path not in ('','/')):
            raise ValueError('Service must be fixed loopback HTTPS')
        if operation=='invoice':
            if set(body)!={'text'}: raise ValueError()
            text=body['text']
            if not isinstance(text,str) or not text.strip() or len(text)>4000: raise ValueError()
            payload={'offer_id':config['offer_id'],'text':text}; route='/v1/invoices'
        elif operation=='fulfill':
            if set(body)!={'invoice_id','access_token'}: raise ValueError()
            if not all(isinstance(v,str) and len(v)<=128 for v in body.values()): raise ValueError()
            payload=body; route='/v1/fulfill'
        else: raise ValueError()
        context=ssl.create_default_context(cafile=config['tls_ca'])
        client=urllib.request.build_opener(urllib.request.ProxyHandler({}),
            urllib.request.HTTPSHandler(context=context), NoRedirect())
        request=urllib.request.Request(config['endpoint'].rstrip('/')+route,
            data=json.dumps(payload).encode(),headers={'Content-Type':'application/json'})
        with client.open(request,timeout=30) as response:
            data=response.read(16385)
        if len(data)>16384: raise ValueError()
        result=json.loads(data)
        if operation=='invoice':
            if (result['pay_to']!=self.config['receive_address'] or result['offer_id']!=config['offer_id']
                or result['amount_zat']!=config['price_zat']): raise ValueError('Invoice mismatch')
            result['network']=self.config['network']
            result['local_unpublished_test']=True
        return result
