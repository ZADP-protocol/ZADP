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
  const commerce=offerCommerce(o);
  const dl = $('detail-fields'); dl.replaceChildren();
  const values = {'Offer area':o.geo ? `${o.geo.radius_m/1000} km radius around published approximate center (${o.geo.lat_e5/100000}, ${o.geo.lon_e5/100000}); not a verified seller position` : 'Global / no geographic scope','Board':offerBoard(o) || 'Unavailable','Capability':o.capability,'Commerce':commerce?`${commerce.price.amount} ${commerce.price.currency}`:'Discovery only','Service endpoint':commerce?.endpoint ?? 'No automated purchase endpoint','Offer':o.id,'Network':o.chain?.network ?? 'Unavailable','Block height':o.chain?.block_height ?? 'Unavailable','Expiry':o.expires_at,'Seller public identity':o.seller,'Confirmations':o.chain?.confirmations ?? 'Unavailable','Transaction':o.chain?.transaction_id ?? 'Unavailable','Signature status':'Accepted by the indexer; the browser does not independently verify signatures','Service availability':!commerce ? 'Discovery only; no automated purchase endpoint.' : commerce.endpoint === 'https://example.com/zadp' ? 'Test placeholder. This service is not operational.' : (o.capability === 'text.uppercase' ? 'Direct seller commerce supported. Review the seller endpoint before creating an order.' : 'This capability does not yet have a reference buyer form.')};
  for (const [key,value] of Object.entries(values)) dl.append(element('dt',key),element('dd',String(value)));
  $('detail').showModal();
}
function render() {
  const query = $('search').value.toLocaleLowerCase('en');
  const scoped = offersForBoard(offers, $('board-filter').value);
  const filtered = buyerGeo.filter(scoped).filter(o => `${o.title} ${o.capability}`.toLocaleLowerCase('en').includes(query));
  $('offers').replaceChildren(); $('count').textContent = `${filtered.length} ${filtered.length === 1 ? 'offer' : 'offers'}`;
  for (const o of filtered) {
    const card = element('article','','offer');
    const commerce=offerCommerce(o);
    const price=element('p',commerce?`${commerce.price.amount} ${commerce.price.currency}`:'Discovery only','price');
    const btn = element('button','View details','secondary'); btn.addEventListener('click',()=>details(o));
    const active = o.chain?.offer_status === 'active';
    card.append(element('span',o.capability,'badge'),element('p',`Board: ${offerBoard(o) || 'Unavailable'}`),element('h2',o.title),element('p',o.simulation===true ? 'Known local fixture · no blockchain transaction' : active ? `${o.chain.network} · ${o.chain.confirmations} ${o.chain.confirmations === 1 ? 'confirmation' : 'confirmations'} · Discovered from Zcash` : 'Offer is inactive'),price,btn);
    if(buyerGeo.mode==='nearby')card.append(element('p',buyerGeo.radius_m===0?'Available in your area':`Matches your ${buyerGeo.radius_m/1000} km search`));
    if(o.geo)card.append(element('p',`Service area: ${o.geo.radius_m/1000} km radius around a published approximate center`));
    if(o.simulation===true) {card.append(element('p','LOCAL SIMULATION · No on-chain discovery or real payment'));}
    try {DirectCommerce.endpoint(o); const buy=element('button','Buy directly from seller','primary');buy.addEventListener('click',()=>selectCommerceOffer(o));card.append(buy);}catch{}
    if(commerce?.endpoint === 'https://example.com/zadp') card.append(element('p','Test offer · placeholder endpoint'));
    $('offers').append(card);
  }
  $('message').textContent = buyerGeo.mode==='nearby'&&!buyerGeo.location ? 'Choose a location to discover matching offers.' : filtered.length ? '' : (offers.length ? 'No offers match your search.' : 'No offers have been indexed yet.');
}
function syncBuyerControls() {
  const nearby=buyerGeo.mode==='nearby',active=nearby&&!!buyerGeo.location;
  $('geo-mode').value=buyerGeo.mode;
  $('mode-anywhere').setAttribute('aria-pressed',String(!nearby));
  $('mode-nearby').setAttribute('aria-pressed',String(nearby));
  $('nearby-controls').hidden=!nearby;
  $('buyer-radius-label').hidden=!active;$('buyer-clear').hidden=!active;
  $('buyer-location').hidden=active;$('buyer-manual-fields').hidden=active;
  $('buyer-geo-status').textContent=active?'Location active · kept on this device':buyerGeo.message;
  $('buyer-location').disabled=buyerGeo.busy;
}
const buyerGeo=new GeoDiscovery.BuyerLocation(()=>{syncBuyerControls();render();});
syncBuyerControls();
$('mode-anywhere').addEventListener('click',()=>buyerGeo.setMode('anywhere'));
$('mode-nearby').addEventListener('click',()=>buyerGeo.setMode('nearby'));
$('menu-toggle').addEventListener('click',()=>{
  const nav=$('main-navigation'),open=nav.classList.toggle('is-open');
  $('menu-toggle').setAttribute('aria-expanded',String(open));
});
function closeMenu(){ $('main-navigation').classList.remove('is-open');$('menu-toggle').setAttribute('aria-expanded','false'); }
document.addEventListener('keydown',event=>{if(event.key==='Escape')closeMenu();});
$('buyer-location').addEventListener('click',()=>buyerGeo.request(navigator.geolocation));
$('geo-mode').addEventListener('change',()=>buyerGeo.setMode($('geo-mode').value));
$('buyer-radius').addEventListener('change',()=>buyerGeo.setRadius(Number($('buyer-radius').value)));
$('buyer-manual').addEventListener('click',()=>{
  try{buyerGeo.setDegrees($('buyer-lat').value,$('buyer-lon').value);}
  catch(error){$('buyer-geo-status').textContent=error.message;}
});
$('buyer-clear').addEventListener('click',()=>{buyerGeo.clear();$('buyer-lat').value='';$('buyer-lon').value='';});
async function refresh() {
  $('refresh').disabled = true;
  try {
    const [health,response] = await Promise.all([fetch('/api/healthz'),fetch('/api/offers')]);
    if(!health.ok || !response.ok) throw Error();
    const data = await response.json();
    offers = parseOffersResponse(data);
    const selected = $('board-filter').value;
    const boards = [...new Set(offers.map(offerBoard).filter(Boolean))].sort();
    $('board-filter').replaceChildren(element('option','All boards'));
    $('board-filter').firstChild.value = '';
    for (const id of boards) { const option = element('option',id); option.value = id; $('board-filter').append(option); }
    $('board-filter').value = boards.includes(selected) ? selected : '';
    $('network-label').textContent=offersNetwork(offers); render();
    $('connection').textContent = '● Index connected'; $('index-status').textContent = 'Connected. Offers are read from the connected Zcash index.';
    const heights = offers.map(o=>o.chain?.scanned_height).filter(Number.isFinite);
    $('height').textContent = heights.length ? `Scanned block height: ${Math.max(...heights).toLocaleString('en')}` : 'Block height unavailable.';
  } catch {
    offers = []; $('network-label').textContent='Network unavailable'; $('offers').replaceChildren(); $('count').textContent = '';
    $('message').textContent = 'Index unavailable or response invalid. Check the indexer connection and try again.';
    $('connection').textContent = '○ Index disconnected'; $('index-status').textContent = 'Index unavailable.'; $('height').textContent = '';
  } finally { $('refresh').disabled = false; }
}
for(const button of document.querySelectorAll('[data-tab]')) button.addEventListener('click',()=>{
  for(const b of document.querySelectorAll('[data-tab]')) b.setAttribute('aria-selected',String(b===button));
  for(const section of document.querySelectorAll('.panel')) section.hidden = section.id !== button.dataset.tab;
  closeMenu();
});
$('board-filter').addEventListener('change',render); $('search').addEventListener('input',render); $('refresh').addEventListener('click',refresh); $('close').addEventListener('click',()=>$('detail').close());
function draft() {
  const result=Object.fromEntries(new FormData(form));
  if($('seller-commerce-enabled').checked) {result.commerce={endpoint:result.endpoint,price:{amount:result.price,currency:'ZEC'}};} else {result.commerce=null;}
  delete result.endpoint;delete result.price;
  if($('seller-geo-enabled').checked)result.geo=GeoDiscovery.fromDegrees($('seller-lat').value,$('seller-lon').value,Number($('seller-radius').value==='custom'?$('seller-custom-radius').value:$('seller-radius').value));
  return result;
}
function sellerAreaChanged() {
  const commerce=$('seller-commerce-enabled').checked;
  $('seller-commerce-fields').hidden=!commerce;
  for(const name of ['endpoint','price']){const field=form.elements.namedItem(name);field.disabled=!commerce;field.required=commerce;}
  $('seller-geo-fields').hidden=!$('seller-geo-enabled').checked;
  $('seller-custom-label').hidden=$('seller-radius').value!=='custom';
  $('fee-preview').hidden=true;checkBudget();
}
$('seller-commerce-enabled').addEventListener('change',sellerAreaChanged);
$('seller-geo-enabled').addEventListener('change',sellerAreaChanged);
$('seller-radius').addEventListener('change',sellerAreaChanged);
$('seller-location').addEventListener('click',()=>{
  if(!$('seller-geo-enabled').checked)return;
  if(!navigator.geolocation){$('seller-geo-status').textContent='Location unavailable; enter an approximate offer-area center manually.';return;}
  $('seller-location').disabled=true;
  navigator.geolocation.getCurrentPosition(position=>{
    $('seller-location').disabled=false;
    try {
      const center=GeoDiscovery.roundedPublicCenter(position.coords.latitude,position.coords.longitude);
      $('seller-lat').value=center.latitude;
      $('seller-lon').value=center.longitude;
      $('seller-geo-status').textContent='Rounded public location · 3 decimal degrees. Review or edit this offer-area center before saving or publishing.';
      checkBudget();
    } catch {
      $('seller-geo-status').textContent='Location unavailable; enter an approximate offer-area center manually.';
    }
  },()=>{$('seller-location').disabled=false;$('seller-geo-status').textContent='Location not granted; enter an approximate offer-area center manually.';},{enableHighAccuracy:false,maximumAge:0,timeout:10000});
});
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
try { const saved=JSON.parse(localStorage.getItem(draftKey)); if(saved) {
  for(const [key,value] of Object.entries(saved)) if(form.elements.namedItem(key)) form.elements.namedItem(key).value=value;
  if(saved.commerce || saved.endpoint){const c=saved.commerce??{endpoint:saved.endpoint,price:{amount:saved.price}};$('seller-commerce-enabled').checked=true;form.elements.endpoint.value=c.endpoint;form.elements.price.value=c.price.amount;}
  if(saved.geo){GeoDiscovery.validateArea(saved.geo);$('seller-geo-enabled').checked=true;$('seller-lat').value=saved.geo.lat_e5/100000;$('seller-lon').value=saved.geo.lon_e5/100000;$('seller-radius').value='custom';$('seller-custom-radius').value=saved.geo.radius_m;}
} } catch {}
sellerAreaChanged();
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
    if(!offers.length && ['mainnet','testnet'].includes(status.network))$('network-label').textContent=status.network;
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
    $('fee-summary').textContent=`${previewDraft.title} · ${previewDraft.commerce?`Service price: ${previewDraft.commerce.price.amount} ZEC`:`Discovery only`} · Listing: ${result.listing_value_zat} zat · Network fee: ${result.network_fee_zat} zat · Total: ${result.total_cost_zat} zat (${(result.total_cost_zat/100000000).toFixed(8)} ZEC). Memo payload: ${result.payload_bytes}/504 bytes.`;
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

