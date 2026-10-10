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
  // Reference seller-client policy only. Protocol and buyer precision stay unchanged.
  function roundedPublicCenter(latitude,longitude) {
    if(typeof latitude!=='number'||typeof longitude!=='number')throw Error('Invalid browser coordinates.');
    fromDegrees(latitude,longitude,1); // Validate raw bounds before privacy rounding.
    // Intl decimal rounding defaults to halfExpand: halfway values round away
    // from zero on both signs, unlike Math.round's negative-half behavior.
    const formatter=new Intl.NumberFormat('en-US',{useGrouping:false,minimumFractionDigits:3,maximumFractionDigits:3});
    const rounded=value=>{
      const decimal=Number(formatter.format(value));
      return (Object.is(decimal,-0)?0:decimal).toFixed(3);
    };
    return {latitude:rounded(latitude),longitude:rounded(longitude)};
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
    constructor(onChange=()=>{}) {this.onChange=onChange;this.location=null;this.mode='anywhere';this.radius_m=25000;this.message='Anywhere · no buyer location requested.';this.busy=false;this.generation=0;}
    setDegrees(lat,lon) {
      const location=fromDegrees(lat,lon,this.radius_m,true);this.generation++;this.busy=false;this.location=location;this.mode='nearby';this.message='Nearby · location stays in this client memory only.';this.onChange();
    }
    setRadius(radius_m) {if(!Number.isInteger(radius_m)||radius_m<0||radius_m>MAX_RADIUS_M)throw Error('Invalid search radius.');this.radius_m=radius_m;if(this.location)this.location={...this.location,radius_m};this.onChange();}
    setMode(mode) {if(!['anywhere','nearby'].includes(mode))throw Error('Invalid discovery mode.');if(this.busy){this.generation++;this.busy=false;}this.mode=mode;this.message=mode==='nearby'?(this.location?'Location active · kept on this device':'Choose a location to find matching offers.'):'Anywhere · no location required.';this.onChange();}
    clear() {this.generation++;this.busy=false;this.location=null;this.mode='anywhere';this.message='Location cleared · Anywhere discovery.';this.onChange();}
    // Only a user click calls this method; never watchPosition or automatic retries.
    request(geolocation) {
      if(this.busy)return;
      this.mode='nearby';
      if(!geolocation){this.message='Location unavailable. Use Anywhere or enter coordinates manually.';this.onChange();return;}
      const generation=++this.generation;this.busy=true;this.message='Waiting for browser location permission…';this.onChange();
      geolocation.getCurrentPosition(position=>{if(generation!==this.generation)return;this.busy=false;try{this.setDegrees(position.coords.latitude,position.coords.longitude);}catch{this.message='Location unavailable. Use Anywhere or enter coordinates manually.';this.onChange();}},()=>{if(generation!==this.generation)return;this.busy=false;this.message='Location not granted. Enter coordinates manually or choose Anywhere.';this.onChange();},{enableHighAccuracy:false,maximumAge:0,timeout:10000});
    }
    filter(offers) {return this.mode==='nearby'?(this.location?nearby(offers,this.location).map(r=>r.offer):[]):offers;}
  }
  return {MAX_RADIUS_M,validateArea,fromDegrees,roundedPublicCenter,distanceMeters,circlesIntersect,geoAreasIntersect,offerMatchesLocation,nearby,BuyerLocation};
})();

