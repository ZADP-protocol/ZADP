'use strict';
// CommonJS entry for the single pure matcher shipped in the existing browser asset.
// Keeps already-running fixed-asset UI servers compatible without a runtime restart.
if(typeof module!=='undefined')module.exports=require('./offers.js').GeoDiscovery;
