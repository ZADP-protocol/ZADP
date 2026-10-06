"""Direct seller CLI client. Stores only a protected recovery capability.

Create/check use the same API as the browser. No signing or payment sending.
Use an HTTPS seller endpoint, or explicit --demo for the local mock fixture.
"""
import argparse,json,os,secrets
from pathlib import Path
from urllib.request import Request,urlopen,build_opener,HTTPRedirectHandler
from urllib.parse import urlsplit
class NoRedirect(HTTPRedirectHandler):
    def redirect_request(self,*args):raise ValueError('Seller redirects are forbidden')
def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('operation',choices=['create','check'])
    parser.add_argument('--endpoint',required=True)
    parser.add_argument('--offer')
    parser.add_argument('--text-file',type=Path)
    parser.add_argument('--recovery',type=Path,required=True)
    parser.add_argument('--demo',action='store_true')
    args=parser.parse_args();url=urlsplit(args.endpoint)
    if url.username or url.password or url.query or url.fragment or url.path not in ('','/') or not (url.scheme=='https' or args.demo and args.endpoint=='http://127.0.0.1:8094'):raise ValueError('Safe seller origin required')
    endpoint=args.endpoint.rstrip('/')
    if args.recovery.exists():
        if args.recovery.is_symlink() or args.recovery.stat().st_mode & 0o077 or args.recovery.stat().st_uid!=os.getuid():raise ValueError('Protected recovery file required')
        saved=json.loads(args.recovery.read_text());assert saved['endpoint']==endpoint
    else:
        if args.operation!='create':raise ValueError('Recovery file needed')
        saved={'endpoint':endpoint,'key':secrets.token_hex(32),'offer_id':args.offer}
        fd=os.open(args.recovery,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
        with os.fdopen(fd,'w') as out:json.dump(saved,out)
    if args.operation=='create':
        if saved.get('order_id'):raise ValueError('Order exists; check it rather than create again')
        if not args.offer or args.offer!=saved['offer_id'] or not args.text_file:raise ValueError('Offer and input file required')
        req=Request(endpoint+'/orders',data=json.dumps({'offer_id':args.offer,'request':{'text':args.text_file.read_text()}}).encode(),headers={'Content-Type':'application/json','Idempotency-Key':saved['key']})
    else:req=Request(endpoint+'/orders/'+saved['order_id'],headers={'Authorization':'Bearer '+saved['claim_token']})
    with build_opener(NoRedirect()).open(req,timeout=25) as response:result=json.loads(response.read(200001))
    if args.operation=='create':
        saved.update(order_id=result['order_id'],claim_token=result['claim_token'])
        # Existing file is already owned/private and contains no wallet material.
        args.recovery.write_text(json.dumps(saved));result.pop('claim_token',None)
    result.pop('payment_qr_svg',None)
    print(json.dumps(result,indent=2))
if __name__=='__main__':main()
