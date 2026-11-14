import {mkdirSync,writeFileSync} from 'node:fs';
import {dirname} from 'node:path';
import {randomBytes} from 'node:crypto';
const file=process.env.DEMO_SIGNING_KEY ?? `${process.env.HOME}/.config/exact-mf/demo-signing.key`;
mkdirSync(dirname(file),{recursive:true,mode:0o700});
writeFileSync(file,randomBytes(32),{flag:'wx',mode:0o600});
console.log('Created private demo signing key.');
