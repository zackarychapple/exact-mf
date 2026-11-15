import {configureExact} from './configure-exact.mjs';
const {buildNativeFederation} = await import(configureExact() + '/js/bake/src/native_federation.mjs');
import {mkdirSync,writeFileSync} from 'node:fs';
import {spawnSync} from 'node:child_process';
mkdirSync('.exact',{recursive:true});
mkdirSync('resources',{recursive:true});
await buildNativeFederation({input:new URL('./host.mjs',import.meta.url).pathname,file:'.exact/host.js'});
function compile(input,output){let r=spawnSync(process.env.HERMESC,['-emit-binary','-out',output,input],{stdio:'inherit'});if(r.status!==0)throw Error('HBC compilation failed');}
compile('.exact/host.js','resources/host.hbc');
for(const v of [1,2]){
 writeFileSync(`.exact/remote-v${v}.js`,`globalThis.exact={abi:2,appId:'remote.proof',grants:'',sources:['remote'],answer(source,args){return JSON.stringify({name:${JSON.stringify(v===1?'Blue Edition':'Open House')},version:'${v}.0.0'});}};`);
 compile(`.exact/remote-v${v}.js`,`.exact/remote-v${v}.hbc`);
}
