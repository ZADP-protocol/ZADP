'use strict';
// Compares the browser estimator with the actual Rust signing/memo encoder.
const {spawnSync} = require('node:child_process');
const assert = require('node:assert/strict');
const {memoBudget} = require('./memo-budget.js');
const base = {board_id:'ai-services',title:'Test',capability:'llm',price:'0.01',endpoint:'https://example.com/zadp',issued_at:1791228000};
const cases = [base,{...base,title:'t',capability:'x',board_id:'x',price:'0.00000001',endpoint:'https://x.example'},...([23,24,255,256,300,400].map(n=>({...base,endpoint:'https://example.com/'+ 'x'.repeat(n)})))];
for (const n of [1,23,24,25,127,128,129,255,256]) cases.push({...base,title:'x'.repeat(n)});
for (const title of ['äö你好','😀'.repeat(60),'😀'.repeat(80)]) cases.push({...base,title});
for (const description of ['', 'a'.repeat(23), 'a'.repeat(24), 'a'.repeat(255), 'a'.repeat(256), 'a'.repeat(292), 'a'.repeat(293), '😀'.repeat(100)]) cases.push({...base,description});
for (const price of ['0.00000001','0.00000023','0.00000024','0.00000255','0.00000256','0.00065535','0.00065536','42.94967295','42.94967296','21000000']) cases.push({...base,price});
for(const board_id of ['x','x'.repeat(24),'x'.repeat(64),'x'.repeat(65)])cases.push({...base,board_id});
for(const issued_at of [2147483647,4294967295])cases.push({...base,issued_at});
for(const geo of [{lat_e5:3777493,lon_e5:-12241942,radius_m:50000},{lat_e5:-9000000,lon_e5:-18000000,radius_m:500000},{lat_e5:0,lon_e5:0,radius_m:1},{lat_e5:9000001,lon_e5:0,radius_m:1},{lat_e5:0,lon_e5:0,radius_m:0},{lat_e5:0,lon_e5:0,radius_m:500001}]){
 cases.push({...base,geo});for(const n of [23,24,255,270,280,300])cases.push({...base,geo,description:'x'.repeat(n)});
}
for(const geo of [undefined,{lat_e5:3777500,lon_e5:-12241900,radius_m:25000}])for(const description of ['', 'x'.repeat(200),'x'.repeat(320),'x'.repeat(400)])cases.push({board_id:'ai-services',title:'Bike repair',capability:'local.service',commerce:null,geo,description,issued_at:1791228000});
for(const ttl_seconds of [1,604800,1209600,2592000,4294967295])cases.push({...base,ttl_seconds});
const result = spawnSync('cargo',['run','--quiet','-p','zadp-protocol','--example','memo_budget_reference'],{cwd:require('node:path').resolve(__dirname,'../..'),input:cases.map(c=>JSON.stringify(c)).join('\n')+'\n',encoding:'utf8',timeout:300000});
if (result.status !== 0) throw Error(result.stderr || result.error || 'Rust oracle failed');
const rust = result.stdout.trim().split('\n').map(line=>JSON.parse(line));
assert.equal(rust.length,cases.length);
cases.forEach((draft,i)=>{
 let js;
 try { js=memoBudget(draft,draft.issued_at); } catch { js={fits:false}; }
 assert.equal(js.fits,rust[i].fits,`fit mismatch at case ${i}`);
 if(js.fits)assert.equal(js.used,rust[i].used,`byte mismatch at case ${i}`);
});
console.log(`${cases.length} browser estimates match the actual Rust encoder (including exact-limit acceptance and overflow rejection).`);
