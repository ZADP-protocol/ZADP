'use strict';
// Direct browser -> seller protocol. Never call a discovery or UI order proxy.
const DirectCommerce = (() => {
  const storageKey='zadp-direct-commerce-v1';
  function endpoint(offer) {
    if(offer.capability!=='text.uppercase' || Date.parse(offer.expires_at)<=Date.now()) throw Error('This offer is not available for the reference commerce flow.');
    const url=new URL(offer.endpoint);
    const demo=offer.simulation===true && offer.chain?.network==='testnet';
    if(url.username || url.password || url.search || url.hash || !['','/'].includes(url.pathname)
       || !(url.protocol==='https:' || (demo && url.protocol==='http:' && url.hostname==='127.0.0.1' && url.port==='8094'))
       || url.hostname==='example.com') throw Error('An HTTPS seller commerce endpoint is required.');
    return url.origin;
  }
  function zat(amount) {
    if(typeof amount!=='string' || !/^\d+(\.\d{1,8})?$/.test(amount)) throw Error('Invalid amount.');
    const [whole,fraction='']=amount.split('.');return BigInt(whole)*100000000n+BigInt(fraction.padEnd(8,'0'));
  }
  function validate(data,offer,id) {
    if(!data || data.offer_id!==offer.id || !/^[a-f0-9]{32}$/.test(data.order_id) || (id && data.order_id!==id)
      || !['waiting_payment','paid','processing','completed','expired','failed','cancelled'].includes(data.status)
      || !['mainnet','testnet'].includes(data.network) || (offer.chain?.network && data.network!==offer.chain.network)
      || data.simulation!==(offer.simulation===true) || !Number.isSafeInteger(data.amount_zat)
      || BigInt(data.amount_zat)!==zat(offer.price.amount) || zat(data.amount_zec)!==zat(offer.price.amount)
      || !Number.isSafeInteger(data.expires_at) || data.expires_at*1000>Date.parse(offer.expires_at)+1000
      || typeof data.recipient!=='string' || data.recipient.length>400 || !data.recipient.startsWith(data.network==='mainnet'?'u1':'utest1')
      || typeof data.memo!=='string' || !new RegExp(`^ZADP-COMMERCE/1:${data.order_id}:[a-f0-9]{64}$`).test(data.memo)
      || typeof data.payment_request!=='string' || data.payment_request.length>2048
      || !Number.isSafeInteger(data.confirmations) || data.confirmations<0 || !Number.isSafeInteger(data.confirmations_required) || data.confirmations_required<1 || data.confirmations_required>100
      || typeof data.payment_check!=='string' || data.payment_check.length>64 || !Array.isArray(data.transitions) || data.transitions.length>8 || data.transitions.some(s=>!['created','waiting_payment','paid','processing','completed','expired','failed','cancelled'].includes(s))
      || (data.claim_token!=null && !/^[a-f0-9]{64}$/.test(data.claim_token))
      || (data.result!=null && (typeof data.result!=='string' || new TextEncoder().encode(data.result).length>12000))) throw Error('Seller response does not match this offer/order.');
    const uri=new URL(data.payment_request);
    if(uri.protocol!=='zcash:' || uri.pathname!==data.recipient || uri.hash || uri.searchParams.getAll('amount').length!==1 || uri.searchParams.getAll('memo').length!==1
       || zat(uri.searchParams.get('amount'))!==BigInt(data.amount_zat)) throw Error('Invalid payment request.');
    // No extra recipients, amounts or required parameters can slip into a ZIP-321 QR.
    for(const name of uri.searchParams.keys()) if(!['amount','memo','message','label'].includes(name)) throw Error('Unsupported payment parameter.');
    const encoded=uri.searchParams.get('memo');
    if(!/^[A-Za-z0-9_-]+$/.test(encoded)) throw Error('Invalid memo encoding.');
    const memo=atob(encoded.replace(/-/g,'+').replace(/_/g,'/')+'='.repeat((4-encoded.length%4)%4)).replace(/\0+$/,'');
    if(memo!==data.memo) throw Error('Payment memo mismatch.');
    return data;
  }
  function paymentMode(offer,order) {
    const simulation=order?.simulation===true || offer?.simulation===true;
    const network=order?.network || offer?.chain?.network;
    if(simulation)return {label:'LOCAL PAYMENT SIMULATION',notice:'Mock payment evidence only. No funds are sent or received. The QR is a demonstration: never fund this synthetic receiver.',walletAllowed:false};
    if(network==='mainnet')return {label:'REAL MAINNET PAYMENT REQUEST',notice:'Real ZEC would be spent by your separate buyer wallet. Review the complete request and wallet fee preview; explicit human approval is required. This interface does not send funds.',walletAllowed:true};
    if(network==='testnet')return {label:'TESTNET PAYMENT REQUEST',notice:'Testnet only: never use mainnet funds. This is not a mainnet purchase. This interface does not send funds.',walletAllowed:true};
    return {label:'NO PAYMENT MODE SELECTED',notice:'Select an offer to see its network and payment mode. No payment has been prepared or sent.',walletAllowed:false};
  }
  class Flow {
    constructor({storage,fetch,randomKey,onChange=()=>{}}) {this.storage=storage;this.fetch=fetch;this.randomKey=randomKey;this.onChange=onChange;this.saved=null;this.order=null;this.busy=false;this.error='';}
    emit(){this.onChange(this);}
    async request(path,options={}) {
      const controller=new AbortController();const timeout=setTimeout(()=>controller.abort(),25000);
      try {
        const response=await this.fetch(this.saved.endpoint+path,{...options,signal:controller.signal,redirect:'error',credentials:'omit',cache:'no-store',referrerPolicy:'no-referrer'});
        if(!response.ok)throw Error('Seller request failed. Your order is saved; do not pay again.');
        let text;
        if(response.body?.getReader){
          const reader=response.body.getReader();const chunks=[];let size=0;
          try{while(true){const {done,value}=await reader.read();if(done)break;size+=value.length;if(size>200000){await reader.cancel();throw Error('Seller response too large.');}chunks.push(value);}}finally{reader.releaseLock();}
          const bytes=new Uint8Array(size);let offset=0;for(const chunk of chunks){bytes.set(chunk,offset);offset+=chunk.length;}text=new TextDecoder('utf-8',{fatal:true}).decode(bytes);
        }else{text=await response.text();}
        if(text.length>200000)throw Error('Seller response too large.');return JSON.parse(text);
      } finally {clearTimeout(timeout);}
    }
    async create(offer,text) {
      if(this.busy)return;
      this.busy=true;this.error='';this.emit();
      try {
        const origin=endpoint(offer);
        if(typeof text!=='string'||!text.trim()||new TextEncoder().encode(text).length>4000)throw Error('Enter 1–4,000 UTF-8 bytes of text.');
        if(this.saved) {
          if(this.saved.offer.id!==offer.id || this.saved.endpoint!==origin)throw Error('Recover or forget the saved order before buying another offer.');
          if(this.saved.order_id){await this.checkInternal();return;}
        } else {
          this.saved={endpoint:origin,offer:JSON.parse(JSON.stringify(offer)),key:this.randomKey()};
          this.storage.setItem(storageKey,JSON.stringify(this.saved));
        }
        const data=validate(await this.request('/orders',{method:'POST',headers:{'Content-Type':'application/json','Idempotency-Key':this.saved.key},body:JSON.stringify({offer_id:offer.id,request:{text}})}),this.saved.offer);
        if(!data.claim_token)throw Error('Missing order claim token.');
        this.saved.order_id=data.order_id;this.saved.claim_token=data.claim_token;
        this.storage.setItem(storageKey,JSON.stringify(this.saved));this.order=data;
      } catch(e){this.error=e.message;}finally{this.busy=false;this.emit();}
    }
    async checkInternal() {
      if(!this.saved?.order_id)return;
      this.order=validate(await this.request(`/orders/${this.saved.order_id}`,{headers:{Authorization:`Bearer ${this.saved.claim_token}`}}),this.saved.offer,this.saved.order_id);
    }
    async check(){if(this.busy||!this.saved?.order_id)return;this.busy=true;this.error='';this.emit();try{await this.checkInternal();}catch(e){this.order=null;this.error=e.message;}finally{this.busy=false;this.emit();}}
    async restore() {
      try {
        const value=JSON.parse(this.storage.getItem(storageKey)||'null');
        if(value) {
          // Expired discovery offer does not invalidate retrieval of an existing paid order.
          const copy={...value.offer,expires_at:new Date(Date.now()+60000).toISOString()};
          if(endpoint(copy)!==value.endpoint || !/^[a-f0-9]{64}$/.test(value.key) || (value.order_id && (!/^[a-f0-9]{32}$/.test(value.order_id)||!/^[a-f0-9]{64}$/.test(value.claim_token))))throw Error();
          this.saved=value;
        }
      }catch{this.error='Saved order cannot be read. Do not send another payment.';}
      this.emit();await this.check();
    }
    forget(){if(this.busy)return false;this.storage.removeItem(storageKey);this.saved=null;this.order=null;this.error='';this.emit();return true;}
  }
  return {Flow,endpoint,validate,paymentMode,storageKey};
})();
if(typeof module!=='undefined')module.exports=DirectCommerce;
