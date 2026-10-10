/* Mirrors the versioned 13/14-element WireOffer in zadp-protocol/src/memo.rs.
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
  const commerce=Object.hasOwn(draft,'commerce')?draft.commerce:{endpoint:draft.endpoint,price:{amount:draft.price,currency:'ZEC'}};
  const version=draft.offer_version??draft.zadp;
  if(version!=null && ![1,2,3].includes(version))throw Error('Unsupported Offer version');
  const v3=version===3 || (version==null && commerce===null && draft.price!=null);
  const v2=version===2 || (version==null && !v3 && (commerce===null || draft.geo!=null));
  if(version===1 && (commerce===null || draft.geo!=null))throw Error('Invalid v1 draft');
  if(version===2 && draft.price!=null)throw Error('V2 uses nested commerce price');
  if(v3 && (draft.endpoint!=null || (commerce!==null && (!commerce || Object.keys(commerce).sort().join(',')!=='endpoint'))))throw Error('V3 uses endpoint-only commerce');
  const price=v3?draft.price:commerce?.price;
  const amount = String(typeof price==='string'?price:price?.amount || '');
  let zat=0n;
  if(price!=null) {
  if (!/^\d{1,8}(?:\.\d{1,8})?$/.test(amount)) throw Error('Use a positive ZEC price with at most 8 decimal places.');
  const [whole, fraction = ''] = amount.split('.');
  zat = BigInt(whole) * 100000000n + BigInt(fraction.padEnd(8, '0'));
  if (zat <= 0n || zat > 2100000000000000n) throw Error('Price must be greater than zero and no more than 21 million ZEC.');
  }
  if(v3 && commerce!==null && price==null)throw Error('Endpoint requires price');
  if(price?.currency!=null && price.currency!=='ZEC')throw Error('Invalid currency');
  for (const key of ['board_id', 'title', 'capability']) {
    if (typeof draft[key] !== 'string' || !draft[key].trim()) throw Error('Complete the board, name and capability fields.');
    if (/[\u0000-\u001f\u007f]/.test(draft[key])) throw Error('Fields cannot contain control characters.');
  }
  for (const [key, limit] of [['title',128],['capability',64],['board_id',64]]) {
    if (new TextEncoder().encode(draft[key]).length > limit) throw Error(`${key} exceeds its ${limit}-byte UTF-8 limit.`);
  }
  if(commerce!==null){
  if(!v3 && commerce.price.currency!=null && commerce.price.currency!=='ZEC')throw Error('Invalid currency');
  let endpoint;
  try { endpoint = new URL(commerce.endpoint); } catch { throw Error('Enter a valid HTTPS service endpoint.'); }
  if (endpoint.protocol !== 'https:' || !endpoint.hostname || endpoint.username || endpoint.password) throw Error('Use an HTTPS endpoint without embedded credentials.');
  }
  // array(13), protocol version, board, ULID(26), public key(32), capability,
  // title, optional description, zat integer, endpoint, two times, nonce(16), signature(64).
  const ttl=draft.ttl_seconds??86400;
  if(!Number.isSafeInteger(ttl)||ttl<=0||ttl>4294967295)throw Error('Invalid publication lifetime.');
  const description = draft.description || null;
  let used = 1 + 1 + textBytes(draft.board_id) + textBytes('0'.repeat(26)) + 34
    + textBytes(draft.capability) + textBytes(draft.title)
    + (description === null ? 1 : textBytes(description))
    + (v3 ? (price==null?1:cborHeadBytes(zat))+(commerce===null?1:textBytes(commerce.endpoint)) : v2 ? (commerce===null?1:1+textBytes(commerce.endpoint)+cborHeadBytes(zat)) : textBytes(commerce.endpoint)+cborHeadBytes(zat)) + cborHeadBytes(issuedAt) + cborHeadBytes(issuedAt + ttl)
    + 17 + 66;
  if((v2||v3) && draft.geo==null)used+=1;
  if(draft.geo!=null) {
    const g=draft.geo;
    if(!g || Object.keys(g).sort().join(',')!=='lat_e5,lon_e5,radius_m' || !Number.isInteger(g.lat_e5) || Math.abs(g.lat_e5)>9000000 || !Number.isInteger(g.lon_e5) || Math.abs(g.lon_e5)>18000000 || !Number.isInteger(g.radius_m) || g.radius_m<1 || g.radius_m>500000)throw Error('Invalid offer area.');
    const integerBytes=n=>cborHeadBytes(n<0?-1-n:n);
    used+=1+integerBytes(g.lat_e5)+integerBytes(g.lon_e5)+integerBytes(g.radius_m);
  }
  return {used, remaining:504-used, limit:504, fits:used<=504, board:draft.board_id};
}
if (typeof module !== 'undefined') module.exports = {memoBudget, textBytes, cborHeadBytes};
