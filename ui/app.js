'use strict';
const $ = id => document.getElementById(id);
let offers = [];
let operatorReady = false;
let previewRunning = false;
const form = $('draft-form');
const draftKey = 'zadp-offer-draft-v1';
function element(tag, text, cls) { const e = document.createElement(tag); e.textContent = text; if (cls) e.className = cls; return e; }
function details(o) {
  $('detail-title').textContent = o.title;
  const dl = $('detail-fields'); dl.replaceChildren();
  const values = {'Capability':o.capability,'Price':`${o.price.amount} ${o.price.currency}`,'Service endpoint':o.endpoint,'Offer':o.id,'Network':o.chain?.network ?? 'Unavailable','Block height':o.chain?.block_height ?? 'Unavailable','Expiry':o.expires_at,'Seller public identity':o.seller,'Confirmations':o.chain?.confirmations ?? 'Unavailable','Transaction':o.chain?.transaction_id ?? 'Unavailable','Signature status':'Accepted by the indexer; the browser does not independently verify signatures','Service availability':o.endpoint === 'https://example.com/zadp' ? 'Test placeholder. This service is not operational.' : 'Purchasing is not yet available.'};
  for (const [key,value] of Object.entries(values)) dl.append(element('dt',key),element('dd',String(value)));
  $('detail').showModal();
}
function render() {
  const query = $('search').value.toLocaleLowerCase('en');
  const filtered = offers.filter(o => `${o.title} ${o.capability}`.toLocaleLowerCase('en').includes(query));
  $('offers').replaceChildren(); $('count').textContent = `${filtered.length} ${filtered.length === 1 ? 'offer' : 'offers'}`;
  for (const o of filtered) {
    const card = element('article','','offer');
    const price = element('p',`${o.price.amount} `,'price'); price.append(element('small',o.price.currency));
    const btn = element('button','View details','secondary'); btn.addEventListener('click',()=>details(o));
    const active = o.chain?.offer_status === 'active';
    card.append(element('span',o.capability,'badge'),element('h2',o.title),element('p',active ? `${o.chain.network} · ${o.chain.confirmations} ${o.chain.confirmations === 1 ? 'confirmation' : 'confirmations'} · Discovered from Zcash` : 'Offer is inactive'),price,btn);
    if(o.endpoint === 'https://example.com/zadp') card.append(element('p','Test offer · placeholder endpoint'));
    $('offers').append(card);
  }
  $('message').textContent = filtered.length ? '' : (offers.length ? 'No offers match your search.' : 'No offers have been indexed yet.');
}
async function refresh() {
  $('refresh').disabled = true;
  try {
    const [health,response] = await Promise.all([fetch('/api/healthz'),fetch('/api/offers')]);
    if(!health.ok || !response.ok) throw Error();
    const data = await response.json();
    offers = parseOffersResponse(data);
    $('network-label').textContent=offersNetwork(offers); render();
    $('connection').textContent = '● Index connected'; $('index-status').textContent = 'Connected. Offers are read from the local Zcash index.';
    const heights = offers.map(o=>o.chain?.scanned_height).filter(Number.isFinite);
    $('height').textContent = heights.length ? `Scanned block height: ${Math.max(...heights).toLocaleString('en')}` : 'Block height unavailable.';
  } catch {
    offers = []; $('network-label').textContent='Network unavailable'; $('offers').replaceChildren(); $('count').textContent = '';
    $('message').textContent = 'Index unavailable or response invalid. Check the local indexer and try again.';
    $('connection').textContent = '○ Index disconnected'; $('index-status').textContent = 'Index unavailable.'; $('height').textContent = '';
  } finally { $('refresh').disabled = false; }
}
for(const button of document.querySelectorAll('[data-tab]')) button.addEventListener('click',()=>{
  for(const b of document.querySelectorAll('[data-tab]')) b.setAttribute('aria-selected',String(b===button));
  for(const section of document.querySelectorAll('.panel')) section.hidden = section.id !== button.dataset.tab;
});
$('search').addEventListener('input',render); $('refresh').addEventListener('click',refresh); $('close').addEventListener('click',()=>$('detail').close());
function draft() { return Object.fromEntries(new FormData(form)); }
function checkBudget() {
  try {
    const budget = memoBudget(draft());
    $('memo-budget').textContent = budget.fits
      ? `${budget.used} / ${budget.limit} payload bytes · ${budget.remaining} bytes available`
      : `${budget.used} / ${budget.limit} payload bytes · ${-budget.remaining} bytes over the limit. Shorten the name, description or endpoint.`;
    $('preview-costs').disabled = !budget.fits || !operatorReady || previewRunning;
    $('save-draft').disabled = !budget.fits; $('export').disabled = !budget.fits;
    return budget.fits;
  } catch (error) {
    $('memo-budget').textContent = error.message;
    $('preview-costs').disabled = true;
    $('save-draft').disabled = true; $('export').disabled = true;
    return false;
  }
}
form.addEventListener('input',()=>{ $('fee-preview').hidden=true; checkBudget(); });
try { const saved=JSON.parse(localStorage.getItem(draftKey)); if(saved) for(const [key,value] of Object.entries(saved)) if(form.elements.namedItem(key)) form.elements.namedItem(key).value=value; } catch {}
checkBudget();
form.addEventListener('submit',event=>{event.preventDefault();if(!checkBudget())return;try { localStorage.setItem(draftKey,JSON.stringify(draft())); $('draft-message').textContent='Draft saved in this browser. It has not been published.'; } catch { $('draft-message').textContent='Could not save the draft. You can download it as a file.'; }});
$('export').addEventListener('click',()=>{if(!form.reportValidity() || !checkBudget())return; const url=URL.createObjectURL(new Blob([JSON.stringify({type:'unsigned-ui-draft',...draft()},null,2)],{type:'application/json'}));const a=element('a','');a.href=url;a.download='zadp-offer-draft.json';a.click();URL.revokeObjectURL(url);});
refresh();

