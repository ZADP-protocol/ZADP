'use strict';
(function(root){
  const storageKey='zadp-local-order-v1';
  const statuses=new Set(['awaiting_payment','confirming','expired','observer_unavailable','payment_mismatch','payment_already_used','fulfilled']);
  function invoice(value){
    if(!value || typeof value!=='object' || !/^[a-f0-9]{32}$/.test(value.invoice_id) || !/^[A-Za-z0-9_-]{32,128}$/.test(value.access_token)
       || !['mainnet','testnet'].includes(value.network) || value.currency!=='ZEC' || value.local_unpublished_test!==true
       || typeof value.pay_to!=='string' || value.pay_to.length>500 || !value.pay_to.startsWith(value.network==='mainnet'?'u1':'utest1')
       || value.memo!==`ZADP-PAY/1:${value.invoice_id}` || !Number.isSafeInteger(value.amount_zat) || value.amount_zat<=0 || value.amount_zat>2100000000000000
       || !Number.isSafeInteger(value.expires_at) || !Number.isSafeInteger(value.confirmations_required) || value.confirmations_required<1 || value.confirmations_required>100
       || typeof value.offer_id!=='string' || value.offer_id.length>128 || (value.simulation!==undefined && typeof value.simulation!=='boolean')) throw Error('Invalid invoice response');
    return value;
  }
  function fulfillment(value,required){
    if(!value || !statuses.has(value.status))throw Error('Invalid payment response');
    if(['confirming','fulfilled'].includes(value.status) && (!Number.isSafeInteger(value.confirmations) || value.confirmations<0 || value.confirmations_required!==required
       || !/^[a-f0-9]{64}$/.test(value.txid)))throw Error('Invalid confirmation evidence');
    if(value.status==='fulfilled' && (value.confirmations<required || typeof value.output!=='string' || value.output.length>4000))throw Error('Invalid result');
    return value;
  }
  class Flow {
    constructor({storage,fetch,randomKey,lock,onChange=()=>{},now=()=>Date.now()}){
      Object.assign(this,{storage,fetch,randomKey,lock,onChange,now});this.key=null;this.busy=false;this.paymentSeen=false;
      this.state={phase:'idle',invoice:null,result:null,error:''};
    }
    emit(patch){this.state={...this.state,...patch};this.onChange(this.state);}
    storedKey(){const raw=this.storage.getItem(storageKey);if(!raw){this.paymentSeen=false;return null;}const saved=JSON.parse(raw);if(saved.version!==1 || !/^[a-f0-9]{64}$/.test(saved.request_key))throw Error('Saved order is invalid');this.paymentSeen=saved.payment_seen===true;return saved.request_key;}
    async post(operation,body){const response=await this.fetch(`/api/purchase/${operation}`,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body),cache:'no-store',signal:AbortSignal.timeout(35000)});if(!response.ok)throw Error('Service unavailable');return response.json();}
    finish(){this.busy=false;this.onChange(this.state);if(this.restoreQueued){this.restoreQueued=false;this.restore().then(()=>this.check());}}
    async restore(){
      if(this.busy){this.restoreQueued=true;return;}this.busy=true;
      try{this.key=this.storedKey();if(!this.key){this.emit({phase:'idle',invoice:null,result:null,error:''});return;}
        this.emit({phase:'recovering',invoice:null,result:null,error:''});const key=this.key;
        const value=invoice(await this.post('recover',{request_key:key}));
        if(this.storedKey()!==key)return;
        this.emit({phase:'ready',invoice:value,error:''});
      }catch{this.emit({phase:'recovery_error',result:null,error:'Could not recover the saved order. Retry recovery, or re-enter the original text and retry preparation using the same order key. Do not pay again.'});}
      finally{this.finish();}
    }
    async create(text){
      if(this.busy || this.state.invoice)return;
      if(typeof text!=='string' || !text.trim() || text.length>4000){this.emit({error:'Enter 1 to 4,000 characters.'});return;}
      this.busy=true;this.emit({phase:'creating',result:null,error:''});
      try{
        if(!this.lock)throw Error('Cross-tab order lock unavailable');
        await this.lock(async()=>{
          this.key=this.storedKey();
          if(!this.key){this.key=this.randomKey();if(!/^[a-f0-9]{64}$/.test(this.key))throw Error('Invalid random key');this.storage.setItem(storageKey,JSON.stringify({version:1,request_key:this.key}));}
          const key=this.key;
          const value=invoice(await this.post('invoice',{text,request_key:key}));
          if(this.storedKey()!==key)return;
          this.emit({phase:'ready',invoice:value,error:''});
        });
      }catch{this.emit({phase:'recovery_error',result:null,error:'Preparation was interrupted or is unavailable. Retry with the same text or recover the saved order. No payment was sent.'});}
      finally{this.finish();}
    }
    async check(){
      if(this.busy || !this.state.invoice)return;
      const current=this.state.invoice,key=this.key;this.busy=true;
      this.emit({phase:'checking',result:null,error:''});
      try{const value=fulfillment(await this.post('fulfill',{invoice_id:current.invoice_id,access_token:current.access_token}),current.confirmations_required);
        if(this.storedKey()!==key)return;
        if(['confirming','fulfilled'].includes(value.status)){this.paymentSeen=true;this.storage.setItem(storageKey,JSON.stringify({version:1,request_key:key,payment_seen:true}));}
        this.emit({phase:value.status, result:value,error:''});
      }catch{this.emit({phase:'unavailable',result:null,error:'Payment check unavailable. Your order is saved. Retry the check; do not send another payment.'});}
      finally{this.finish();}
    }
    forget(){
      if(this.busy || !['expired','fulfilled'].includes(this.state.phase))return false;
      this.storage.removeItem(storageKey);this.key=null;this.paymentSeen=false;this.emit({phase:'idle',invoice:null,result:null,error:''});return true;
    }
    expired(){return this.state.invoice && this.state.invoice.expires_at*1000<=this.now();}
  }
  const api={Flow,invoice,fulfillment,storageKey};
  if(typeof module!=='undefined' && module.exports)module.exports=api;else root.PurchaseFlow=api;
})(typeof window!=='undefined'?window:globalThis);
