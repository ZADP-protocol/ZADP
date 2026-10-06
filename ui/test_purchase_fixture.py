import json,tempfile,unittest
from pathlib import Path
from dev_purchase_fixture import FixtureOperator

class FixtureTests(unittest.TestCase):
    def test_separate_simulated_orders_have_distinct_payment_evidence(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);state=root/'state.json';state.write_text(json.dumps({'status':'fulfilled'}))
            operator=FixtureOperator(root/'fixture.sqlite',state)
            try:
                ids=[]
                for key in ('a'*64,'b'*64):
                    invoice=operator.service_request('invoice',{'text':'One. Two. Three. Four.','request_key':key})
                    self.assertTrue(invoice['simulation'])
                    result=operator.service_request('fulfill',{k:invoice[k] for k in ('invoice_id','access_token')})
                    self.assertEqual(result['status'],'fulfilled');ids.append(result['txid'])
                self.assertNotEqual(*ids)
                self.assertFalse(operator.status()['available'])
            finally:operator.commerce.db.close()
