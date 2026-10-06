import tempfile
import unittest
from pathlib import Path
from commerce import Commerce


class CommerceTests(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory()
        self.path=Path(self.tmp.name)/'service.sqlite'
        self.service=Commerce(self.path,'offer-test','seller-account','test-receiver',1000)
        self.invoice=self.service.create('offer-test','One. Two. Three. Four.',now=100)
        self.output={'pool':'ironwood','output_index':0,'to_account':'seller-account',
                     'to_address':'test-receiver','is_change':False,'value':1000,'memo':self.invoice['memo']}
        self.tx={'account_uuid':'seller-account','mined_height':10,'block_time':120,'txid':'a'*64,'outputs':[self.output]}
    def tearDown(self):
        self.service.db.close();self.tmp.cleanup()
    def run_order(self, transactions=None,tip=12,synced=True,token=None,now=150):
        return self.service.fulfill(self.invoice['invoice_id'],token or self.invoice['access_token'],
            [self.tx] if transactions is None else transactions,tip,synced,now=now)
    def test_only_confirmed_exact_shielded_payment_fulfills(self):
        self.assertEqual(self.run_order(tip=11),{'status':'confirming','confirmations':2,'confirmations_required':3,'txid':self.tx['txid']})
        result=self.run_order()
        self.assertEqual(result['output'],'One. Two. Three.')
        self.assertEqual(self.run_order(),result)
        self.assertEqual(self.run_order(transactions=[])['status'],'awaiting_payment')
    def test_reject_wrong_amount_receiver_account_memo_change_pool(self):
        for key,value in [('value',999),('to_address','other'),('to_account','other'),
                          ('memo','other'),('is_change',True),('pool','transparent')]:
            old=self.output[key];self.output[key]=value
            self.assertEqual(self.run_order()['status'],'awaiting_payment',key)
            self.output[key]=old
    def test_self_payment_requires_exact_operator_pinned_transaction(self):
        self.output['is_change']=True
        self.assertEqual(self.run_order()['status'],'awaiting_payment')
        self.service.self_test_payment={'txid':self.tx['txid'],'invoice_id':self.invoice['invoice_id'],'amount_zat':1000}
        self.assertTrue(self.run_order()['self_payment_test'])
        for key,value in [('txid','b'*64),('invoice_id','other'),('amount_zat',999)]:
            old=self.service.self_test_payment[key];self.service.self_test_payment[key]=value
            self.assertEqual(self.run_order()['status'],'awaiting_payment')
            self.service.self_test_payment[key]=old
        self.output['memo']='other'
        self.assertEqual(self.run_order()['status'],'awaiting_payment')

    def test_auth_expiry_and_sync_fail_closed(self):
        with self.assertRaises(PermissionError): self.run_order(token='wrong')
        self.assertEqual(self.run_order(synced=False)['status'],'observer_unavailable')
        self.assertEqual(self.run_order(transactions=[],now=2000)['status'],'expired')
        self.tx['block_time']=2000
        self.assertEqual(self.run_order(now=2100)['status'],'expired')
    def test_restart_preserves_invoice_and_result(self):
        result=self.run_order();self.service.db.close()
        self.service=Commerce(self.path,'offer-test','seller-account','test-receiver',1000)
        self.assertEqual(self.run_order(),result)
    def test_idempotent_creation_and_recovery_survive_restart(self):
        key='c'*64
        first=self.service.create('offer-test','Saved text.',now=100,request_key=key)
        self.assertEqual(self.service.create('offer-test','Saved text.',now=200,request_key=key),first)
        self.assertEqual(self.service.recover('offer-test',key),first)
        with self.assertRaises(ValueError):self.service.create('offer-test','Changed text.',request_key=key)
        with self.assertRaises(PermissionError):self.service.recover('offer-test','d'*64)
        self.service.db.close()
        self.service=Commerce(self.path,'offer-test','seller-account','test-receiver',1000)
        self.assertEqual(self.service.recover('offer-test',key),first)
        self.assertEqual(self.service.db.execute('SELECT count(*) FROM invoices WHERE request_hash IS NOT NULL').fetchone()[0],1)
        columns=self.service.db.execute('SELECT request_hash,token_hash FROM invoices WHERE request_hash IS NOT NULL').fetchone()
        self.assertNotIn(key,list(columns));self.assertNotIn(first['access_token'],list(columns))
        for bad in ('short','g'*64,None):
            with self.assertRaises(ValueError):self.service.recover('offer-test',bad)

    def test_legacy_schema_migration_preserves_paid_receipt(self):
        import sqlite3,hashlib,os
        path=Path(self.tmp.name)/'legacy.sqlite'
        connection=sqlite3.connect(path)
        connection.execute('CREATE TABLE invoices(id TEXT PRIMARY KEY,token_hash TEXT NOT NULL,offer_id TEXT NOT NULL,account TEXT NOT NULL,address TEXT NOT NULL,amount INTEGER NOT NULL,created INTEGER NOT NULL,expires INTEGER NOT NULL,input TEXT NOT NULL,output TEXT,payment TEXT UNIQUE)')
        connection.execute('INSERT INTO invoices VALUES(?,?,?,?,?,?,?,?,?,?,?)',('legacy',hashlib.sha256(b'legacy-token').hexdigest(),'offer-test','seller-account','test-receiver',1000,100,1900,'Original.','Original.','existing:orchard:0'))
        connection.commit();connection.close();os.chmod(path,0o600)
        migrated=Commerce(path,'offer-test','seller-account','test-receiver',1000)
        row=migrated.db.execute('SELECT * FROM invoices WHERE id=?',('legacy',)).fetchone()
        self.assertEqual(row['token_hash'],hashlib.sha256(b'legacy-token').hexdigest())
        self.assertEqual(row['payment'],'existing:orchard:0');self.assertEqual(row['output'],'Original.')
        self.assertIsNone(row['request_hash']);migrated.db.close()

    def test_concurrent_retries_create_one_invoice(self):
        from concurrent.futures import ThreadPoolExecutor
        with ThreadPoolExecutor(max_workers=8) as executor:
            results=list(executor.map(lambda _:self.service.create('offer-test','Same.',now=100,request_key='e'*64),range(16)))
        self.assertEqual(len({result['invoice_id'] for result in results}),1)

    def test_input_limits_and_unknown_offer(self):
        with self.assertRaises(ValueError):self.service.create('other','hello')
        with self.assertRaises(ValueError):self.service.create('offer-test','x'*4001)
        with self.assertRaises(ValueError):self.service.create('offer-test','')

if __name__=='__main__': unittest.main()
