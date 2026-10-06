"""Supply one protected local mock observation, never a real payment.

No wallet calls and no remote payment callback. Optional confirmations let a
buyer verify that fulfillment waits for the seller's configured policy.
"""
import argparse,hashlib,json,os,sqlite3,time
from pathlib import Path

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--directory',type=Path,required=True)
    parser.add_argument('--order',required=True)
    parser.add_argument('--confirmations',type=int,default=3)
    args=parser.parse_args();directory=args.directory.resolve()
    config=json.loads((directory/'config.json').read_text())
    if config['observer']['mode']!='mock' or config['network']!='testnet':raise ValueError('Mock mode required')
    if not 1<=args.confirmations<=100:raise ValueError('Invalid confirmations')
    db=sqlite3.connect(f'file:{config["database"]}?mode=ro',uri=True);db.row_factory=sqlite3.Row
    row=db.execute('SELECT id,account,recipient,amount,memo,created,expires,payment FROM orders WHERE id=?',(args.order,)).fetchone()
    if row is None or not row['created']<=int(time.time())<=row['expires']:raise ValueError('Unknown or expired order')
    snapshot={'synchronized':True,'tip':100+args.confirmations-1,'payments':[{
        'txid':row['payment'].split(':')[0] if row['payment'] else hashlib.sha256(('mock:'+row['id']).encode()).hexdigest(),'pool':'orchard','output_index':0,'account':row['account'],
        'recipient':row['recipient'],'amount_zat':row['amount'],'memo':row['memo'],
        'is_change':False,'mined_height':100,'block_time':row['created']}]}
    path=Path(config['observer']['snapshot_file'])
    if path.is_symlink() or path.stat().st_mode & 0o077 or path.stat().st_uid!=os.getuid():raise ValueError('Protected snapshot file required')
    temporary=path.with_suffix('.tmp');fd=os.open(temporary,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
    with os.fdopen(fd,'w') as out:json.dump(snapshot,out)
    temporary.replace(path)
    print(f'Synthetic observation supplied: {args.confirmations} confirmations. No funds sent.')
if __name__=='__main__':main()
