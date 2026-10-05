"""Payment-gated deterministic service. No transaction signing or broadcasting.

Wallet observations must come from a trusted local observer, never HTTP clients.
"""
import hashlib
import os
import secrets
import sqlite3
import stat
import threading
import time
from pathlib import Path


def shorten(text):
    import re
    return ' '.join(re.split(r'(?<=[.!?])\s+', text.strip())[:3])


class Commerce:
    def __init__(self, database, offer_id, account, address, price_zat, confirmations=3):
        if not offer_id or not account or not address or type(price_zat) is not int or not 0 < price_zat <= 2100000000000000:
            raise ValueError('Invalid seller configuration')
        if not 1 <= confirmations <= 100:
            raise ValueError('Invalid confirmation threshold')
        self.offer_id, self.account, self.address = offer_id, account, address
        self.price_zat, self.confirmations = price_zat, confirmations
        self.lock = threading.RLock()
        path = Path(database)
        if not path.exists():
            path.touch(mode=0o600, exist_ok=False)
        if (not stat.S_ISREG(path.lstat().st_mode) or path.stat().st_mode & 0o077
            or path.stat().st_uid != os.getuid()):
            raise ValueError('Database must be owner-only')
        self.db = sqlite3.connect(path, check_same_thread=False)
        self.db.row_factory = sqlite3.Row
        self.db.executescript('''
          CREATE TABLE IF NOT EXISTS invoices (
            id TEXT PRIMARY KEY, token_hash TEXT NOT NULL, offer_id TEXT NOT NULL,
            account TEXT NOT NULL, address TEXT NOT NULL, amount INTEGER NOT NULL,
            created INTEGER NOT NULL, expires INTEGER NOT NULL, input TEXT NOT NULL,
            output TEXT, payment TEXT UNIQUE);
        ''')
        self.db.commit()

    def create(self, offer_id, text, now=None):
        now = int(time.time()) if now is None else now
        if offer_id != self.offer_id:
            raise ValueError('Unknown offer')
        if not isinstance(text, str) or not text.strip() or len(text) > 4000:
            raise ValueError('Input must contain 1 to 4000 characters')
        invoice_id, token = secrets.token_hex(16), secrets.token_urlsafe(32)
        with self.lock, self.db:
            total=self.db.execute('SELECT count(*) FROM invoices').fetchone()[0]
            if total >= 10000:
                raise ValueError('Invoice storage limit reached; operator review required')
            count = self.db.execute('SELECT count(*) FROM invoices WHERE expires > ?', (now,)).fetchone()[0]
            if count >= 100:
                raise ValueError('Service capacity reached')
            self.db.execute('INSERT INTO invoices(id,token_hash,offer_id,account,address,amount,created,expires,input) VALUES(?,?,?,?,?,?,?,?,?)',
                            (invoice_id, hashlib.sha256(token.encode()).hexdigest(), offer_id,
                             self.account, self.address, self.price_zat, now, now+1800, text))
        return {'invoice_id':invoice_id, 'access_token':token, 'offer_id':offer_id,
                'amount_zat':self.price_zat, 'currency':'ZEC', 'pay_to':self.address,
                'memo':f'ZADP-PAY/1:{invoice_id}', 'expires_at':now+1800,
                'confirmations_required':self.confirmations, 'status':'awaiting_payment'}

    def fulfill(self, invoice_id, token, transactions, tip, synchronized, now=None):
        """Transactions must be authenticated z_listtransactions observations.

        Re-check payment on every retrieval, even when a result was persisted,
        so a reorg cannot release previously cached results without confirmation.
        """
        now = int(time.time()) if now is None else now
        with self.lock:
            row = self.db.execute('SELECT * FROM invoices WHERE id=?', (invoice_id,)).fetchone()
            digest = hashlib.sha256(token.encode()).hexdigest()
            if row is None or not secrets.compare_digest(row['token_hash'], digest):
                raise PermissionError('Unknown invoice or invalid access token')
            if not synchronized:
                return {'status':'observer_unavailable'}
            payment = None
            for tx in transactions:
                height, block_time = tx.get('mined_height'), tx.get('block_time')
                txid = tx.get('txid', '')
                if (tx.get('account_uuid') != row['account'] or not isinstance(height,int)
                    or height > tip or tip-height+1 < self.confirmations
                    or not isinstance(block_time,int) or not row['created'] <= block_time <= row['expires']
                    or len(txid) != 64 or any(c not in '0123456789abcdef' for c in txid)):
                    continue
                for output in tx.get('outputs',[]):
                    index = output.get('output_index')
                    if (output.get('pool') not in ('sapling','orchard','ironwood')
                        or output.get('to_account') != row['account']
                        or output.get('to_address') != row['address']
                        or output.get('is_change') is not False
                        or output.get('value') != row['amount']
                        or output.get('memo') != f'ZADP-PAY/1:{invoice_id}'
                        or not isinstance(index,int) or index < 0):
                        continue
                    payment = f"{txid}:{output['pool']}:{index}"
                    break
                if payment: break
            if payment is None:
                return {'status':'expired' if now > row['expires'] else 'awaiting_payment'}
            # An output cannot be consumed by another invoice, nor swapped after fulfillment.
            if row['payment'] is not None and row['payment'] != payment:
                return {'status':'payment_mismatch'}
            try:
                with self.db:
                    result = row['output'] if row['output'] is not None else shorten(row['input'])
                    self.db.execute('UPDATE invoices SET output=?,payment=? WHERE id=?', (result,payment,invoice_id))
            except sqlite3.IntegrityError:
                return {'status':'payment_already_used'}
            return {'status':'fulfilled','invoice_id':invoice_id,'output':result}
