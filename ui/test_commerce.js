'use strict';
const assert=require('node:assert/strict');
const {Flow,endpoint,validate,paymentMode,storageKey}=require('./commerce.js');
const qrcode=require('./vendor/qrcode.js');
const offer={id:'known',capability:'text.uppercase',title:'Uppercase',endpoint:'http://127.0.0.1:8094',expires_at:new Date(Date.now()+3600000).toISOString(),price:{amount:'0.01',currency:'ZEC'},chain:{network:'testnet'},simulation:true};
const orderId='1'.repeat(32),claim='2'.repeat(64);
function response(status='waiting_payment'){
 const memo=`ZADP-COMMERCE/1:${orderId}:${'3'.repeat(64)}`;
 return {order_id:orderId,offer_id:offer.id,status,simulation:true,network:'testnet',amount_zat:1000000,amount_zec:'0.01000000',recipient:'utest1synthetic',memo,payment_request:`zcash:utest1synthetic?amount=0.01&memo=${Buffer.from(memo).toString('base64url')}`,expires_at:Math.floor(Date.now()/1000)+300,claim_token:claim,confirmations:0,confirmations_required:3,payment_check:'not_checked',transitions:['created','waiting_payment']};
}
(async()=>{
 assert.equal(paymentMode(null,null).walletAllowed,false);
 assert.equal(paymentMode(offer,null).label,'LOCAL PAYMENT SIMULATION');
 for(const state of ['waiting_payment','paid','processing','completed']){
   const mode=paymentMode(offer,response(state));assert.equal(mode.walletAllowed,false);assert(mode.notice.includes('never fund'));
 }
 assert.equal(paymentMode({...offer,simulation:false,chain:{network:'mainnet'}},null).label,'REAL MAINNET PAYMENT REQUEST');
 assert.equal(paymentMode({...offer,simulation:false},null).label,'TESTNET PAYMENT REQUEST');
 assert.equal(paymentMode({...offer,simulation:true,chain:{network:'mainnet'}},null).walletAllowed,false);
 assert.equal(endpoint(offer),'http://127.0.0.1:8094');
 const v2={...offer,zadp:2,commerce:{endpoint:'https://seller.example',price:offer.price}};delete v2.endpoint;delete v2.price;
 assert.equal(endpoint(v2),'https://seller.example');assert.equal(validate(response(),v2).order_id,orderId);
 assert.throws(()=>endpoint({...v2,commerce:null}));assert.throws(()=>validate(response(),{...v2,commerce:null}));
 for(const endpointURL of ['http://external.example','https://user:pass@seller.example','https://example.com/zadp','https://seller.example?q=1','https://seller.example/not-root','javascript:alert(1)'])assert.throws(()=>endpoint({...offer,endpoint:endpointURL}));
 assert.throws(()=>endpoint({...offer,simulation:false}));
 assert.throws(()=>endpoint({...offer,expires_at:'2000-01-01'}));
 for(const change of [{amount_zat:1},{amount_zec:'0.02'},{network:'mainnet'},{order_id:'bad'},{offer_id:'other'},{memo:'bad'},{simulation:false},{result:'x'.repeat(12001)},{payment_request:response().payment_request+'&address.1=other'}])assert.throws(()=>validate({...response(),...change},offer));
 let stored=null,calls=[],mode='waiting_payment',fail=false;
 const storage={getItem:()=>stored,setItem:(key,value)=>{assert.equal(key,storageKey);stored=value;},removeItem:()=>{stored=null;}};
 const fetch=async(url,options)=>{calls.push({url,options});if(fail)throw Error('Synthetic outage');let value=response(mode);if(mode==='completed'){value.payment_check='verified';value.result='HELLO WORLD';}return {ok:true,text:async()=>JSON.stringify(value)};};
 const flow=new Flow({storage,fetch,randomKey:()=> '4'.repeat(64)});await flow.create(offer,'hello world');assert.equal(flow.order.order_id,orderId);
 assert(!stored.includes('hello world'));assert(stored.includes(claim));
 assert.equal(calls[0].url,'http://127.0.0.1:8094/orders');assert.equal(calls[0].options.redirect,'error');assert.equal(calls[0].options.credentials,'omit');
 for(const state of ['paid','processing','completed']){mode=state;await flow.check();assert.equal(flow.order.status,state);}
 assert.equal(flow.order.result,'HELLO WORLD');
 const restored=new Flow({storage,fetch,randomKey:()=>{throw Error('No fresh order key on reload');}});await restored.restore();assert.equal(restored.order.result,'HELLO WORLD');assert.equal(calls.at(-1).options.headers.Authorization,`Bearer ${claim}`);
 fail=true;await restored.check();assert.equal(restored.order,null);assert(restored.error);
 assert(calls.every(call=>!call.url.includes('/api/') && !call.url.includes('8080')));
 restored.forget();assert.equal(stored,null);
 // Lost creation response retains the same private idempotency key and does not store input.
 const retry=new Flow({storage,fetch,randomKey:()=> '5'.repeat(64)});await retry.create(offer,'hello world');const key=retry.saved.key;fail=false;await retry.create(offer,'hello world');assert.equal(retry.saved.key,key);
 const code=qrcode(0,'M');code.addData(response().payment_request);code.make();assert(code.createDataURL().startsWith('data:image/gif;base64,'));
 console.log('Direct commerce client: endpoint safety, request binding, direct calls, recovery, outage and QR passed.');
})();
