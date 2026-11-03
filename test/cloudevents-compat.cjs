'use strict';
const assert=require('node:assert/strict');
const {CloudEvent,HTTP}=require('cloudevents');
const event=new CloudEvent({source:'urn:config:compat',type:'config.test',data:{value:42}});
assert.match(event.id,/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i);
for(const encode of [HTTP.binary,HTTP.structured]){
 const encoded=encode(event);const decoded=HTTP.toEvent(encoded);
 assert.equal(decoded.id,event.id);assert.equal(decoded.type,event.type);assert.equal(decoded.source,event.source);assert.deepEqual(decoded.data,event.data);
}
console.log('CloudEvents UUIDv4, binary and structured roundtrips passed');
