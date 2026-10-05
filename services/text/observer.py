"""Read-only Zallet beta.3 observer; all errors and RPC bodies stay private."""
import base64
import json
import os
import stat
import urllib.request
from pathlib import Path
from urllib.parse import urlsplit


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, file, code, message, headers, new_url):
        raise ValueError('Redirects are not allowed in the local boundary')


class WalletObserver:
    def __init__(self, url, auth_path, account):
        parsed = urlsplit(url)
        if (parsed.scheme != 'http' or parsed.hostname not in ('127.0.0.1','::1')
            or parsed.username or parsed.password or parsed.query or parsed.fragment):
            raise ValueError('Observer requires credential-free loopback RPC URL')
        path = Path(auth_path)
        mode = path.lstat().st_mode
        if not stat.S_ISREG(mode) or mode & 0o077:
            raise ValueError('Authentication file must be a protected regular file')
        self.url, self.path, self.account = url, path, account
        self.client = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())

    def _rpc(self, method, params):
        if method not in ('getwalletstatus','z_listtransactions'):
            raise ValueError('Method is not read-only allowlisted')
        fd = os.open(self.path, os.O_RDONLY | os.O_NOFOLLOW)
        with os.fdopen(fd, 'rb') as auth:
            info = os.fstat(auth.fileno())
            if not stat.S_ISREG(info.st_mode) or info.st_mode & 0o077:
                raise ValueError('Unprotected authentication file')
            credentials = auth.read(4097).strip()
        if len(credentials) > 4096: raise ValueError('Invalid authentication file')
        request = urllib.request.Request(self.url,
            data=json.dumps({'jsonrpc':'2.0','id':'payment-observer','method':method,'params':params}).encode(),
            headers={'Content-Type':'application/json','Authorization':'Basic '+base64.b64encode(credentials).decode()})
        with self.client.open(request,timeout=10) as response:
            value = json.load(response)
        if value.get('error'): raise ValueError('Read-only observation failed')
        return value['result']

    def snapshot(self):
        try:
            status = self._rpc('getwalletstatus', [])
            height = status['node_tip']['height']
            synced = status.get('locked') is False and status.get('fully_synced_height') == height
            if not synced: return [], 0, False
            # Bounded pagination: refuse incomplete history rather than declaring payment.
            transactions = []
            for offset in range(0,1000,100):
                page = self._rpc('z_listtransactions',[self.account,None,None,offset,100])
                if not isinstance(page,list): raise ValueError()
                transactions.extend(page)
                if len(page) < 100:
                    final = self._rpc('getwalletstatus', [])
                    if (final.get('locked') is False and final.get('node_tip') == status['node_tip']
                        and final.get('fully_synced_height') == height):
                        return transactions,height,True
                    return [],0,False
            return [],0,False
        except Exception:
            return [],0,False
