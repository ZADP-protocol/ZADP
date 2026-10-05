import json
import tempfile
import threading
import unittest
from http.server import ThreadingHTTPServer
from pathlib import Path
from urllib.error import HTTPError
from urllib.request import Request,urlopen
from commerce import Commerce
from server import Handler

class FixtureObserver:
    def __init__(self): self.transactions=[]
    def snapshot(self): return self.transactions,12,True

class HttpFlow(unittest.TestCase):
    def test_invoice_payment_gate_result_and_restart_storage(self):
        with tempfile.TemporaryDirectory() as temp:
            service=Commerce(Path(temp)/'invoices.sqlite','test-offer','account','test-receiver',1000)
            server=ThreadingHTTPServer(('127.0.0.1',0),Handler)
            server.commerce=service;server.observer=FixtureObserver();server.network='testnet'
            thread=threading.Thread(target=server.serve_forever);thread.start()
            def post(path,body):
                request=Request(f'http://127.0.0.1:{server.server_port}'+path,
                    data=json.dumps(body).encode(),headers={'Content-Type':'application/json'})
                return json.load(urlopen(request))
            try:
                invoice=post('/v1/invoices',{'offer_id':'test-offer','text':'One. Two. Three. Four.'})
                access={k:invoice[k] for k in ('invoice_id','access_token')}
                self.assertEqual(post('/v1/fulfill',access)['status'],'awaiting_payment')
                with self.assertRaises(HTTPError):post('/v1/fulfill',{**access,'payment':'forged'})
                # Trusted fixture only: no simulated-payment endpoint exists in the server.
                import time
                server.observer.transactions=[{'account_uuid':'account','mined_height':10,
                    'block_time':int(time.time()),'txid':'a'*64,'outputs':[{
                    'pool':'orchard','output_index':0,'to_account':'account','to_address':'test-receiver',
                    'value':1000,'is_change':False,'memo':invoice['memo']}]}]
                result=post('/v1/fulfill',access)
                self.assertEqual(result['status'],'fulfilled')
                self.assertEqual(result['output'],'One. Two. Three.')
                self.assertEqual(post('/v1/fulfill',access),result)
            finally:
                server.shutdown();server.server_close();thread.join();service.db.close()

if __name__=='__main__': unittest.main()
