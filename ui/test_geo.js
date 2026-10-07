'use strict';
const assert=require('node:assert/strict');
const fs=require('node:fs');
const vm=require('node:vm');
const geo=require('./geo.js');
const point=(lat,lon,r=0)=>geo.fromDegrees(lat,lon,r,true);
assert.equal(geo.distanceMeters(point(0,0),point(0,0)),0);
for(const [degrees,expected] of [[.009,1000],[.22483,25000],[.89932,100000]])assert(Math.abs(geo.distanceMeters(point(0,0),point(degrees,0))-expected)<2);
const area=geo.fromDegrees(37.77493,-122.41942,50000);
assert(geo.geoAreasIntersect(point(37.785,-122.409),area));
assert(!geo.geoAreasIntersect(point(0,0),area));
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
assert.equal(calls,1);assert.equal(controller.mode,'anywhere');assert(!controller.message.includes('private data'));
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
 const offer={id:'fixture',seller:'ed25519:fixture',title:'Cabin cleaning',capability:'cleaning',endpoint:'https://fixture.example',expires_at:new Date(Date.now()+86400000).toISOString(),price:{amount:'0.01',currency:'ZEC'},zadp:2,geo:area};
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
 assert.equal(gpsCalls,0,'No page-load GPS');assert.deepEqual(requests.map(r=>r.url).sort(),['/api/healthz','/api/offers','/api/wallet-status']);
 const networkBefore=requests.length;
 for(const handler of get('buyer-location').handlers.click)handler();
 assert.equal(gpsCalls,1);assert.equal(get('geo-mode').value,'nearby');assert(get('buyer-geo-status').textContent.includes('client memory'));
 get('buyer-radius').value='25000';for(const handler of get('buyer-radius').handlers.change)handler();
 get('geo-mode').value='anywhere';for(const handler of get('geo-mode').handlers.change)handler();
 get('buyer-lat').value='37.77493';get('buyer-lon').value='-122.41942';for(const handler of get('buyer-manual').handlers.click)handler();
 for(const handler of get('buyer-clear').handlers.click)handler();
 assert.equal(get('buyer-lat').value,'');assert.equal(requests.length,networkBefore,'Location actions trigger no request');assert.equal(writes.length,0);assert.equal(logs.length,0);
 sandbox.navigator.geolocation.getCurrentPosition=(_success,error)=>{gpsCalls++;error({message:'private browser location error'});};
 for(const handler of get('buyer-location').handlers.click)handler();assert.equal(get('geo-mode').value,'anywhere');assert.equal(requests.length,networkBefore);assert.equal(writes.length,0);assert.equal(logs.length,0);
 for(const handler of get('refresh').handlers.click)await handler();
 assert(requests.every(r=>['/api/healthz','/api/offers','/api/wallet-status'].includes(r.url)));
 assert(requests.every(r=>!JSON.stringify(r).includes('37.77493')&&!JSON.stringify(r).includes('-122.41942')));

 // Saving a no-geo seller draft cannot include the buyer coordinates.
 for(const handler of get('draft-form').handlers.submit)handler({preventDefault(){}});
 assert.equal(writes.length,1);assert(!writes[0][1].includes('37.77493'));assert(!writes[0][1].includes('-12241942'));assert(!JSON.parse(writes[0][1]).geo);
 const html=fs.readFileSync(`${__dirname}/index.html`,'utf8');assert(html.includes('This geographic area becomes public ZADP discovery metadata.'));
 for(const id of ['buyer-lat','buyer-lon'])assert(!html.match(new RegExp(`<input[^>]*id="${id}"[^>]*>`))[0].includes('name='));
 console.log('Local geo and app privacy: distances, intersections, ordering, denial, no automatic GPS, no location network/storage/logging, seller/buyer separation passed.');
})();
