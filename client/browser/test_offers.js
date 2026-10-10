'use strict';
const assert=require('node:assert/strict');
const {parseOffersResponse,offersNetwork,offersForBoard}=require('./offers.js');
const offer={id:'fixture',seller:'ed25519:fixture',title:'A'.repeat(1000),capability:'text.shorten',endpoint:'https://example.com',expires_at:'2030-01-01T00:00:00Z',price:{amount:'0.01',currency:'ZEC'},chain:{network:'mainnet',block_height:1,scanned_height:3,confirmations:3,transaction_id:'a'.repeat(64)}};
assert.deepEqual(parseOffersResponse({offers:[]}),[]);
assert.equal(offersNetwork([]),'Network unavailable');
assert.equal(parseOffersResponse({offers:[offer]}).length,1);
assert.equal(parseOffersResponse({offers:[offer,offer]}).length,2);
assert.equal(offersNetwork([offer]),'mainnet');
assert.equal(offersNetwork([{...offer,chain:{...offer.chain,network:'testnet'}}]),'testnet');
assert.equal(offersNetwork([offer,{...offer,chain:{...offer.chain,network:'testnet'}}]),'Mixed networks');
for(const data of [null,{}, {offers:[null]}, {offers:[{...offer,price:null}]}, {offers:[{...offer,price:{amount:'NaN',currency:'ZEC'}}]}, {offers:[{...offer,chain:{...offer.chain,confirmations:-1}}]}, {offers:[{...offer,chain:{...offer.chain,network:'wrong'}}]}, {offers:[{...offer,expires_at:'invalid'}]}]) assert.throws(()=>parseOffersResponse(data));
console.log('Offer UI: empty/single/multiple/long-text, invalid nested JSON and network labels passed.');

const a={...offer,board_id:'board-a',chain:{...offer.chain,board_id:'board-a'}};
const b={...offer,board_id:'board-b',chain:{...offer.chain,board_id:'board-b'}};
const two=parseOffersResponse({offers:[a,b]});
assert.equal(two[0].id,two[1].id);
assert.deepEqual(offersForBoard(two,'board-a'),[a]);
assert.deepEqual(offersForBoard(two,'board-b'),[b]);
assert.deepEqual(offersForBoard(two,''),two);
assert.throws(()=>parseOffersResponse({offers:[{...a,board_id:'board-b'}]}));
console.log('Multi-board UI: same offer ids, board filtering and binding mismatch passed.');

const discovery={...offer,zadp:2,geo:{lat_e5:3777500,lon_e5:-12241900,radius_m:25000},commerce:null};delete discovery.endpoint;delete discovery.price;
assert.equal(parseOffersResponse({offers:[discovery]}).length,1);
for(const commerce of [{endpoint:'https://seller.example'}, {price:{amount:'0.01',currency:'ZEC'}}, {endpoint:'http://seller.example',price:{amount:'0.01',currency:'ZEC'}}])assert.throws(()=>parseOffersResponse({offers:[{...discovery,commerce}]}));
assert.equal(parseOffersResponse({offers:[{...discovery,commerce:{endpoint:'https://seller.example',price:{amount:'0.01',currency:'ZEC'}}}]}).length,1);

const verification={status:'verified',origin:'https://seller.example',expires_at:'2030-01-01T00:00:00Z',valid_until:'2029-01-01T00:00:00Z'};
assert.equal(parseOffersResponse({offers:[{...offer,commerce_verification:verification}]}).length,1);
for(const bad of [{status:'unknown'},{status:123},{status:'verified'}, {...verification,valid_until:'invalid'}])assert.throws(()=>parseOffersResponse({offers:[{...offer,commerce_verification:bad}]}));
assert.equal(parseOffersResponse({offers:[{...offer,commerce_verification:{status:'unavailable'}}]}).length,1);
const {offerPrice,offerCommerce,offerPurchaseMode}=require('./offers.js');
for(const mode of ['discovery','manual','automated']){
 const v3={...discovery,zadp:3,price:mode==='discovery'?null:{amount:'0.02',currency:'ZEC'},commerce:mode==='automated'?{endpoint:'https://seller.example'}:null,purchase_mode:mode};
 assert.equal(parseOffersResponse({offers:[v3]}).length,1);assert.equal(offerPurchaseMode(v3),mode);assert.equal(!!offerPrice(v3),mode!=='discovery');assert.equal(!!offerCommerce(v3),mode==='automated');
 if(mode!=='discovery')assert.equal(offerPrice(v3).amount,'0.02');
 assert.throws(()=>parseOffersResponse({offers:[{...v3,purchase_mode:mode==='manual'?'automated':'manual'}]}));
}
const manual={...discovery,zadp:3,price:{amount:'0.02',currency:'ZEC'}};
for(const price of [{amount:'0',currency:'ZEC'},{amount:'-1',currency:'ZEC'},{amount:'0.000000001',currency:'ZEC'},{amount:'0.02',currency:'USD'},{amount:'21000000.00000001',currency:'ZEC'}])assert.throws(()=>parseOffersResponse({offers:[{...manual,price}]}));
assert.throws(()=>parseOffersResponse({offers:[{...discovery,zadp:3,commerce:{endpoint:'https://seller.example'}}]}));
assert.throws(()=>parseOffersResponse({offers:[{...manual,commerce:{endpoint:'https://seller.example',price:manual.price}}]}));
assert.throws(()=>parseOffersResponse({offers:[{...manual,zadp:2}]}));
console.log('v3 discovery/manual/automated semantics and mixed-version rejection passed.');
