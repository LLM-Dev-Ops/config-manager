const {createServer}=require('node:http');
const {resolve}=require('node:path');
async function probe(root){
  const {handler}=require(resolve(root,'index.js'));
  const server=createServer(async(req,res)=>{
    const chunks=[];for await(const chunk of req)chunks.push(chunk);
    req.body=chunks.length?JSON.parse(Buffer.concat(chunks)):undefined;
    req.path=new URL(req.url,'http://localhost').pathname;
    res.status=code=>{res.statusCode=code;return res;};
    res.json=body=>res.end(JSON.stringify(body));
    try{handler(req,res);}catch(error){res.statusCode=500;res.end(JSON.stringify({exception:error.name}));}
  });
  await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
  const origin=`http://127.0.0.1:${server.address().port}`;
  const check=async(name,config,expected)=>{
    const r=await fetch(origin+'/v1/config-manager/validate',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({config})});
    const body=await r.json();return {name,status:r.status,fields:body.data?.stats.fields_validated,passed:r.status===200&&body.success===true&&body.data.stats.fields_validated===expected};
  };
  try{
    const holdout=[];
    for(const [depth,arrays]of [[1800,false],[2500,false],[1200,true]]){
      let value=0;for(let i=0;i<depth;i++)value=arrays?[{child:value}]:{child:value};
      holdout.push(await check(`depth${depth}-arrays${arrays}`,{namespace:'app/depth',key:'deep',value},depth+3));
    }
    const anchors=[];
    for(const [name,value,count]of [['scalar',0,3],['object',{a:1,b:{c:2}},6],['arrays',[1,{a:2},[{b:3}]],5]])anchors.push(await check(name,{namespace:'app/test',key:'valid',value},count));
    return {holdout,anchors};
  }finally{server.closeAllConnections();await new Promise(resolve=>server.close(resolve));}
}
module.exports={probe};
if(require.main===module)probe(process.argv[2]||'.').then(x=>{console.log(JSON.stringify(x));if([...x.holdout,...x.anchors].some(r=>!r.passed))process.exitCode=1;}).catch(e=>{console.error(e);process.exitCode=1;});
