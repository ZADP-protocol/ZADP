import json,os,tempfile,unittest
from pathlib import Path
from server import self_test_journal

class JournalTests(unittest.TestCase):
    def test_explicit_owner_only_broadcast_journal_and_binding(self):
        config={'network':'mainnet','price_zat':1000}
        data={'stage':'broadcast_accepted','self_payment':True,'network':'mainnet','amount_zat':1000,'invoice_id':'a'*32,'txid':'b'*64}
        self.assertIsNone(self_test_journal(None,config))
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'journal.json';path.write_text(json.dumps(data));os.chmod(path,0o600)
            self.assertEqual(self_test_journal(path,config)['txid'],data['txid'])
            for key,value in [('stage','prepared_no_retry'),('network','testnet'),('amount_zat',999),('self_payment',False),('txid','g'*64),('invoice_id','short')]:
                bad=dict(data);bad[key]=value;path.write_text(json.dumps(bad))
                with self.assertRaises(ValueError):self_test_journal(path,config)
            path.write_text(json.dumps(data));os.chmod(path,0o644)
            with self.assertRaises(ValueError):self_test_journal(path,config)
            os.chmod(path,0o600);link=Path(directory)/'link';link.symlink_to(path)
            with self.assertRaises(OSError):self_test_journal(link,config)
