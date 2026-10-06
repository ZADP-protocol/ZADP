"""Loopback-only purchase simulator. No wallet configuration, signing or broadcast.

Run separately from the real UI. --state-file controls synthetic confirmation
and failure states for manual/browser testing; it is never a payment endpoint.
"""
import argparse,hashlib,json,sys,tempfile,time
from pathlib import Path
from http.server import ThreadingHTTPServer
from server import Handler
sys.path.insert(0,str(Path(__file__).resolve().parent.parent/'services'/'text'))
from commerce import Commerce

class FixtureOperator:
    def __init__(self,database,state_file):
        self.commerce=Commerce(database,'local-simulation','fixture-account','utest1localfixture',1000)
        self.state_file=state_file
    def status(self):
        return {"available":False}
    def service_request(self,operation,body):
        state=json.loads(self.state_file.read_text()) if self.state_file.exists() else {'status':'awaiting_payment'}
        if state.get('status')=='unavailable':raise ValueError('Synthetic outage')
        if operation=='invoice':
            result=self.commerce.create('local-simulation',body['text'],request_key=body['request_key'])
        elif operation=='recover':result=self.commerce.recover('local-simulation',body['request_key'])
        elif operation=='fulfill':
            row=self.commerce.db.execute('SELECT * FROM invoices WHERE id=?',(body['invoice_id'],)).fetchone()
            transactions=[];tip=12
            if row and state.get('status') in ('confirming','fulfilled'):
                conf=state.get('confirmations',3 if state['status']=='fulfilled' else 1)
                txid=row['payment'].split(':')[0] if row['payment'] else hashlib.sha256(row['id'].encode()).hexdigest()
                transactions=[{'account_uuid':'fixture-account','mined_height':tip-conf+1,'block_time':row['created']+1,'txid':txid,'outputs':[{'pool':'orchard','output_index':0,'to_account':'fixture-account','to_address':'utest1localfixture','is_change':False,'value':1000,'memo':f"ZADP-PAY/1:{row['id']}"}]}]
            now=row['expires']+1 if row and state.get('status')=='expired' else int(time.time())
            return self.commerce.fulfill(body['invoice_id'],body['access_token'],transactions,tip,True,now=now)
        else:raise ValueError('Unsupported fixture operation')
        return {**result,'network':'testnet','local_unpublished_test':True,'simulation':True}

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--port',type=int,default=8093)
    parser.add_argument('--state-file',type=Path,required=True)
    parser.add_argument('--database',type=Path,help='Optional protected simulator database for restart testing')
    args=parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='zadp-purchase-simulator-') as directory:
        server=ThreadingHTTPServer(('127.0.0.1',args.port),Handler)
        server.operator=FixtureOperator(args.database or Path(directory)/'fixture.sqlite',args.state_file)
        server.upstream='http://127.0.0.1:8080'
        print(f'Local payment SIMULATOR at http://127.0.0.1:{args.port}; no funds or wallet access.',flush=True)
        server.serve_forever()
