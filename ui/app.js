'use strict';
const $ = id => document.getElementById(id);
let offers = [];
const form = $('draft-form');
const draftKey = 'zadp-offer-draft-v1';
function element(tag, text, cls) { const e = document.createElement(tag); e.textContent = text; if (cls) e.className = cls; return e; }
function details(o) {
  $('detail-title').textContent = o.title;
  const dl = $('detail-fields'); dl.replaceChildren();
  const values = {'Capability':o.capability,'Price':`${o.price.amount} ${o.price.currency}`,'Service endpoint':o.endpoint,'Offer':o.id,'Seller public identity':o.seller,'Confirmations':o.chain?.confirmations ?? 'Unavailable','Transaction':o.chain?.transaction_id ?? 'Unavailable','Index acceptance':'Signature and board policy accepted','Service availability':o.endpoint === 'https://example.com/zadp' ? 'Test placeholder. This service is not operational.' : 'Purchasing is not yet available.'};
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
    card.append(element('span',o.capability,'badge'),element('h2',o.title),element('p',active ? `${o.chain.confirmations} ${o.chain.confirmations === 1 ? 'confirmation' : 'confirmations'} · Active` : 'Offer is inactive'),price,btn);
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
    const data = await response.json(); if(!Array.isArray(data.offers)) throw Error();
    offers = data.offers; render();
    $('connection').textContent = '● Index connected'; $('index-status').textContent = 'Connected. Offers are read from the local mainnet index.';
    const heights = offers.map(o=>o.chain?.scanned_height).filter(Number.isFinite);
    $('height').textContent = heights.length ? `Scanned block height: ${Math.max(...heights).toLocaleString('en')}` : 'Block height unavailable.';
  } catch {
    offers = []; $('offers').replaceChildren(); $('count').textContent = '';
    $('message').textContent = 'Cannot connect to the index. Check the local indexer and try again.';
    $('connection').textContent = '○ Index disconnected'; $('index-status').textContent = 'Index unavailable.'; $('height').textContent = '';
  } finally { $('refresh').disabled = false; }
}
for(const button of document.querySelectorAll('[data-tab]')) button.addEventListener('click',()=>{
  for(const b of document.querySelectorAll('[data-tab]')) b.setAttribute('aria-selected',String(b===button));
  for(const section of document.querySelectorAll('.panel')) section.hidden = section.id !== button.dataset.tab;
});
$('search').addEventListener('input',render); $('refresh').addEventListener('click',refresh); $('close').addEventListener('click',()=>$('detail').close());
function draft() { return Object.fromEntries(new FormData(form)); }
try { const saved=JSON.parse(localStorage.getItem(draftKey)); if(saved) for(const [key,value] of Object.entries(saved)) if(form.elements.namedItem(key)) form.elements.namedItem(key).value=value; } catch {}
form.addEventListener('submit',event=>{event.preventDefault();try { localStorage.setItem(draftKey,JSON.stringify(draft())); $('draft-message').textContent='Draft saved in this browser. It has not been published.'; } catch { $('draft-message').textContent='Could not save the draft. You can download it as a file.'; }});
$('export').addEventListener('click',()=>{if(!form.reportValidity())return; const url=URL.createObjectURL(new Blob([JSON.stringify({type:'unsigned-ui-draft',...draft()},null,2)],{type:'application/json'}));const a=element('a','');a.href=url;a.download='zadp-offer-draft.json';a.click();URL.revokeObjectURL(url);});
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
