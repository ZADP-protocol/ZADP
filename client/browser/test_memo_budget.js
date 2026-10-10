'use strict';
const assert = require('node:assert/strict');
const {memoBudget,textBytes,cborHeadBytes} = require('./memo-budget.js');
const draft={board_id:'ai-services',title:'Test',capability:'llm',price:'0.01',endpoint:'https://example.com/zadp'};
const now=1791228000;
assert.equal(textBytes('abc'),4);
assert.equal(textBytes('ä'),3);
assert.equal(textBytes('😀'),5);
assert.equal(textBytes('a'.repeat(24)),26);
assert.equal(cborHeadBytes(23),1);assert.equal(cborHeadBytes(24),2);
assert.equal(cborHeadBytes(65536),5);
const result=memoBudget(draft,now);
// Independently counted CBOR: 1+1+12+28+34+4+5+1+5+26+5+5+17+66+1.
assert.equal(result.used,211);
let atLimit={...draft,description:'x'.repeat(291)};
assert.equal(memoBudget(atLimit,now).used,504);
assert.equal(memoBudget(atLimit,now).fits,true);
assert.equal(memoBudget({...atLimit,description:atLimit.description+'x'},now).fits,false);
assert.equal(memoBudget({...draft,title:'ä'},now).used,memoBudget({...draft,title:'aa'},now).used);
assert.equal(memoBudget({...draft,description:'hello'},now).used,result.used+5);
for(const price of ['0','-1','0.000000001','1e-8','21000001'])assert.throws(()=>memoBudget({...draft,price},now));
for(const endpoint of ['http://example.com','https://user:password@example.com','bad'])assert.throws(()=>memoBudget({...draft,endpoint},now));
assert.throws(()=>memoBudget({...draft,title:'😀'.repeat(33)},now));
assert.throws(()=>memoBudget({...draft,capability:'x'.repeat(65)},now));
assert.throws(()=>memoBudget({...draft,board_id:'x'.repeat(65)},now));
console.log('Memo budget: UTF-8, CBOR boundaries, exact 504-byte limit, overflow, price and HTTPS checks passed.');

for(const offer_version of [1,2,4])assert.throws(()=>memoBudget({board_id:'local-services',title:'Bike repair',capability:'local.service',offer_version,commerce:null,price:{amount:'0.02',currency:'ZEC'}},now));
