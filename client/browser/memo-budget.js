/* Mirrors the Protocol 1.0 14-element wire 3 Offer in zadp-protocol/src/memo.rs.
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
  const version=draft.offer_version??draft.zadp??3;
  if(version!==3)throw Error('Pre-release legacy wire 1/2 cannot be published; Protocol 1.0 uses wire 3.');
  // Unsigned CLI shorthand is normalized to the one v3 model, never legacy wire.
  const explicit=draft.offer_version===3||draft.zadp===3;
  if(explicit && (draft.endpoint!=null || (draft.commerce!=null && Object.keys(draft.commerce).sort().join(',')!=='endpoint')))throw Error('V3 uses endpoint-only commerce');
  const endpoint=draft.commerce?.endpoint??draft.endpoint;
  const commerce=endpoint!=null?{endpoint}:null;
  const price=draft.price??draft.commerce?.price??null;
  const amount = String(typeof price==='string'?price:price?.amount || '');
  let zat=0n;
  if(price!=null) {
  if (!/^\d{1,8}(?:\.\d{1,8})?$/.test(amount)) throw Error('Use a positive ZEC price with at most 8 decimal places.');
  const [whole, fraction = ''] = amount.split('.');
  zat = BigInt(whole) * 100000000n + BigInt(fraction.padEnd(8, '0'));
  if (zat <= 0n || zat > 2100000000000000n) throw Error('Price must be greater than zero and no more than 21 million ZEC.');
  }
  if(price!=null && commerce===null)throw Error('Fixed price requires endpoint');
  if(price?.currency!=null && price.currency!=='ZEC')throw Error('Invalid currency');
  for (const key of ['board_id', 'title', 'capability']) {
    if (typeof draft[key] !== 'string' || !draft[key].trim()) throw Error('Complete the board, name and capability fields.');
    if (/[\u0000-\u001f\u007f]/.test(draft[key])) throw Error('Fields cannot contain control characters.');
  }
  for (const [key, limit] of [['title',128],['capability',64],['board_id',64]]) {
    if (new TextEncoder().encode(draft[key]).length > limit) throw Error(`${key} exceeds its ${limit}-byte UTF-8 limit.`);
  }
  if(commerce!==null){
  let endpoint;
  try { endpoint = new URL(commerce.endpoint); } catch { throw Error('Enter a valid HTTPS service endpoint.'); }
  if (endpoint.protocol !== 'https:' || !endpoint.hostname || endpoint.username || endpoint.password) throw Error('Use an HTTPS endpoint without embedded credentials.');
  }
  // array(14), wire/model identifier, board, ULID(26), public key(32), capability,
  // title, optional description, zat integer, endpoint, two times, nonce(16), signature(64).
  const ttl=draft.ttl_seconds??86400;
  if(!Number.isSafeInteger(ttl)||ttl<=0||ttl>4294967295)throw Error('Invalid publication lifetime.');
  const description = draft.description || null;
  let used = 1 + 1 + textBytes(draft.board_id) + textBytes('0'.repeat(26)) + 34
    + textBytes(draft.capability) + textBytes(draft.title)
    + (description === null ? 1 : textBytes(description))
    + (price==null?1:cborHeadBytes(zat))+(commerce===null?1:textBytes(commerce.endpoint)) + cborHeadBytes(issuedAt) + cborHeadBytes(issuedAt + ttl)
    + 17 + 66;
  if(draft.geo==null)used+=1;
  if(draft.geo!=null) {
    const g=draft.geo;
    if(!g || Object.keys(g).sort().join(',')!=='lat_e5,lon_e5,radius_m' || !Number.isInteger(g.lat_e5) || Math.abs(g.lat_e5)>9000000 || !Number.isInteger(g.lon_e5) || Math.abs(g.lon_e5)>18000000 || !Number.isInteger(g.radius_m) || g.radius_m<1 || g.radius_m>500000)throw Error('Invalid offer area.');
    const integerBytes=n=>cborHeadBytes(n<0?-1-n:n);
    used+=1+integerBytes(g.lat_e5)+integerBytes(g.lon_e5)+integerBytes(g.radius_m);
  }
  return {used, remaining:504-used, limit:504, fits:used<=504, board:draft.board_id};
}
if (typeof module !== 'undefined') module.exports = {memoBudget, textBytes, cborHeadBytes};