let commerceOffer=null;
const commerce=new DirectCommerce.Flow({storage:orderStorage,fetch:window.fetch.bind(window),randomKey:()=>Array.from(crypto.getRandomValues(new Uint8Array(32)),b=>b.toString(16).padStart(2,'0')).join(''),onChange:renderCommerce});
function selectCommerceOffer(offer){
  commerceOffer=offer;document.querySelector('[data-tab="commerce"]').click();renderCommerce(commerce);$('commerce-input').focus();
}
function renderCommerce(flow){
  const offer=flow.saved?.offer || commerceOffer;const order=flow.order;const terms=offer?offerCommerce(offer):null;
  const mode=DirectCommerce.paymentMode(offer,order);
  $('commerce-mode').textContent=mode.label;$('commerce-mode-notice').textContent=mode.notice;
  $('commerce-payment-title').textContent=order?.simulation ? 'Demonstration ZIP-321 request / QR · do not pay' : 'ZIP-321 payment request / QR';
  $('commerce-qr').alt=order?.simulation ? 'Simulation QR only — never send funds' : 'Complete Zcash payment request QR';
  $('commerce-offer').textContent=terms ? `${offer.title} · ${terms.price.amount} ZEC · Seller: ${terms.endpoint}` : 'Choose Buy on a supported text.uppercase offer.';
  $('commerce-create').disabled=flow.busy || !terms || Boolean(flow.saved?.order_id);
  $('commerce-create').textContent=flow.saved && !flow.saved.order_id ? 'Retry saved order with the same text' : 'Prepare seller order';
  $('commerce-check').disabled=flow.busy || !flow.saved?.order_id;
  $('commerce-forget').hidden=!flow.saved;$('commerce-forget').disabled=flow.busy;
  const progress={waiting_payment:'Waiting for payment',paid:'Payment received',processing:'Processing',completed:'Completed',expired:'Expired · do not pay',failed:'Execution failed · contact seller',cancelled:'Cancelled · do not pay'};
  $('commerce-status').textContent=flow.error || (flow.busy ? 'Contacting the seller directly…' : order ? `${order.simulation?'LOCAL SIMULATION · ':''}${progress[order.status]} · ${order.payment_check.replaceAll('_',' ')}` : flow.saved ? 'Order recovery saved. If creation was interrupted, retry with the same text. Do not pay again.' : 'No order prepared.');
  $('commerce-summary').textContent=order ? `Order ${order.order_id} · ${order.amount_zec} ZEC · ${order.network} · Expires ${new Date(order.expires_at*1000).toLocaleString('en')} · ${order.confirmations}/${order.confirmations_required} confirmations` : '';
  $('commerce-history').textContent=order ? order.transitions.join(' → ') : '';
  const payable=order && order.status==='waiting_payment' && order.expires_at*1000>Date.now() && ['not_checked','waiting_payment'].includes(order.payment_check) && !flow.busy && !flow.error;
  $('commerce-payment').hidden=!payable;
  if(payable){
    const code=qrcode(0,'M');code.addData(order.payment_request);code.make();$('commerce-qr').src=code.createDataURL(4,16);
    $('commerce-uri').textContent=order.payment_request;$('commerce-memo').textContent=`Required memo: ${order.memo}`;
    $('commerce-wallet').hidden=!mode.walletAllowed;
    if(order.simulation){$('commerce-wallet').removeAttribute('href');$('commerce-status').textContent+=' · Synthetic receiver: never send funds.';}
    else{$('commerce-wallet').href=order.payment_request;}
  }else{$('commerce-qr').removeAttribute('src');$('commerce-uri').textContent='';$('commerce-memo').textContent='';$('commerce-wallet').removeAttribute('href');}
  const completed=order?.status==='completed' && order.payment_check==='verified' && typeof order.result==='string' && !flow.busy && !flow.error;
  $('commerce-result-title').hidden=!completed;$('commerce-result').textContent=completed ? order.result : '';
}
$('commerce-form').addEventListener('submit',async event=>{event.preventDefault();const offer=commerce.saved?.offer||commerceOffer;if(offer)await commerce.create(offer,$('commerce-input').value);});
$('commerce-check').addEventListener('click',()=>commerce.check());
$('commerce-forget').addEventListener('click',()=>{if(window.confirm('Forget this browser’s recovery capability? This does not cancel the order or refund a payment.'))commerce.forget();});
setInterval(()=>{if(!document.hidden && commerce.saved?.order_id && !['completed','expired','failed','cancelled'].includes(commerce.order?.status))commerce.check();},5000);
document.addEventListener('visibilitychange',()=>{if(!document.hidden)commerce.check();});
(async()=>{await commerce.restore();})();
