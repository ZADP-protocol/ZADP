'use strict';
const assert=require('node:assert/strict');
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


console.log('Reference local geo matching, no automatic GPS, denied/late permissions and volatile memory passed.');