$('test-form').addEventListener('submit', async event => {
  event.preventDefault(); $('test-submit').disabled = true;
  $('test-result').hidden = true; $('test-message').textContent = 'Running locally…';
  try {
    const response = await fetch('/api/test-service/summarize', {
      method: 'POST', headers: {'Content-Type': 'application/json'},
      body: JSON.stringify({text: $('test-input').value})
    });
    if (!response.ok) throw Error();
    const result = await response.json();
    $('test-output').textContent = result.summary;
    $('test-result').hidden = false;
    $('test-message').textContent = 'Completed locally. No payment or blockchain transaction.';
  } catch {
    $('test-message').textContent = 'Test failed. Check the local service and your input, then try again.';
  } finally { $('test-submit').disabled = false; }
});

async function walletStatus() {
  try {
    const response=await fetch('/api/wallet-status');
    if(!response.ok)throw Error();
    const status=await response.json();
    if(!status.available)throw Error();
    operatorReady=true;
    $('wallet-state').textContent=status.sync_locked ? 'Wallet is synchronizing.' : 'Wallet available.';
    const balance=status.seller_total_zat;
    $('wallet-balance').textContent=`Spendable: ${balance.spendable === null ? 'Unavailable' : (balance.spendable/100000000).toFixed(8)+' ZEC'} · Pending: ${balance.pending === null ? 'Unavailable' : (balance.pending/100000000).toFixed(8)+' ZEC'}`;
    $('wallet-sync').textContent=`Wallet scanned height: ${status.wallet_scanned_height ?? 'Unavailable'} · Node tip: ${status.tip ?? 'Unavailable'} (${status.network}). This is wallet readiness, not a public network-tip percentage.`;
    $('receive-address').textContent=status.receive_address;
    $('receive-qr').src='/api/receive-qr'; $('receive-qr').hidden=false;
    $('receive-qr').addEventListener('error',()=>{$('receive-qr').hidden=true;},{once:true});
  } catch {
    operatorReady=false;
    $('wallet-state').textContent='Wallet unavailable or not configured.';
    $('wallet-balance').textContent='';$('wallet-sync').textContent='';$('receive-address').textContent='';$('receive-qr').hidden=true;
  }
  checkBudget();
}
$('preview-costs').addEventListener('click',async()=>{
  if(!form.reportValidity() || !checkBudget())return;
  const previewDraft=draft();
  previewRunning=true;checkBudget();$('fee-preview').hidden=true;
  $('draft-message').textContent='Preparing an unsigned proposal and checking the signed board…';
  try {
    const response=await fetch('/api/publication-preview',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(previewDraft)});
    const result=await response.json();
    if(JSON.stringify(draft())!==JSON.stringify(previewDraft))throw Error();
    if(!response.ok || result.broadcast !== false || result.zcash_transaction_signed !== false)throw Error();
    $('fee-summary').textContent=`${previewDraft.title} · ${previewDraft.endpoint} · Service price: ${previewDraft.price} ZEC · Listing: ${result.listing_value_zat} zat · Network fee: ${result.network_fee_zat} zat · Total: ${result.total_cost_zat} zat (${(result.total_cost_zat/100000000).toFixed(8)} ZEC). Memo payload: ${result.payload_bytes}/504 bytes.`;
    $('fee-board').textContent=`${result.network} · Board ${result.board_id} · Verified fingerprint: ${result.board_fingerprint}`;
    $('fee-preview').hidden=false;$('draft-message').textContent='Preview ready. Nothing has been published.';
  } catch { $('draft-message').textContent='Preview unavailable. Check synchronization, signed board policy and draft fields.'; }
  finally { previewRunning=false;checkBudget(); }
});
walletStatus();

