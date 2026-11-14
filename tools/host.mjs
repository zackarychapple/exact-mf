import {ModuleFederation} from '@module-federation/runtime-core';
let native; let retained=0; const events=[];
const host=new ModuleFederation({name:'exact-proof',remotes:[],plugins:[{name:'isolated-prepared',async loadEntry({remoteInfo}){
 const generation=remoteInfo.entry;
 native.call({generation,op:'check'});
 return {async init(){events.push(native.call({generation,op:'init'}));},async get(id){
 native.call({generation,op:'get',id});
 return ()=>({value:native.call({generation,op:'factory'})});
 }};
}}]});
globalThis.exact={abi:2,appId:'test.mf',grants:'',sources:['proof'],async answer(source,args,_store,_storage,bridge){native=bridge;
 const v=args[0]; host.registerRemotes([{name:v,entry:v}]);
 const mod=await host.loadRemote(v+'/value');
 return JSON.stringify({value:mod.value,events,registrations:host.options.remotes.length,modules:host.moduleCache.size});
}};
