'use strict';
// Validate untrusted index JSON before rendering nested fields. Never execute URLs.
function parseOffersResponse(data) {
  if (!data || !Array.isArray(data.offers) || data.offers.length > 1000) throw Error('Invalid index response');
  for (const offer of data.offers) {
    if (!offer || typeof offer !== 'object') throw Error('Invalid offer');
    for (const key of ['id','seller','title','capability','endpoint','expires_at']) {
      if (typeof offer[key] !== 'string' || !offer[key] || offer[key].length > 4096) throw Error('Invalid offer field');
    }
    if (offer.board_id != null && (typeof offer.board_id !== 'string' || !/^[a-z0-9-]{1,64}$/.test(offer.board_id))) throw Error('Invalid board');
    if (offer.board_id != null && offer.chain?.board_id != null && offer.board_id !== offer.chain.board_id) throw Error('Board binding mismatch');
    if (!Number.isFinite(Date.parse(offer.expires_at))) throw Error('Invalid expiry');
    if (!offer.price || offer.price.currency !== 'ZEC' || typeof offer.price.amount !== 'string'
        || !/^\d+(\.\d{1,8})?$/.test(offer.price.amount)
        || !(Number(offer.price.amount) > 0 && Number(offer.price.amount) <= 21000000)) throw Error('Invalid price');
    if (offer.chain != null) {
      if (!['mainnet','testnet'].includes(offer.chain.network)) throw Error('Invalid network');
      for (const key of ['block_height','scanned_height','confirmations']) {
        if (!Number.isSafeInteger(offer.chain[key]) || offer.chain[key] < 0) throw Error('Invalid chain metadata');
      }
      if (!/^[a-f0-9]{64}$/.test(offer.chain.transaction_id)) throw Error('Invalid transaction');
    }
  }
  return data.offers;
}
function offersNetwork(offers) {
  const networks = [...new Set(offers.map(o=>o.chain?.network).filter(Boolean))];
  return networks.length === 1 ? networks[0] : (networks.length > 1 ? 'Mixed networks' : 'Network unavailable');
}
function offerBoard(offer) { return offer.board_id ?? offer.chain?.board_id ?? ''; }
function offersForBoard(offers, board) { return board ? offers.filter(o => offerBoard(o) === board) : offers; }
if(typeof module !== 'undefined') module.exports={parseOffersResponse,offersNetwork,offerBoard,offersForBoard};
