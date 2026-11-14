import {readdirSync,existsSync,cpSync,writeFileSync} from 'node:fs';
import {join} from 'node:path';
import {execFileSync} from 'node:child_process';
function find(root){for(const e of readdirSync(root,{withFileTypes:true})){if(!e.isDirectory())continue;let p=join(root,e.name);if(e.name.endsWith('.app')&&existsSync(join(p,'Info.plist'))){let id=execFileSync('/usr/libexec/PlistBuddy',['-c','Print :CFBundleIdentifier',join(p,'Info.plist')],{encoding:'utf8'}).trim();if(id.endsWith('.host'))return p;}if(!e.name.endsWith('.app')){let p2=find(p);if(p2)return p2;}}}
const app=find('target');if(!app)throw Error('HOST app not found');
for(const file of ['host.hbc','public-key.bin','origin.txt'])cpSync(join('resources',file),join(app,file));
execFileSync('codesign',['--force','--sign','-',app]);execFileSync('codesign',['--verify','--deep','--strict',app]);writeFileSync('.exact/host-app.txt',process.cwd()+'/'+app+'\n');console.log(process.cwd()+'/'+app);
