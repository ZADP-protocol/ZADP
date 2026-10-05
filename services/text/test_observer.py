import tempfile
import unittest
from pathlib import Path
from unittest.mock import Mock
from observer import WalletObserver

class ObserverTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory()
        self.auth=Path(self.temp.name)/'fixture.wallet-auth'
        self.auth.write_text('fixture:fixture');self.auth.chmod(0o600)
        self.observer=WalletObserver('http://127.0.0.1:8232',self.auth,'fixture-account')
        self.status={'locked':False,'node_tip':{'height':12,'blockhash':'a'},'fully_synced_height':12}
    def tearDown(self):self.temp.cleanup()
    def test_snapshot_refuses_sync_lock_or_tip_change(self):
        self.observer._rpc=Mock(return_value={**self.status,'locked':True})
        self.assertEqual(self.observer.snapshot(),([],0,False))
        self.observer._rpc=Mock(side_effect=[self.status,[],{**self.status,'node_tip':{'height':13,'blockhash':'b'}}])
        self.assertEqual(self.observer.snapshot(),([],0,False))
    def test_pagination_is_account_scoped_and_refuses_truncation(self):
        self.observer._rpc=Mock(side_effect=[self.status]+[[{}]*100]*10)
        self.assertEqual(self.observer.snapshot(),([],0,False))
        for call in self.observer._rpc.call_args_list[1:]:
            self.assertEqual(call.args[0],'z_listtransactions')
            self.assertEqual(call.args[1][0],'fixture-account')
        self.observer._rpc=Mock(side_effect=[self.status,[],self.status])
        self.assertEqual(self.observer.snapshot(),([],12,True))
    def test_remote_urls_symlinks_and_non_read_only_methods_rejected(self):
        for url in ['https://evil.example','http://localhost:8232','http://u:p@127.0.0.1:8232','http://127.0.0.1:8232#fragment']:
            with self.assertRaises(ValueError):WalletObserver(url,self.auth,'fixture-account')
        with self.assertRaises(ValueError):self.observer._rpc('pczt_sign',[])
        link=Path(self.temp.name)/'symlink';link.symlink_to(self.auth)
        with self.assertRaises(ValueError):WalletObserver('http://127.0.0.1:8232',link,'fixture-account')
