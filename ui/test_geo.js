'use strict';
const assert=require('node:assert/strict');
const fs=require('node:fs');
const vm=require('node:vm');
const geo=require('./geo.js');
// Automatic seller GPS rounds decimal ties away from zero; manual e5 is unchanged.
for(const [lat,lon,expectedLat,expectedLon] of [
 [37.774929,-122.419416,'37.775','-122.419'],
 [-37.774929,122.419416,'-37.775','122.419'],
 [37.7745,-122.4195,'37.775','-122.420'],
 [1.0055,-1.0055,'1.006','-1.006'],
 [89.999999,179.999999,'90.000','180.000'],
 [-89.999999,-179.999999,'-90.000','-180.000'],
 [90,-180,'90.000','-180.000'],
 [.0005,-.0005,'0.001','-0.001'],
 [.0001,-.0001,'0.000','0.000'],
]) {
 const rounded=geo.roundedPublicCenter(lat,lon);
 assert.deepEqual(rounded,{latitude:expectedLat,longitude:expectedLon});
 geo.fromDegrees(rounded.latitude,rounded.longitude,50000);
}
for(const values of [[90.000001,0],[0,-180.000001],[NaN,0],[0,Infinity],['37.774929',25]])assert.throws(()=>geo.roundedPublicCenter(...values));
assert.deepEqual(geo.fromDegrees('37.77493','-122.41941',50000),{lat_e5:3777493,lon_e5:-12241941,radius_m:50000});
assert.equal(geo.fromDegrees(geo.roundedPublicCenter(37.774929,-122.419416).latitude,'-122.419',50000).lat_e5,3777500);

const point=(lat,lon,r=0)=>geo.fromDegrees(lat,lon,r,true);
assert.equal(geo.distanceMeters(point(0,0),point(0,0)),0);
for(const [degrees,expected] of [[.009,1000],[.22483,25000],[.89932,100000]])assert(Math.abs(geo.distanceMeters(point(0,0),point(degrees,0))-expected)<2);
const area=geo.fromDegrees(37.77493,-122.41942,50000);
assert(geo.geoAreasIntersect(point(37.785,-122.409),area));
assert(!geo.geoAreasIntersect(point(42.360,-71.059),area));
for(const radius of [5000,25000,50000]){
 const buyer=point(0,0,radius),offer=geo.fromDegrees(0,(radius+1000)/111195.08,2000);
 assert(geo.geoAreasIntersect(buyer,offer));assert(!geo.geoAreasIntersect(point(0,0,0),offer));
}
assert(geo.circlesIntersect(25000,5000,20000));assert(!geo.circlesIntersect(25000.001,5000,20000));
const distance=geo.distanceMeters(point(0,0),point(.01,0));
// Integer-meter radii straddle the computed tangent, with no fake fuzzy allowance.
assert(!geo.geoAreasIntersect(point(0,0,Math.floor(distance)-1),geo.fromDegrees(.01,0,1)));
assert(geo.geoAreasIntersect(point(0,0,Math.ceil(distance)-1),geo.fromDegrees(.01,0,1)));
assert(geo.geoAreasIntersect(point(0,0,100),geo.fromDegrees(0,0,1)));
assert(geo.distanceMeters(point(0,179.99999),point(0,-179.99999))<3);
assert(geo.distanceMeters(point(90,0),point(90,180))<.00001);
assert(geo.distanceMeters(point(-90,-180),point(90,180))>20000000);
for(const values of [[91,0,1],[0,181,1],[0,0,500001],['',0,1],[Infinity,0,1]])assert.throws(()=>geo.fromDegrees(...values));
assert.throws(()=>geo.validateArea({lat_e5:0,lon_e5:0,radius_m:0}));
const near={id:'a',geo:geo.fromDegrees(0,0,5000)},far={id:'b',geo:geo.fromDegrees(.01,0,5000)},global={id:'global'};
assert.deepEqual(geo.nearby([far,global,near],point(0,0)).map(r=>r.offer.id),['a','b']);
assert(!geo.offerMatchesLocation(global,point(0,0)));
let calls=0;
const controller=new geo.BuyerLocation();assert.equal(controller.mode,'anywhere');assert.equal(controller.location,null);
controller.request({getCurrentPosition(_success,error){calls++;error({message:'private data must not reach UI'});}});
assert.equal(calls,1);assert.equal(controller.mode,'nearby');assert(!controller.message.includes('private data'));
controller.setDegrees(0,0);assert.equal(controller.mode,'nearby');controller.setRadius(0);assert.deepEqual(controller.filter([global,near]),[near]);
controller.setMode('anywhere');assert.deepEqual(controller.filter([global,near]),[global,near]);controller.clear();assert.equal(controller.location,null);

const pending=new geo.BuyerLocation();let lateSuccess;
pending.request({getCurrentPosition(success){lateSuccess=success;}});pending.clear();lateSuccess({coords:{latitude:37.775,longitude:-122.419}});assert.equal(pending.location,null);assert.equal(pending.mode,'anywhere');assert.equal(pending.busy,false);

