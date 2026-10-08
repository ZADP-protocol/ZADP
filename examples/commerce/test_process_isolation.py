"""Cross-process offline acceptance, using the real persistent board repository.

Build beforehand: cargo build -p zadp-commerce --bins --examples
Run: python3 -m unittest discover -s examples/commerce -p 'test_*.py'
No wallet, real network scanning, transaction signing or broadcasting.
"""
import contextlib,json,os,shutil,socket,sqlite3,ssl,subprocess,sys,tempfile,time,unittest
from pathlib import Path
from urllib.request import Request,urlopen
from urllib.error import HTTPError,URLError
ROOT=Path(__file__).resolve().parents[2]
def free_port():
    with socket.socket() as sock:sock.bind(('127.0.0.1',0));return sock.getsockname()[1]
def wait_ready(url,context=None):
    end=time.monotonic()+10
    while time.monotonic()<end:
        try:
            with urlopen(url+'/healthz',timeout=.3,context=context) as response:
                if response.status==200:return
        except Exception:time.sleep(.05)
    raise AssertionError('Separate component did not become ready')
def database_state(path):
    with sqlite3.connect(f'file:{path}?mode=ro',uri=True) as db:return list(db.iterdump())
class ProcessIsolation(unittest.TestCase):
    def test_independent_components_https_and_private_seller_state(self):
        for binary in ('target/debug/zadp-commerce','target/debug/examples/fixture','target/debug/examples/fixture_indexer'):
            self.assertTrue((ROOT/binary).is_file(),'Build the Rust fixture examples first')
        with tempfile.TemporaryDirectory(prefix='zadp-separated-commerce-') as temporary,contextlib.ExitStack() as stack:
            base=Path(temporary);os.chmod(base,0o700)
            seller_dir=base/'seller';discovery_dir=base/'discovery';buyer_dir=base/'buyer';discovery_dir.mkdir(mode=0o700);buyer_dir.mkdir(mode=0o700)
            seller_port,tls_port,index_port,ui_port=free_port(),free_port(),free_port(),free_port()
            endpoint=f'https://127.0.0.1:{tls_port}';origin=f'http://127.0.0.1:{ui_port}'
            subprocess.run([str(ROOT/'target/debug/examples/fixture'),str(seller_dir),'--seller-endpoint',endpoint,'--browser-origin',origin],check=True,stdout=subprocess.DEVNULL)
            for filename in ('board.json','offers.json'):shutil.copyfile(seller_dir/filename,discovery_dir/filename)
            certificate=base/'test-ca.pem';key=base/'test-ca.key'
            subprocess.run(['openssl','req','-x509','-newkey','rsa:2048','-nodes','-days','1','-subj','/CN=127.0.0.1','-addext','subjectAltName=IP:127.0.0.1','-keyout',str(key),'-out',str(certificate)],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
            context=ssl.create_default_context(cafile=str(certificate))
            processes=[]
            def start(command):
                process=subprocess.Popen(command,cwd=ROOT,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL);processes.append(process)
                def stop():
                    if process.poll() is None:process.terminate()
                    try:process.wait(timeout=5)
                    except subprocess.TimeoutExpired:process.kill();process.wait()
                stack.callback(stop);return process
            seller_command=[str(ROOT/'target/debug/zadp-commerce'),'--config',str(seller_dir/'config.json'),'--bind',f'127.0.0.1:{seller_port}']
            seller=start(seller_command)
            proxy=start([sys.executable,'examples/commerce/https_fixture.py','--port',str(tls_port),'--upstream-port',str(seller_port),'--certificate',str(certificate),'--key',str(key)])
            indexer_command=[str(ROOT/'target/debug/examples/fixture_indexer'),'--directory',str(discovery_dir),'--bind',f'127.0.0.1:{index_port}']
            indexer=start(indexer_command)
            ui=start([sys.executable,'examples/browser/server.py','--port',str(ui_port),'--indexer-port',str(index_port)])
            wait_ready(endpoint,context)
            with self.assertRaises(URLError):urlopen(endpoint+'/healthz',timeout=2)
            preflight=Request(endpoint+'/orders',method='OPTIONS',headers={'Origin':origin,'Access-Control-Request-Method':'POST','Access-Control-Request-Headers':'content-type,idempotency-key'})
            with urlopen(preflight,context=context) as response:
                self.assertEqual(response.headers['Access-Control-Allow-Origin'],origin)
                self.assertIn('POST',response.headers['Access-Control-Allow-Methods'])
            wait_ready(f'http://127.0.0.1:{index_port}')
            end=time.monotonic()+10
            while True:
                try:
                    with urlopen(origin+'/api/offers',timeout=.5) as response:offers=json.load(response)['offers']
                    break
                except Exception:
                    if time.monotonic()>end:raise
                    time.sleep(.05)
            self.assertEqual(len(offers),1);offer=offers[0];self.assertEqual(offer['endpoint'],endpoint);self.assertTrue(offer['simulation'])
            self.assertEqual(offer['chain']['confirmations'],3)
            with urlopen(origin) as response:self.assertIn(b'Discovery reference client',response.read())
            before=database_state(discovery_dir/'index.sqlite')
            original_public={p.name:p.read_bytes() for p in discovery_dir.glob('*.json')}
            input_text='cross-process private input: hello world'
            text_file=buyer_dir/'input.txt';text_file.write_text(input_text);os.chmod(text_file,0o600)
            recovery=buyer_dir/'recovery.json'
            def buyer(operation):
                command=[sys.executable,'examples/commerce/buyer.py',operation,'--endpoint',endpoint,'--recovery',str(recovery),'--ca-cert',str(certificate),'--demo']
                if operation=='create':command+=['--offer',offer['id'],'--text-file',str(text_file)]
                result=subprocess.run(command,cwd=ROOT,check=True,capture_output=True,text=True);return json.loads(result.stdout)
            order=buyer('create');self.assertEqual(order['status'],'waiting_payment');self.assertNotIn('claim_token',order)
            saved=json.loads(recovery.read_text());claim=saved['claim_token']
            # Invalid claim is rejected at the seller, with no discovery request.
            with self.assertRaises(HTTPError) as invalid:urlopen(Request(endpoint+'/orders/'+order['order_id'],headers={'Authorization':'Bearer invalid'}),context=context)
            self.assertEqual(invalid.exception.code,401)
            def payment(confirmations):subprocess.run([sys.executable,'examples/commerce/mock_payment.py','--directory',str(seller_dir),'--order',order['order_id'],'--confirmations',str(confirmations)],cwd=ROOT,check=True,stdout=subprocess.DEVNULL)
            payment(1);pending=buyer('check');self.assertEqual(pending['payment_check'],'confirming');self.assertNotIn('result',pending)
            payment(3);self.assertEqual(buyer('check')['status'],'paid');self.assertEqual(buyer('check')['status'],'processing');result=buyer('check');self.assertEqual(result['result'],input_text.upper())
            self.assertEqual(database_state(discovery_dir/'index.sqlite'),before)
            self.assertEqual({p.name:p.read_bytes() for p in discovery_dir.glob('*.json')},original_public)
            index_dump=''.join(before)
            for private in (claim,saved['key'],order['memo'],order['order_id'],input_text,result['result']):self.assertFalse(private in index_dump,'Private commerce data leaked into fixture discovery')
            for component in (f'http://127.0.0.1:{index_port}',origin):
                with self.assertRaises(HTTPError):urlopen(component+'/orders/'+order['order_id'])
            # Stop discovery/UI: a fresh buyer process can still retrieve its order.
            ui.terminate();ui.wait(timeout=5);indexer.terminate();indexer.wait(timeout=5)
            self.assertEqual(buyer('check')['result'],result['result'])
            # Consistent protected seller-only backup before the isolated restart.
            with sqlite3.connect(seller_dir/'orders.sqlite') as db,sqlite3.connect(seller_dir/'restart-backup.sqlite') as backup:db.backup(backup)
            os.chmod(seller_dir/'restart-backup.sqlite',0o600)
            seller.terminate();seller.wait(timeout=5);seller=start(seller_command);wait_ready(f'http://127.0.0.1:{seller_port}')
            self.assertEqual(buyer('check')['result'],result['result'])
            with sqlite3.connect(seller_dir/'orders.sqlite') as db:self.assertEqual(db.execute('SELECT execution_count FROM orders').fetchone()[0],1)
            # Reconstruct the board in a new DB; commerce has no dependency on its cursor/state.
            for file in discovery_dir.glob('index.sqlite*'):file.unlink()
            indexer=start(indexer_command);wait_ready(f'http://127.0.0.1:{index_port}')
            with urlopen(f'http://127.0.0.1:{index_port}/offers') as response:self.assertEqual(json.load(response)['offers'][0]['id'],offer['id'])
            self.assertEqual(buyer('check')['result'],result['result'])
            self.assertEqual(database_state(discovery_dir/'index.sqlite'),before)
            self.assertEqual(len({p.pid for p in processes}),len(processes))
            self.assertEqual(set(p.name for p in discovery_dir.iterdir())-{'index.sqlite-wal','index.sqlite-shm'},{'board.json','offers.json','index.sqlite'})
if __name__=='__main__':unittest.main()
