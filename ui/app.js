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

// Invoice access stays in memory, never localStorage or a URL.
let currentInvoice=null;
$('create-invoice').addEventListener('click',async()=>{
  const text=$('test-input').value;
  if(!text.trim() || text.length>4000){$('payment-status').textContent='Enter 1 to 4,000 characters above.';return;}
  $('create-invoice').disabled=true;currentInvoice=null;
  $('invoice-preview').hidden=true;$('paid-result').textContent='';
  try {
    const response=await fetch('/api/purchase/invoice',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({text})});
    if(!response.ok)throw Error();
    const invoice=await response.json();currentInvoice=invoice;
    $('invoice-summary').textContent=`${invoice.network} · Local unpublished test · ${(invoice.amount_zat/100000000).toFixed(8)} ZEC (${invoice.amount_zat} zat) · ${invoice.confirmations_required} confirmations required · Expires ${new Date(invoice.expires_at*1000).toLocaleString('en')}. Buyer network fee is additional and has not been quoted.`;
    $('invoice-address').textContent=invoice.pay_to;$('invoice-memo').textContent=invoice.memo;
    $('invoice-preview').hidden=false;$('payment-status').textContent='Payment instructions prepared. No funds have been sent. Mainnet payment requires separate human approval.';
  }catch{$('payment-status').textContent='Paid local test unavailable. Check the operator configuration and service.';}
  finally{$('create-invoice').disabled=false;}
});
$('check-payment').addEventListener('click',async()=>{
  if(!currentInvoice)return;
  $('check-payment').disabled=true;
  try{
    const response=await fetch('/api/purchase/fulfill',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({invoice_id:currentInvoice.invoice_id,access_token:currentInvoice.access_token})});
    if(!response.ok)throw Error();
    const result=await response.json();
    const messages={awaiting_payment:'No matching payment with enough confirmations yet.',expired:'Invoice expired. Do not pay it.',observer_unavailable:'Wallet observer unavailable or still synchronizing.',payment_mismatch:'Payment evidence changed. Result withheld.',payment_already_used:'Payment is already assigned to another invoice.',fulfilled:'Payment verified. Result released.'};
    $('payment-status').textContent=messages[result.status] ?? 'Payment status unavailable.';
    $('paid-result').textContent=result.status==='fulfilled' ? result.output : '';
  }catch{$('payment-status').textContent='Payment check unavailable.';$('paid-result').textContent='';}
  finally{$('check-payment').disabled=false;}
});
$('refresh').addEventListener('click',walletStatus);