// Only the random order recovery capability is persisted, never input or wallet data.
let orderStorage;try{orderStorage=localStorage;}catch{orderStorage={getItem:()=>{throw Error('Browser storage unavailable');},setItem:()=>{throw Error('Browser storage unavailable');}};}
const purchase=new PurchaseFlow.Flow({storage:orderStorage,fetch:window.fetch.bind(window),
  randomKey:()=>Array.from(crypto.getRandomValues(new Uint8Array(32)),b=>b.toString(16).padStart(2,'0')).join(''),
  lock:navigator.locks ? fn=>navigator.locks.request('zadp-local-order',fn) : null,
  onChange:renderPurchase});
function renderPurchase(state){
  const i=state.invoice,r=state.result;
  $('create-invoice').disabled=purchase.busy || Boolean(i);
  $('create-invoice').textContent=purchase.key && !i ? 'Retry preparing saved order' : 'Prepare order';
  $('check-payment').disabled=purchase.busy || !i;
  $('recover-order').hidden=!purchase.key || Boolean(i);$('recover-order').disabled=purchase.busy;
  $('new-order').hidden=!['expired','fulfilled'].includes(state.phase);
  $('invoice-preview').hidden=!i;
  $('paid-result-card').hidden=state.phase!=='fulfilled';
  $('paid-result').textContent=state.phase==='fulfilled' ? r.output : '';
  $('order-progress').value={idle:0,creating:1,recovering:1,ready:2,awaiting_payment:2,checking:2,confirming:3,fulfilled:4}[state.phase] ?? 2;
  if(i){
    $('order-mode').textContent=i.simulation ? 'LOCAL SIMULATION · No wallet or mainnet transaction' : 'LOCAL SERVICE · Mainnet payment requires separate human approval';
    $('invoice-summary').textContent=`${i.network} · ${(i.amount_zat/100000000).toFixed(8)} ZEC (${i.amount_zat.toLocaleString('en')} zat) · Expires ${new Date(i.expires_at*1000).toLocaleString('en')}. Network fee is additional and is not quoted here.`;
    $('invoice-address').textContent=i.pay_to;$('invoice-memo').textContent=i.memo;
    $('invoice-expiry').textContent=i.simulation ? 'Simulation only. Do not send funds to this fixture receiver.' : purchase.expired() ? 'Payment window closed. Do not pay this invoice. You may still check a payment mined within its window.' : 'One invoice, one exact payment. If already paid, wait for confirmation; do not pay again.';
    $('invoice-payment-fields').hidden=purchase.paymentSeen || purchase.expired() || ['checking','confirming','fulfilled','expired','unavailable','observer_unavailable','payment_mismatch','payment_already_used'].includes(state.phase);
  }else{$('order-mode').textContent='LOCAL SERVICE · No payment is sent by this interface';}
  const messages={idle:'Enter text above, then prepare one order.',creating:'Preparing your saved order…',recovering:'Recovering your saved order…',ready:'Order saved. No payment has been sent by this interface.',checking:'Checking payment evidence…',awaiting_payment:'No confirmed matching payment yet. If you have already paid, do not resend.',confirming:`Matching payment found: ${r?.confirmations ?? 0} / ${i?.confirmations_required ?? 3} confirmations. Waiting for the result.`,expired:'Invoice expired without an eligible confirmed payment. Do not pay it.',observer_unavailable:'Wallet observer unavailable or synchronizing. Your order is saved; do not pay again.',payment_mismatch:'Payment evidence changed. Result withheld; do not pay again.',payment_already_used:'Payment is assigned to another invoice. Result withheld; contact the operator.',fulfilled:i?.simulation ? 'Simulation completed · No funds were sent.' : r?.self_payment_test ? 'Completed · Verified local self-payment test.' : 'Completed · Payment verified and result released.'};
  $('payment-status').textContent=state.error || messages[state.phase] || 'Order status unavailable.';
  $('order-transaction').textContent=r?.txid ? `Transaction: ${r.txid}` : '';
}
$('create-invoice').addEventListener('click',()=>purchase.create($('test-input').value));
$('check-payment').addEventListener('click',()=>purchase.check());
$('recover-order').addEventListener('click',async()=>{await purchase.restore();await purchase.check();});
$('new-order').addEventListener('click',()=>{if(purchase.forget()){$('test-input').value='';$('test-input').focus();}});
window.addEventListener('storage',async e=>{if(e.key===PurchaseFlow.storageKey){await purchase.restore();await purchase.check();}});
document.addEventListener('visibilitychange',()=>{if(!document.hidden)purchase.check();});
setInterval(()=>{if(!document.hidden && purchase.state.invoice && !['fulfilled','expired','payment_mismatch','payment_already_used'].includes(purchase.state.phase))purchase.check();},15000);
(async()=>{await purchase.restore();await purchase.check();})();
$('refresh').addEventListener('click',walletStatus);
