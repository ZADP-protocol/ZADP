/* Mirrors the 13-element WireOffer in zadp-protocol/src/memo.rs.
 * No keys or signatures are generated: only their fixed encoded lengths count.
 */
'use strict';
function cborHeadBytes(value) {
  value = BigInt(value);
  if (value < 24n) return 1;
  if (value <= 255n) return 2;
  if (value <= 65535n) return 3;
  if (value <= 4294967295n) return 5;
  return 9;
}
function textBytes(value) {
  const length = new TextEncoder().encode(value).length;
  return cborHeadBytes(length) + length;
}
function memoBudget(draft, issuedAt = Math.floor(Date.now() / 1000)) {
  const amount = String(draft.price || '');
  if (!/^\d{1,8}(?:\.\d{1,8})?$/.test(amount)) throw Error('Use a positive ZEC price with at most 8 decimal places.');
  const [whole, fraction = ''] = amount.split('.');
  const zat = BigInt(whole) * 100000000n + BigInt(fraction.padEnd(8, '0'));
  if (zat <= 0n || zat > 2100000000000000n) throw Error('Price must be greater than zero and no more than 21 million ZEC.');
  for (const key of ['board_id', 'title', 'capability', 'endpoint']) {
    if (typeof draft[key] !== 'string' || !draft[key].trim()) throw Error('Complete the board, name, capability and endpoint fields.');
    if (/[\u0000-\u001f\u007f]/.test(draft[key])) throw Error('Fields cannot contain control characters.');
  }
  let endpoint;
  try { endpoint = new URL(draft.endpoint); } catch { throw Error('Enter a valid HTTPS service endpoint.'); }
  if (endpoint.protocol !== 'https:' || !endpoint.hostname || endpoint.username || endpoint.password) throw Error('Use an HTTPS endpoint without embedded credentials.');
  // array(13), protocol version, board, ULID(26), public key(32), capability,
  // title, optional description, zat integer, endpoint, two times, nonce(16), signature(64).
  const description = draft.description || null;
  const used = 1 + 1 + textBytes(draft.board_id) + textBytes('0'.repeat(26)) + 34
    + textBytes(draft.capability) + textBytes(draft.title)
    + (description === null ? 1 : textBytes(description)) + cborHeadBytes(zat)
    + textBytes(draft.endpoint) + cborHeadBytes(issuedAt) + cborHeadBytes(issuedAt + 86400)
    + 17 + 66;
  return {used, remaining:504-used, limit:504, fits:used<=504, board:draft.board_id};
}
if (typeof module !== 'undefined') module.exports = {memoBudget, textBytes, cborHeadBytes};
