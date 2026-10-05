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
        self.assertEqual(self.run_order(tip=11)['status'],'awaiting_payment')
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
    def test_input_limits_and_unknown_offer(self):
        with self.assertRaises(ValueError):self.service.create('other','hello')
        with self.assertRaises(ValueError):self.service.create('offer-test','x'*4001)
        with self.assertRaises(ValueError):self.service.create('offer-test','')

if __name__=='__main__': unittest.main()
