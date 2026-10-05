'use strict';
const $ = id => document.getElementById(id);
let offers = [];
const form = $('draft-form');
const draftKey = 'zadp-offer-draft-v1';
function element(tag, text, cls) { const e = document.createElement(tag); e.textContent = text; if (cls) e.className = cls; return e; }
function details(o) {
  $('detail-title').textContent = o.title;
  const dl = $('detail-fields'); dl.replaceChildren();
  const values = {'Ominaisuus':o.capability,'Hinta':`${o.price.amount} ${o.price.currency}`,'Palveluosoite':o.endpoint,'Tarjous':o.id,'Myyjän julkinen tunniste':o.seller,'Vahvistuksia':o.chain?.confirmations ?? 'Ei tietoa','Tapahtuma':o.chain?.transaction_id ?? 'Ei tietoa','Indeksin hyväksyntä':'Allekirjoitus ja ilmoitustaulun käytäntö hyväksytty','Palvelun käyttö':o.endpoint === 'https://example.com/zadp' ? 'Testin paikkamerkki. Palvelu ei ole toiminnassa.' : 'Ostaminen ei ole vielä käytössä.'};
  for (const [key,value] of Object.entries(values)) dl.append(element('dt',key),element('dd',String(value)));
  $('detail').showModal();
}
function render() {
  const query = $('search').value.toLocaleLowerCase('fi');
  const filtered = offers.filter(o => `${o.title} ${o.capability}`.toLocaleLowerCase('fi').includes(query));
  $('offers').replaceChildren(); $('count').textContent = `${filtered.length} tarjousta`;
  for (const o of filtered) {
    const card = element('article','','offer');
    const price = element('p',`${o.price.amount} `,'price'); price.append(element('small',o.price.currency));
    const btn = element('button','Näytä tiedot','secondary'); btn.addEventListener('click',()=>details(o));
    const active = o.chain?.offer_status === 'active';
    card.append(element('span',o.capability,'badge'),element('h2',o.title),element('p',active ? `${o.chain.confirmations} vahvistusta · Aktiivinen` : 'Tarjous ei ole aktiivinen'),price,btn);
    if(o.endpoint === 'https://example.com/zadp') card.append(element('p','Testitarjous · palveluosoite on paikkamerkki'));
    $('offers').append(card);
  }
  $('message').textContent = filtered.length ? '' : (offers.length ? 'Hakua vastaavia tarjouksia ei löytynyt.' : 'Indeksissä ei ole vielä tarjouksia.');
}
async function refresh() {
  $('refresh').disabled = true;
  try {
    const [health,response] = await Promise.all([fetch('/api/healthz'),fetch('/api/offers')]);
    if(!health.ok || !response.ok) throw Error();
    const data = await response.json(); if(!Array.isArray(data.offers)) throw Error();
    offers = data.offers; render();
    $('connection').textContent = '● Indeksi yhteydessä'; $('index-status').textContent = 'Yhteys toimii. Tarjoukset luetaan paikallisesta mainnet-indeksistä.';
    const heights = offers.map(o=>o.chain?.scanned_height).filter(Number.isFinite);
    $('height').textContent = heights.length ? `Luettu lohkokorkeus: ${Math.max(...heights).toLocaleString('fi')}` : 'Lohkokorkeutta ei saatavilla.';
  } catch {
    offers = []; $('offers').replaceChildren(); $('count').textContent = '';
    $('message').textContent = 'Indeksiin ei saada yhteyttä. Tarkista paikallinen indeksipalvelu ja yritä uudelleen.';
    $('connection').textContent = '○ Ei yhteyttä indeksiin'; $('index-status').textContent = 'Indeksi ei ole saatavilla.'; $('height').textContent = '';
  } finally { $('refresh').disabled = false; }
}
for(const button of document.querySelectorAll('[data-tab]')) button.addEventListener('click',()=>{
  for(const b of document.querySelectorAll('[data-tab]')) b.setAttribute('aria-selected',String(b===button));
  for(const section of document.querySelectorAll('.panel')) section.hidden = section.id !== button.dataset.tab;
});
$('search').addEventListener('input',render); $('refresh').addEventListener('click',refresh); $('close').addEventListener('click',()=>$('detail').close());
function draft() { return Object.fromEntries(new FormData(form)); }
try { const saved=JSON.parse(localStorage.getItem(draftKey)); if(saved) for(const [key,value] of Object.entries(saved)) if(form.elements.namedItem(key)) form.elements.namedItem(key).value=value; } catch {}
form.addEventListener('submit',event=>{event.preventDefault();try { localStorage.setItem(draftKey,JSON.stringify(draft())); $('draft-message').textContent='Luonnos tallennettu tähän selaimeen. Sitä ei ole julkaistu.'; } catch { $('draft-message').textContent='Tallennus epäonnistui. Voit ladata luonnoksen tiedostoksi.'; }});
$('export').addEventListener('click',()=>{if(!form.reportValidity())return; const url=URL.createObjectURL(new Blob([JSON.stringify({type:'unsigned-ui-draft',...draft()},null,2)],{type:'application/json'}));const a=element('a','');a.href=url;a.download='zadp-offer-draft.json';a.click();URL.revokeObjectURL(url);});
refresh();
