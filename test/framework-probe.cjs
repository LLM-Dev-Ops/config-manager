'use strict';
const assert=require('node:assert/strict');
const {createRequire}=require('node:module');
const {resolve,join}=require('node:path');
async function probe(root){
 const local=createRequire(join(resolve(root),'package.json'));
 const framework=local('@google-cloud/functions-framework');
 const {getTestServer}=local('@google-cloud/functions-framework/testing');
 const name='config-compat-'+require('node:crypto').randomUUID();framework.http(name,local('./index.js').handler);
 const server=getTestServer(name);await new Promise(ok=>server.listen(0,'127.0.0.1',ok));
 let value=0;for(let i=0;i<3000;i++)value={child:value};
 const cases=[
  ['health','GET','/health',undefined,200,b=>assert.equal(b.status,'healthy')],
  ['deep JSON','POST','/v1/config-manager/validate',JSON.stringify({config:{namespace:'app/test',key:'valid',value}}),200,b=>assert.equal(b.data.stats.fields_validated,3003)],
  ['invalid config','POST','/v1/config-manager/validate','{}',400],
  ['unknown route','GET','/unknown',undefined,404],
  ['method boundary','DELETE','/health',undefined,405],
  ['malformed JSON','POST','/v1/config-manager/validate','{"bad":',400],
  ['oversized JSON','POST','/v1/config-manager/validate',JSON.stringify({config:{namespace:'app/test',key:'valid',value:'x'.repeat(1048576)}}),413],
  ['preflight','OPTIONS','/health',undefined,204],
 ];const results=[];
 try{for(const [name,method,path,body,status,check]of cases){const result={name,pass:false};try{const response=await fetch('http://127.0.0.1:'+server.address().port+path,{method,headers:{'content-type':'application/json'},body,signal:AbortSignal.timeout(5000)});result.status=response.status;const raw=await response.text();assert.equal(response.status,status);if(check)check(JSON.parse(raw));result.pass=true;}catch(e){result.error=e.message;}results.push(result);}}
 finally{server.closeAllConnections();await new Promise(ok=>server.close(ok));}
 return results;
}
module.exports={probe};
if(require.main===module)probe(process.argv[2]||'.').then(results=>{console.log(JSON.stringify(results));if(results.some(x=>!x.pass))process.exitCode=1;});