// Run the actual app with DOM/network/storage stubs, then exercise click handlers.
// GPS is a synthetic point; no OS/browser permission or actual coordinate acquisition.
(async()=>{
 const elements=new Map(),requests=[],writes=[],logs=[];
 class Element {
  constructor(){this.value='';this.checked=false;this.hidden=false;this.disabled=false;this.handlers={};this.children=[];this.elements={namedItem:name=>get(name)};}
  addEventListener(event,handler){(this.handlers[event]??=[]).push(handler);}
  append(...items){this.children.push(...items);}replaceChildren(...items){this.children=items;}get firstChild(){return this.children[0];}
  setAttribute(){}removeAttribute(){}showModal(){}close(){}focus(){}reportValidity(){return true;}
 }
 function get(id){if(!elements.has(id))elements.set(id,new Element());return elements.get(id);}
 get('board_id').value='ai-services';get('title').value='Fixture';get('capability').value='cleaning';get('endpoint').value='https://fixture.example';get('price').value='0.01';get('description').value='';get('seller-radius').value='50000';get('buyer-radius').value='50000';get('geo-mode').value='anywhere';
 const offer={id:'fixture',seller:'ed25519:fixture',title:'Bike repair — San Francisco area',capability:'text.uppercase',commerce:null,expires_at:new Date(Date.now()+86400000).toISOString(),zadp:2,geo:area};
 let gpsCalls=0;
 const sandbox={console:{log:(...v)=>logs.push(v),warn:(...v)=>logs.push(v),error:(...v)=>logs.push(v)},TextEncoder,TextDecoder,URL,Blob,AbortController,setTimeout,clearTimeout,setInterval:()=>0,
  localStorage:{getItem:()=>null,setItem:(...v)=>writes.push(v),removeItem:(...v)=>writes.push(v)},
  navigator:{geolocation:{getCurrentPosition(success){gpsCalls++;success({coords:{latitude:37.785,longitude:-122.409}});}}},
  document:{getElementById:get,createElement:()=>new Element(),querySelectorAll:()=>[],addEventListener(){},hidden:false},
  FormData:function(){return ['board_id','title','capability','endpoint','price','description'].map(name=>[name,get(name).value]);},
  fetch:async(url,options)=>{requests.push({url,options});return {ok:true,json:async()=>url==='/api/offers'?{offers:[offer]}:{available:false}}},
  crypto:{getRandomValues:value=>value.fill(7)},atob:s=>Buffer.from(s,'base64').toString('binary')};
 sandbox.window=sandbox;sandbox.confirm=()=>false;sandbox.addEventListener=()=>{};vm.createContext(sandbox);
 for(const file of ['geo.js','offers.js','memo-budget.js','purchase.js','commerce.js','app.js'])vm.runInContext(fs.readFileSync(`${__dirname}/${file}`,'utf8'),sandbox,{filename:file});
 await new Promise(resolve=>setImmediate(resolve));
 assert.equal(get('nearby-controls').hidden,true);assert.equal(get('buyer-radius-label').hidden,true);assert.equal(get('buyer-clear').hidden,true);
 for(const handler of get('mode-nearby').handlers.click)handler();
 assert.equal(get('nearby-controls').hidden,false);assert.equal(get('buyer-radius-label').hidden,true);assert.equal(get('buyer-clear').hidden,true);assert.equal(get('buyer-location').hidden,false);assert.equal(get('offers').children.length,0);
 assert.equal(gpsCalls,0,'No page-load GPS');assert.deepEqual(requests.map(r=>r.url).sort(),['/api/healthz','/api/offers','/api/wallet-status']);
 const networkBefore=requests.length;
 for(const handler of get('buyer-location').handlers.click)handler();
 assert.equal(get('buyer-radius-label').hidden,false);assert.equal(get('buyer-clear').hidden,false);assert.equal(get('buyer-location').hidden,true);
 assert.equal(gpsCalls,1);assert.equal(get('geo-mode').value,'nearby');assert(get('buyer-geo-status').textContent.includes('kept on this device'));
 get('buyer-radius').value='25000';for(const handler of get('buyer-radius').handlers.change)handler();
 get('geo-mode').value='anywhere';for(const handler of get('geo-mode').handlers.change)handler();
 assert.equal(get('nearby-controls').hidden,true);assert.equal(get('buyer-radius-label').hidden,true);assert.equal(get('buyer-clear').hidden,true);assert.equal(get('offers').children.length,1);assert(!get('offers').children[0].children.some(c=>c.textContent==='Buy directly from seller'));
 get('buyer-lat').value='37.77493';get('buyer-lon').value='-122.41942';for(const handler of get('buyer-manual').handlers.click)handler();
 for(const handler of get('buyer-clear').handlers.click)handler();
 assert.equal(get('buyer-lat').value,'');assert.equal(requests.length,networkBefore,'Location actions trigger no request');assert.equal(writes.length,0);assert.equal(logs.length,0);
 sandbox.navigator.geolocation.getCurrentPosition=(_success,error)=>{gpsCalls++;error({message:'private browser location error'});};
 for(const handler of get('buyer-location').handlers.click)handler();assert.equal(get('geo-mode').value,'nearby');assert.equal(requests.length,networkBefore);assert.equal(writes.length,0);assert.equal(logs.length,0);
 for(const handler of get('refresh').handlers.click)await handler();
 assert(requests.every(r=>['/api/healthz','/api/offers','/api/wallet-status'].includes(r.url)));
 assert(requests.every(r=>!JSON.stringify(r).includes('37.77493')&&!JSON.stringify(r).includes('-122.41942')));

 // Saving a no-geo seller draft cannot include the buyer coordinates.
 for(const handler of get('draft-form').handlers.submit)handler({preventDefault(){}});
 assert.equal(writes.length,1);assert.equal(JSON.parse(writes[0][1]).commerce,null);assert(!('endpoint' in JSON.parse(writes[0][1])));assert(!('price' in JSON.parse(writes[0][1])));assert(!writes[0][1].includes('37.77493'));assert(!writes[0][1].includes('-12241942'));assert(!JSON.parse(writes[0][1]).geo);
 // Automatic seller acquisition fills reviewable 3-decimal fields, with no I/O.
 const requestsBeforeSeller=requests.length,writesBeforeSeller=writes.length;
 get('seller-geo-enabled').checked=true;
 for(const handler of get('seller-geo-enabled').handlers.change)handler();
 sandbox.navigator.geolocation.getCurrentPosition=success=>{gpsCalls++;success({coords:{latitude:37.774929,longitude:-122.419416}});};
 for(const handler of get('seller-location').handlers.click)handler();
 assert.equal(get('seller-lat').value,'37.775');assert.equal(get('seller-lon').value,'-122.419');
 assert(get('seller-geo-status').textContent.includes('Rounded public location'));
 assert.equal(requests.length,requestsBeforeSeller);assert.equal(writes.length,writesBeforeSeller);assert.equal(logs.length,0);
 for(const handler of get('draft-form').handlers.submit)handler({preventDefault(){}});
 assert.deepEqual(JSON.parse(writes.at(-1)[1]).geo,{lat_e5:3777500,lon_e5:-12241900,radius_m:50000});
 // Seller edits retain e5 precision; automatic 3-decimal policy does not reapply.
 get('seller-lat').value='37.77493';get('seller-lon').value='-122.41941';
 for(const handler of get('draft-form').handlers.input)handler();
 for(const handler of get('draft-form').handlers.submit)handler({preventDefault(){}});
 assert.equal(get('seller-lat').value,'37.77493');assert.equal(get('seller-lon').value,'-122.41941');
 assert.deepEqual(JSON.parse(writes.at(-1)[1]).geo,{lat_e5:3777493,lon_e5:-12241941,radius_m:50000});
 assert.equal(requests.length,requestsBeforeSeller);
 const html=fs.readFileSync(`${__dirname}/index.html`,'utf8');assert(html.includes('This geographic area becomes public ZADP discovery metadata.') && html.includes('Your browser location is rounded to 3 decimal degrees'));
 assert(!html.includes('src="/geo.js"'),'No new asset route needed by running UI servers');
 for(const id of ['buyer-lat','buyer-lon'])assert(!html.match(new RegExp(`<input[^>]*id="${id}"[^>]*>`))[0].includes('name='));
 // An empty active feed retains public board network/height without location I/O.
 let metadataAvailable=true;
 sandbox.fetch=async(url,options)=>{requests.push({url,options});return {ok:url!='/api/boards'||metadataAvailable,json:async()=>url==='/api/offers'?{offers:[]}:url==='/api/boards'?{boards:[{board_id:'ai-services',network:'mainnet',scanned_height:123}]}:{}};};
 for(const handler of get('refresh').handlers.click)await handler();
 assert.equal(get('network-label').textContent,'mainnet board');assert.equal(get('height').textContent,'Scanned block height: 123');assert.equal(get('offers').children.length,0);
 assert.equal(get('message').textContent,'Choose a location to discover matching offers.');
 for(const handler of get('mode-anywhere').handlers.click)handler();
 assert.equal(get('message').textContent,'No active offers are currently available.');
 const afterMetadata=requests.length;for(const handler of get('mode-nearby').handlers.click)handler();assert.equal(requests.length,afterMetadata);
 metadataAvailable=false;for(const handler of get('refresh').handlers.click)await handler();
 assert.equal(get('network-label').textContent,'Network unavailable');assert.equal(get('connection').textContent,'● Index connected');
 console.log('Local geo and app privacy: matching, no location network/storage/logging, seller rounding/manual precision, and empty-feed board status passed.');
})();
