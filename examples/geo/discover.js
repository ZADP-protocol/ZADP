#!/usr/bin/env node
'use strict';
// Read a PUBLIC GET /offers feed file; accept the private buyer circle on stdin.
// No coordinate arguments, network, persistence, analytics or buyer-point output.
const fs=require('node:fs');
const geo=require('../../client/browser/geo.js');
try {
  if(process.argv.length!==3)throw Error('Expected a public feed file.');
  const feed=JSON.parse(fs.readFileSync(process.argv[2],'utf8'));
  const buyer=geo.validateArea(JSON.parse(fs.readFileSync(0,'utf8')),true);
  if(!Array.isArray(feed.offers))throw Error('Expected a public offers feed.');
  console.log(JSON.stringify({offers:geo.nearby(feed.offers,buyer).map(row=>row.offer)},null,2));
} catch {console.error('Local discovery failed: provide a public feed file and complete integer e5 search circle on stdin (radius 0–500000 m).');process.exitCode=1;}
