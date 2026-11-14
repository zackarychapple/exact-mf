use base64::Engine;
use ed25519_dalek::{Signer, SigningKey};
use std::{fs, path::PathBuf};
fn main() {
    let root = PathBuf::from(std::env::args().nth(1).expect("app root"));
    let key_path = PathBuf::from(std::env::var("DEMO_SIGNING_KEY").expect("DEMO_SIGNING_KEY"));
    let seed: [u8; 32] = fs::read(key_path).unwrap().try_into().expect("32-byte key");
    let key = SigningKey::from_bytes(&seed);
    fs::write(
        root.join("resources/public-key.bin"),
        key.verifying_key().as_bytes(),
    )
    .unwrap();
    for seq in [1, 2] {
        let dir = root.join(format!("public-v{seq}"));
        let blobs = dir.join(".exact/blobs");
        fs::create_dir_all(&blobs).unwrap();
        let plan = contract::compile(
            &fs::read_to_string(root.join(format!("remotes/v{seq}.contract"))).unwrap(),
        )
        .unwrap()
        .encode();
        let hash = exact_update::sha256_hex(&plan);
        fs::write(blobs.join(&hash), &plan).unwrap();
        let mut assets = vec![];
        for (name,bytes) in [("remote.hbc",fs::read(root.join(format!(".exact/remote-v{seq}.hbc"))).unwrap()),("release.json",serde_json::to_vec(&serde_json::json!({"name":if seq==1 {"Blue Edition"} else {"Open House"},"version":format!("{seq}.0.0"),"planSha256":hash})).unwrap())] {
 let sha=exact_update::sha256_hex(&bytes);fs::write(blobs.join(&sha),&bytes).unwrap();assets.push(serde_json::json!({"name":name,"sha256":sha,"bytes":bytes.len(),"url":format!("../../blobs/{sha}")}));
 }
        let mut e = serde_json::json!({"exact":1,"app":{"id":"com.zephyr.exact.mf","name":"Exact MF"},"plan":{"sha256":hash,"bytes":plan.len(),"url":format!("../../blobs/{hash}"),"formatVersion":5,"kernelSchema":format!("{:016x}",u64::from_le_bytes(plan[16..24].try_into().unwrap()))},"assets":assets,"stream":{"app":"com.zephyr.exact.mf","channel":"demo","compatibilityId":"visual-remote-v1","seq":seq},"release":format!("visual-remote-{seq}.0.0")});
        let sig = key.sign(&exact_update::canonical_bytes(&e.to_string()).unwrap());
        e["signature"] = serde_json::json!({"keyId":"demo","ed25519":base64::engine::general_purpose::STANDARD.encode(sig.to_bytes())});
        let head = dir.join(".exact/demo/visual-remote-v1");
        fs::create_dir_all(&head).unwrap();
        fs::write(head.join("exact.json"), e.to_string()).unwrap();
        fs::write(dir.join("public-key.bin"), key.verifying_key().as_bytes()).unwrap();
        fs::write(dir.join("index.html"),format!("<!doctype html><title>Exact remote {seq}</title><h1>Exact remote release {seq}</h1><p>Signed native plan distribution.</p>")).unwrap();
    }
}
