import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import {spawnSync} from 'node:child_process';
import {resolve,join} from 'node:path';
import {createHash} from 'node:crypto';
import {pathToFileURL} from 'node:url';
const [base,candidate,tools,out]=process.argv.slice(2).map(x=>resolve(x));mkdirSync(out,{recursive:true});
const run=(cwd,args)=>{const p=spawnSync(args[0],args.slice(1),{cwd,encoding:'utf8',timeout:60000,maxBuffer:8*1024*1024});return {command:args,status:p.status,signal:p.signal,error:p.error?.message,stdout:p.stdout,stderr:p.stderr};};
const measurements={};
for(const [label,root]of [['baseline',base],['candidate',candidate]]){
 const tests=run(root,['npm','test']),audit=run(root,['npm','audit','--json']);
 const trials=Array.from({length:3},()=>run(root,['node',join(candidate,'test/framework-probe.cjs'),root]));
 measurements[label]={revision:run(root,['git','rev-parse','HEAD']).stdout.trim(),tests,audit,trials};
 writeFileSync(join(out,label+'.json'),JSON.stringify(measurements[label],null,2));
}
for(const p of ['index.js','Cargo.lock','test/handler.test.js','test/depth-probe.cjs'])if(!readFileSync(join(base,p)).equals(readFileSync(join(candidate,p))))throw Error('Existing implementation/assertions changed: '+p);
const fw=await import(pathToFileURL(join(tools,'node_modules/@metaharness/flywheel/dist/index.js')));
const parent=readFileSync(join(base,'package-lock.json'),'utf8');
const result=await fw.runFlywheelGenerations({rootPolicy:{revision:measurements.baseline.revision},
 proposer:async()=>({value:measurements.candidate.revision,summary:'Exactly pinned forward Functions Framework upgrade and tested scoped UUID override; no downgraded framework or suppressed advisory',inverse:{path:'package-lock.json',parentBytes:parent,hash:createHash('sha256').update(parent).digest('hex')}}),
 evaluator:async(policy,suite)=>{const r=Object.values(measurements).find(x=>x.revision===policy.revision);if(!r)throw Error('Unmeasured revision');
  const wins=suite.id==='holdout'?[r.audit.status===0&&JSON.parse(r.audit.stdout).metadata.vulnerabilities.total===0]:r.trials.flatMap(t=>JSON.parse(t.stdout).map(x=>x.pass));
  // Frozen full acceptance includes operational Ruflo/CI/approval and current
  // Rust gates. Those are incomplete, so this is a fail-closed rejection.
  return {primary:wins.filter(Boolean).length/wins.length,noopRate:0,costPerWin:0,regressed:true,itemWins:wins};},
 holdout:{id:'holdout',items:['full audited dependency graph is vulnerability-free']},anchor:{id:'anchor',items:Array.from({length:24},(_,i)=>i)},maxGenerations:1,signer:fw.makeSigner(),promotionRule:fw.meetsPromotionRule,dataSource:'LIVE_NPM_AUDIT_AND_NATIVE_HTTP_SYNTHETIC_FIXTURES',cacheEvaluations:false});
const replay=fw.verifyReplayBundle(result.replayBundle,{pinnedGateFingerprint:fw.gateFingerprint(fw.meetsPromotionRule)});
writeFileSync(join(out,'flywheel.json'),JSON.stringify({measurements,replay,replayBundle:result.replayBundle,limitation:'Rejected full acceptance, not evidence of completed Ruflo execution or production deployment; zero cost is neutral unmeasured input'},null,2));
console.log(JSON.stringify({measurements:Object.fromEntries(Object.entries(measurements).map(([k,v])=>[k,{revision:v.revision,test:v.tests.status,audit:v.audit.status,findings:JSON.parse(v.audit.stdout).metadata.vulnerabilities.total,trials:v.trials.map(x=>JSON.parse(x.stdout).filter(i=>i.pass).length)}])),promotions:result.replayBundle.verified_improvements,replay}));if(!replay.pass)process.exitCode=1;
