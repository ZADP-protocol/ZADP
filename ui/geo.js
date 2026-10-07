'use strict';
// Public offer areas + private in-memory buyer point. No I/O, storage or logging.
const GeoDiscovery = (() => {
  const MAX_RADIUS_M=500000, EARTH_RADIUS_M=6371008.8;
  function validateArea(area, buyer=false) {
    if(!area || Object.keys(area).sort().join(',')!=='lat_e5,lon_e5,radius_m'
      || !Number.isInteger(area.lat_e5) || Math.abs(area.lat_e5)>9000000
      || !Number.isInteger(area.lon_e5) || Math.abs(area.lon_e5)>18000000
      || !Number.isInteger(area.radius_m) || area.radius_m<(buyer?0:1) || area.radius_m>MAX_RADIUS_M) throw Error('Enter a complete valid area, with radius up to 500 km.');
    return area;
  }
  function fromDegrees(lat,lon,radius_m,buyer=false) {
    if(typeof lat==='string' && !lat.trim() || typeof lon==='string' && !lon.trim())throw Error('Enter both latitude and longitude.');
    const latitude=Number(lat),longitude=Number(lon);
    if(!Number.isFinite(latitude)||Math.abs(latitude)>90||!Number.isFinite(longitude)||Math.abs(longitude)>180)throw Error('Latitude must be between −90 and 90; longitude between −180 and 180.');
    return validateArea({lat_e5:Math.round(latitude*100000),lon_e5:Math.round(longitude*100000),radius_m},buyer);
  }
  function distanceMeters(a,b) {
    validateArea(a,true);validateArea(b,true);
    const rad=Math.PI/18000000;
    const lat1=a.lat_e5*rad,lat2=b.lat_e5*rad,dlat=lat2-lat1,dlon=(b.lon_e5-a.lon_e5)*rad;
    const h=Math.sin(dlat/2)**2+Math.cos(lat1)*Math.cos(lat2)*Math.sin(dlon/2)**2;
    return 2*EARTH_RADIUS_M*Math.asin(Math.sqrt(Math.max(0,Math.min(1,h))));
  }
  function circlesIntersect(distance_m,buyer_radius_m,offer_radius_m) {
    if(!Number.isFinite(distance_m)||distance_m<0 || !Number.isInteger(buyer_radius_m)||buyer_radius_m<0||buyer_radius_m>MAX_RADIUS_M || !Number.isInteger(offer_radius_m)||offer_radius_m<1||offer_radius_m>MAX_RADIUS_M)throw Error('Invalid discovery circles.');
    return distance_m<=buyer_radius_m+offer_radius_m;
  }
  function geoAreasIntersect(buyer,area) {
    validateArea(buyer,true);validateArea(area);
    return circlesIntersect(distanceMeters(buyer,area),buyer.radius_m,area.radius_m);
  }
  function offerMatchesLocation(offer,buyer) {return offer.geo!=null && geoAreasIntersect(buyer,offer.geo);}
  function nearby(offers,buyer) {
    validateArea(buyer,true);
    return offers.filter(o=>offerMatchesLocation(o,buyer)).map(offer=>({offer,distance_m:distanceMeters(buyer,offer.geo)}))
      .sort((a,b)=>a.distance_m-b.distance_m || a.offer.id.localeCompare(b.offer.id));
  }
  class BuyerLocation {
    constructor(onChange=()=>{}) {this.onChange=onChange;this.location=null;this.mode='anywhere';this.radius_m=50000;this.message='Anywhere · no buyer location requested.';this.busy=false;this.generation=0;}
    setDegrees(lat,lon) {
      const location=fromDegrees(lat,lon,this.radius_m,true);this.generation++;this.busy=false;this.location=location;this.mode='nearby';this.message='Nearby · location stays in this client memory only.';this.onChange();
    }
    setRadius(radius_m) {if(!Number.isInteger(radius_m)||radius_m<0||radius_m>MAX_RADIUS_M)throw Error('Invalid search radius.');this.radius_m=radius_m;if(this.location)this.location={...this.location,radius_m};this.onChange();}
    setMode(mode) {if(!['anywhere','nearby'].includes(mode))throw Error('Invalid discovery mode.');if(mode==='nearby'&&!this.location){this.message='Choose a location or enter coordinates to use Nearby.';this.onChange();return;}this.mode=mode;this.message=mode==='nearby'?'Nearby · location stays in this client memory only.':this.location?'Anywhere · your search location remains only in client memory until cleared or reloaded.':'Anywhere · no buyer location requested.';this.onChange();}
    clear() {this.generation++;this.busy=false;this.location=null;this.mode='anywhere';this.message='Location cleared · Anywhere discovery.';this.onChange();}
    // Only a user click calls this method; never watchPosition or automatic retries.
    request(geolocation) {
      if(this.busy)return;
      if(!geolocation){this.message='Location unavailable. Use Anywhere or enter coordinates manually.';this.onChange();return;}
      const generation=++this.generation;this.busy=true;this.message='Waiting for browser location permission…';this.onChange();
      geolocation.getCurrentPosition(position=>{if(generation!==this.generation)return;this.busy=false;try{this.setDegrees(position.coords.latitude,position.coords.longitude);}catch{this.message='Location unavailable. Use Anywhere or enter coordinates manually.';this.onChange();}},()=>{if(generation!==this.generation)return;this.busy=false;this.mode='anywhere';this.message='Location not granted. Anywhere remains available; manual coordinates are optional.';this.onChange();},{enableHighAccuracy:false,maximumAge:0,timeout:10000});
    }
    filter(offers) {return this.mode==='nearby'&&this.location?nearby(offers,this.location).map(r=>r.offer):offers;}
  }
  return {MAX_RADIUS_M,validateArea,fromDegrees,distanceMeters,circlesIntersect,geoAreasIntersect,offerMatchesLocation,nearby,BuyerLocation};
})();
if(typeof module!=='undefined')module.exports=GeoDiscovery;
