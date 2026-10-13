/* Hosted presentation policy v1; canonical Offer/signing semantics are unchanged. */
'use strict';
const OfferModeration = (() => {
 function assess(text) {
  const signals=[];
  const normalized=text.toLowerCase().trim().split(/\s+/).join(' ');
  if(['stolen credit card numbers for sale','buy stolen passwords','child sexual abuse material for sale'].some(p=>normalized.includes(p)))signals.push('explicit_prohibited_content');
  if(/\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b/i.test(text))signals.push('email_address');
  const phones=text.match(/(?:^|[^\w])\+[1-9][0-9]{0,2}[ .()-]+[0-9][0-9 .()-]{6,}[0-9](?:$|[^\w])/g)||[];
  if(phones.some(s=>{const n=s.replace(/\D/g,'').length;return n>=9&&n<=15;}))signals.push('international_phone');
  if(/\b(?:ssn|social security|passport|national id|government id)\s*(?:number\s*)?[:#]\s*[A-Z0-9][A-Z0-9 -]{5,20}\b/i.test(text))signals.push('labelled_government_id');
  if(signals.length)return {outcome:'rejected',signals};
  if(/\b(?:email|e-mail)\s*[:=]\s*\S+\s*(?:\[at\]|\(at\))\s*\S+|\b(?:phone|call|whatsapp|telephone)\s*[:=]\s*[0-9][0-9 ()-]{6,}[0-9]/i.test(text))return {outcome:'pending_review',signals:['ambiguous_contact']};
  return {outcome:'approved',signals};
 }
 function draft(d){return assess([d.title||'',d.description||'',d.capability||'',d.commerce?.endpoint||''].join('\n'));}
 function message(d){const r=draft(d);return r.outcome==='rejected'?'Remove prohibited content, phone numbers, email addresses and government ID numbers from the service name or description. Use your HTTPS service endpoint for contact.':r.outcome==='pending_review'?'This listing contains ambiguous contact information and will require hosted review. Remove it to publish a normal listing.':'';}
 return {assess,draft,message};
})();
if(typeof module!=='undefined')module.exports=OfferModeration;