'use strict';
const OfferGeo=GeoDiscovery;
function offerPrice(offer) {return offer.zadp===2 ? (offer.commerce?.price ?? null) : (offer.price ?? null);}
function offerCommerce(offer) {
  if(offer.zadp===3)return offer.commerce ? {endpoint:offer.commerce.endpoint,price:offer.price} : null;
  return offer.zadp===2 ? (offer.commerce ?? null) : {endpoint:offer.endpoint,price:offer.price};
}
function offerPurchaseMode(offer) {return offerCommerce(offer)?.endpoint ? 'automated' : offerPrice(offer) ? 'manual' : 'discovery';}
// Validate untrusted index JSON before rendering nested fields. Never execute URLs.
function parseOffersResponse(data) {
  if (!data || !Array.isArray(data.offers) || data.offers.length > 1000) throw Error('Invalid index response');
  for (const offer of data.offers) {
    if (!offer || typeof offer !== 'object') throw Error('Invalid offer');
    for (const key of ['id','seller','title','capability','expires_at']) {
      if (typeof offer[key] !== 'string' || !offer[key] || offer[key].length > 4096) throw Error('Invalid offer field');
    }
    if(offer.zadp!=null && ![1,2,3].includes(offer.zadp))throw Error('Unsupported offer version');
    if(offer.geo != null) {
      if(![2,3].includes(offer.zadp))throw Error('Geo requires Offer v2 or v3');
      OfferGeo.validateArea(offer.geo);
    }
    if (offer.board_id != null && (typeof offer.board_id !== 'string' || !/^[a-z0-9-]{1,64}$/.test(offer.board_id))) throw Error('Invalid board');
    if (offer.board_id != null && offer.chain?.board_id != null && offer.board_id !== offer.chain.board_id) throw Error('Board binding mismatch');
    if (!Number.isFinite(Date.parse(offer.expires_at))) throw Error('Invalid expiry');
    if(offer.zadp===2 && ('endpoint' in offer || 'price' in offer))throw Error('V2 requires commerce block');
    if(offer.zadp===3) {
      if('endpoint' in offer)throw Error('V3 requires endpoint-only commerce');
      if(offer.commerce!=null && (typeof offer.commerce!=='object'||Object.keys(offer.commerce).join(',')!=='endpoint'||offer.price==null))throw Error('V3 endpoint requires price');
    }
    const advertised=offerPrice(offer);
    if(advertised!=null && (!advertised || advertised.currency!=='ZEC' || typeof advertised.amount!=='string' || !/^\d+(\.\d{1,8})?$/.test(advertised.amount) || !(Number(advertised.amount)>0 && Number(advertised.amount)<=21000000)))throw Error('Invalid price');
    if(offer.zadp===3 && advertised!=null){const [whole,fraction='']=advertised.amount.split('.');const zat=BigInt(whole)*100000000n+BigInt(fraction.padEnd(8,'0'));if(zat<=0n||zat>2100000000000000n)throw Error('Invalid price');}
    if(offer.purchase_mode!=null && offer.purchase_mode!==offerPurchaseMode(offer))throw Error('Purchase mode mismatch');
    const commerce=offerCommerce(offer);
    if(commerce!==null) {
      if(!commerce || typeof commerce.endpoint!=='string' || !commerce.endpoint || commerce.endpoint.length>4096)throw Error('Invalid endpoint');
      if(offer.zadp>=2){const url=new URL(commerce.endpoint);if(url.protocol!=='https:'||!url.hostname||url.username||url.password)throw Error('Invalid HTTPS endpoint');}
      const price=commerce.price;
      if(!price || price.currency!=='ZEC' || typeof price.amount!=='string' || !/^\d+(\.\d{1,8})?$/.test(price.amount) || !(Number(price.amount)>0 && Number(price.amount)<=21000000))throw Error('Invalid price');
    }
    if(offer.commerce_verification != null) {
      const proof=offer.commerce_verification;
      if(!proof || !['not_required','discovery_only','verified','unverified','unavailable'].includes(proof.status))throw Error('Invalid commerce verification status');
      if(proof.status==='verified' && (typeof proof.origin!=='string' || proof.origin.length>2048
        || typeof proof.expires_at!=='string' || !Number.isFinite(Date.parse(proof.expires_at))
        || typeof proof.valid_until!=='string' || !Number.isFinite(Date.parse(proof.valid_until))))throw Error('Invalid commerce verification metadata');
    }
    if (offer.chain != null) {
      if (!['mainnet','testnet'].includes(offer.chain.network)) throw Error('Invalid network');
      for (const key of ['block_height','scanned_height','confirmations']) {
        if (!Number.isSafeInteger(offer.chain[key]) || offer.chain[key] < 0) throw Error('Invalid chain metadata');
      }
      if (!/^[a-f0-9]{64}$/.test(offer.chain.transaction_id)) throw Error('Invalid transaction');
    }
  }
  return data.offers;
}
function offersNetwork(offers) {
  const networks = [...new Set(offers.map(o=>o.chain?.network).filter(Boolean))];
  return networks.length === 1 ? networks[0] : (networks.length > 1 ? 'Mixed networks' : 'Network unavailable');
}
function offerBoard(offer) { return offer.board_id ?? offer.chain?.board_id ?? ''; }
function offersForBoard(offers, board) { return board ? offers.filter(o => offerBoard(o) === board) : offers; }
if(typeof module !== 'undefined') module.exports={offerPrice,offerPurchaseMode,offerCommerce,GeoDiscovery,parseOffersResponse,offersNetwork,offerBoard,offersForBoard};
