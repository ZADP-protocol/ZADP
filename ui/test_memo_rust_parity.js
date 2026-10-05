'use strict';
// Compares the browser estimator with the actual Rust signing/memo encoder.
const {spawnSync} = require('node:child_process');
const assert = require('node:assert/strict');
const {memoBudget} = require('./memo-budget.js');
const base = {board_id:'ai-services',title:'Test',capability:'llm',price:'0.01',endpoint:'https://example.com/zadp',issued_at:1791228000};
const cases = [base];
for (const n of [1,23,24,25,127,128,129,255,256]) cases.push({...base,title:'x'.repeat(n)});
for (const title of ['äö你好','😀'.repeat(60),'😀'.repeat(80)]) cases.push({...base,title});
for (const description of ['', 'a'.repeat(23), 'a'.repeat(24), 'a'.repeat(255), 'a'.repeat(256), 'a'.repeat(292), 'a'.repeat(293), '😀'.repeat(100)]) cases.push({...base,description});
for (const price of ['0.00000001','0.00000023','0.00000024','0.00000255','0.00000256','0.00065535','0.00065536','42.94967295','42.94967296','21000000']) cases.push({...base,price});
for(const board_id of ['x','x'.repeat(24),'x'.repeat(64),'x'.repeat(65)])cases.push({...base,board_id});
for(const issued_at of [2147483647,4294967295])cases.push({...base,issued_at});
const result = spawnSync('cargo',['run','--quiet','-p','zadp-protocol','--example','memo_budget_reference'],{cwd:require('node:path').resolve(__dirname,'..'),input:cases.map(c=>JSON.stringify(c)).join('\n')+'\n',encoding:'utf8',timeout:300000});
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
