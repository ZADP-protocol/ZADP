import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
from operator_boundary import Operator

class PreviewTests(unittest.TestCase):
    def test_preview_can_only_invoke_dry_run_and_does_not_return_credentials(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp);board=root/'board.json';board.write_text(json.dumps({'board':{'board_id':'board'}}))
            config={'wallet_rpc':'http://127.0.0.1:8232','board':str(board),'network':'mainnet',
                    'seller_binary':'/fake/zadp-seller','seller_identity':'/protected/identity',
                    'wallet_auth_file':'/protected/auth','account_uuid':'private-account',
                    'lightwalletd':'https://test.example','receive_address':'u1test'}
            path=root/'config.json';path.write_text(json.dumps(config));path.chmod(0o600)
            operator=Operator(path)
            def run(command,**kwargs):
                self.assertIn('--dry-run',command);self.assertNotIn('--yes',command)
                self.assertNotIn('broadcast',command)
                output=Path(command[command.index('--preview-output')+1])
                output.write_text(json.dumps({'broadcast':False,'zcash_transaction_signed':False,'total_cost_zat':26000}))
                return type('Result',(),{'returncode':0})()
            with patch('operator_boundary.subprocess.run',side_effect=run):
                result=operator.preview({'board_id':'board','title':'Test','capability':'llm',
                                         'endpoint':'https://example.com','price':'0.01'})
            self.assertEqual(result['total_cost_zat'],26000)
            self.assertNotIn('private-account',json.dumps(result))
            with self.assertRaises(ValueError):operator.preview({'board_id':'wrong'})
            draft={'board_id':'board','title':'Test','capability':'cleaning','endpoint':'https://example.com','price':'0.01','geo':{'lat_e5':3777493,'lon_e5':-12241942,'radius_m':50000}}
            def geo_run(command,**kwargs):
                self.assertEqual(command[command.index('--geo-lat-e5')+1],'3777493')
                self.assertEqual(command[command.index('--geo-lon-e5')+1],'-12241942')
                self.assertEqual(command[command.index('--geo-radius-m')+1],'50000')
                return run(command,**kwargs)
            with patch('operator_boundary.subprocess.run',side_effect=geo_run):operator.preview(draft)
            for bad in ({'lat_e5':0},{'lat_e5':0,'lon_e5':0,'radius_m':0},{'lat_e5':0,'lon_e5':0,'radius_m':500001}):
                with patch('operator_boundary.subprocess.run') as command:
                    with self.assertRaises(ValueError):operator.preview({**draft,'geo':bad})
                    command.assert_not_called()
    def test_paid_bridge_rejects_arbitrary_hosts_and_mismatched_receiver(self):
        with tempfile.TemporaryDirectory() as temp:
            path=Path(temp)/'config.json'
            config={'wallet_rpc':'http://127.0.0.1:8232','network':'mainnet','receive_address':'u1expected',
                'text_service':{'endpoint':'https://evil.example','tls_ca':'/fake/ca',
                                'offer_id':'local-preview','price_zat':1000}}
            path.write_text(json.dumps(config));path.chmod(0o600)
            operator=Operator(path)
            with self.assertRaises(ValueError):operator.service_request('invoice',{'text':'Hello.'})
            operator.config['text_service']['endpoint']='https://127.0.0.1:8092'
            import io
            response={'pay_to':'u1wrong','offer_id':'local-preview','amount_zat':1000}
            client=type('Client',(),{'open':lambda self,*args,**kwargs:io.BytesIO(json.dumps(response).encode())})()
            with patch('operator_boundary.ssl.create_default_context'),patch('operator_boundary.urllib.request.build_opener',return_value=client):
                with self.assertRaises(ValueError):operator.service_request('invoice',{'text':'Hello.'})
            with self.assertRaises(ValueError):operator.service_request('fulfill',{'invoice_id':'x','access_token':'y','payment':'forged'})

    def test_recovery_uses_fixed_tls_bridge_and_verifies_invoice(self):
        import io
        with tempfile.TemporaryDirectory() as temp:
            path=Path(temp)/'config.json'
            config={'wallet_rpc':'http://127.0.0.1:8232','network':'mainnet','receive_address':'u1expected',
                    'text_service':{'endpoint':'https://127.0.0.1:8092','tls_ca':'/fake/ca','offer_id':'local','price_zat':1000}}
            path.write_text(json.dumps(config));path.chmod(0o600);operator=Operator(path)
            calls=[]
            def opened(request,**kwargs):
                calls.append((request.full_url,json.loads(request.data)))
                return io.BytesIO(json.dumps({'pay_to':'u1expected','offer_id':'local','amount_zat':1000}).encode())
            client=type('Client',(),{'open':lambda self,*args,**kwargs:opened(*args,**kwargs)})()
            with patch('operator_boundary.ssl.create_default_context'),patch('operator_boundary.urllib.request.build_opener',return_value=client):
                result=operator.service_request('recover',{'request_key':'a'*64})
                self.assertEqual(result['network'],'mainnet');self.assertTrue(result['local_unpublished_test'])
                self.assertEqual(calls,[('https://127.0.0.1:8092/v1/recover',{'offer_id':'local','request_key':'a'*64})])
                for body in ({'request_key':'short'},{'request_key':'a'*64,'payment':'forged'}):
                    with self.assertRaises(ValueError):operator.service_request('recover',body)
            self.assertEqual(len(calls),1)

    def test_unprotected_config_refused(self):
        with tempfile.TemporaryDirectory() as temp:
            path=Path(temp)/'config.json';path.write_text('{}');path.chmod(0o644)
            with self.assertRaises(ValueError):Operator(path)

if __name__=='__main__':unittest.main()
