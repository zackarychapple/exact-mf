import {readdirSync,readFileSync,writeFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
for(const v of [1,2]){const root=`public-v${v}`;const inventory={};for(const f of readdirSync(root,{recursive:true})){if(f==='inventory.json')continue;let b;try{b=readFileSync(`${root}/${f}`)}catch{continue}inventory[f]={bytes:b.length,sha256:createHash('sha256').update(b).digest('hex')};}writeFileSync(`${root}/inventory.json`,JSON.stringify(inventory,null,2));}
