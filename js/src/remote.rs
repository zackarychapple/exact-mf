//! Isolated bytecode admitted from a host-selected update generation.

use crate::Module;
use exact_runner::{DataSource, Value};
use exact_update::{AssetSet, Generation};

/// A host-owned remote. Preparation pins host-admitted bytes without executing JavaScript.
/// The host chooses identity and grants independently of the remote's exports.
/// No engine objects or shared-scope objects cross this boundary.
pub struct PreparedRemote {
    generation: Generation,
    module: Option<Module>,
    active: bool,
}

impl PreparedRemote {
    /// Resolve a bytecode asset from a verified roster. Missing or changed bytes fail here.
    /// This operation executes no JavaScript. Development trust remains development trust.
    pub fn new(assets: &AssetSet, name: &str, app_id: &str, grants: &str) -> Result<Self, String> {
        let bytes = assets.resolve(name)?.ok_or("remote asset is absent")?;
        Ok(Self {
            generation: assets.generation().clone(),
            module: Some(Module::new(bytes.to_vec(), app_id, grants)),
            active: false,
        })
    }

    /// Bind the host-selected source shapes before activation.
    pub fn bind(&mut self, plan: &exact_plan::Plan) -> Result<(), String> {
        if self.active {
            return Err("remote is already active".into());
        }
        self.module.as_mut().ok_or("remote is disposed")?.bind(plan);
        Ok(())
    }

    /// The host-admitted generation that supplied these bytes, under the store's trust policy.
    pub fn generation(&self) -> &Generation {
        &self.generation
    }

    /// Execute the remote in its own realm. The host must call this after first pixel.
    /// A failed activation disposes the remote. It cannot be retried.
    pub fn activate(&mut self) -> Result<(), String> {
        if self.active {
            return Ok(());
        }
        let module = self.module.as_mut().ok_or("remote is disposed")?;
        if let Err(error) = module.load() {
            self.dispose();
            return Err(error);
        }
        self.active = true;
        Ok(())
    }

    /// Call a data source through the executor's value boundary and turn budget.
    /// An inactive or disposed handle cannot execute code.
    pub fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, String> {
        if !self.active {
            return Err("remote is inactive or disposed".into());
        }
        self.module
            .as_mut()
            .ok_or("remote is disposed")?
            .query(source, args)
            .map_err(|error| error.to_string())
    }

    /// Revoke all calls through this handle and release its realm.
    pub fn dispose(&mut self) {
        self.active = false;
        if let Some(mut module) = self.module.take() {
            module.unload();
        }
    }
}

#[cfg(all(test, exact_js_engine))]
mod tests {
    use super::*;
    use exact_update::{sha256_hex, Check, Embedded, PreparedSelection, Store, Trust};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    const BYTECODE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/pure.hbc"));

    struct Directory(PathBuf);

    impl Directory {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "exact-remote-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn store(&self) -> Store {
            Store::open(
                &self.0,
                Embedded {
                    app_id: "com.exact.caltrain".into(),
                    compatibility_id: "remote-test".into(),
                    seq: 0,
                    channel: "test".into(),
                    trust: Trust::Development,
                    verification_keys: vec![],
                    embedded_plan_sha256: None,
                    embedded_assets: None,
                    entry_digest: None,
                },
            )
            .unwrap()
        }
    }

    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn stage(store: &mut Store, seq: u64) -> PreparedSelection {
        let plan = include_bytes!("../../update/tests/it/fixtures/publisher/app.plan");
        let marker = seq.to_string().into_bytes();
        let head = serde_json::to_vec(&serde_json::json!({
            "exact": 1,
            "app": {"id": "com.exact.caltrain", "name": "Remote test"},
            "stream": {"app": "com.exact.caltrain", "channel": "test", "compatibilityId": "remote-test", "seq": seq},
            "plan": {"url": "./app.plan", "sha256": sha256_hex(plan), "bytes": plan.len()},
            "assets": [
                {"name": "remote.hbc", "url": "./remote.hbc", "sha256": sha256_hex(BYTECODE), "bytes": BYTECODE.len()},
                {"name": "version.txt", "url": "./version.txt", "sha256": sha256_hex(&marker), "bytes": marker.len()}
            ]
        }))
        .unwrap();
        let outcome = store.check(
            "https://remote.test/exact.json",
            &mut |url| match url {
                "https://remote.test/exact.json" => Ok(head.clone()),
                "https://remote.test/app.plan" => Ok(plan.to_vec()),
                "https://remote.test/remote.hbc" => Ok(BYTECODE.to_vec()),
                "https://remote.test/version.txt" => Ok(marker.clone()),
                _ => Err(format!("unexpected URL: {url}")),
            },
            &mut |_| None,
        );
        assert!(matches!(outcome, Ok(Check::Staged { seq: found, .. }) if found == seq));
        store.prepare_activation().unwrap().unwrap()
    }

    #[test]
    fn admission_rejects_absent_missing_and_tampered_assets() {
        for missing in [false, true] {
            let dir = Directory::new();
            let mut store = dir.store();
            let selected = stage(&mut store, 1);
            assert!(PreparedRemote::new(&selected.assets, "absent.hbc", "test.pure", "").is_err());
            let path = store.staged().unwrap().assets_dir.join("remote.hbc");
            if missing {
                std::fs::remove_file(path).unwrap();
            } else {
                let mut bytes = BYTECODE.to_vec();
                bytes[0] ^= 1;
                std::fs::write(path, bytes).unwrap();
            }
            assert!(PreparedRemote::new(&selected.assets, "remote.hbc", "test.pure", "").is_err());
        }
    }

    #[test]
    fn admitted_remote_keeps_its_bytes_and_identity_across_store_activation() {
        let dir = Directory::new();
        let mut store = dir.store();
        let first = stage(&mut store, 1);
        let first_path = store.staged().unwrap().assets_dir.join("remote.hbc");
        let mut remote = PreparedRemote::new(&first.assets, "remote.hbc", "test.pure", "").unwrap();
        assert_eq!(remote.generation(), &first.generation);
        store.commit_activation(&first.generation).unwrap();
        let second = stage(&mut store, 2);
        let next = PreparedRemote::new(&second.assets, "remote.hbc", "test.pure", "").unwrap();
        store.commit_activation(&second.generation).unwrap();
        assert_eq!(next.generation(), &second.generation);
        assert_ne!(remote.generation(), next.generation());
        assert_eq!(store.generation(), second.generation);
        std::fs::remove_file(first_path).unwrap();
        let plan = contract::compile("component App\n  resource base64 = base64() as shape string\n  view\n    text base64\n").unwrap();
        remote.bind(&plan).unwrap();
        remote.activate().unwrap();
        assert!(remote.query("base64", &[]).unwrap().as_str().is_some());
        assert_eq!(remote.generation(), &first.generation);
    }

    fn remote(app: &str, grants: &str) -> PreparedRemote {
        PreparedRemote {
            generation: Generation {
                entry: Some("test-generation".into()),
                seq: 7,
            },
            module: Some(Module::new(
                include_bytes!(concat!(env!("OUT_DIR"), "/pure.hbc")).to_vec(),
                app,
                grants,
            )),
            active: false,
        }
    }

    #[test]
    fn isolated_remote_requires_activation_and_disposal_revokes_calls() {
        let plan = contract::compile("component App\n  resource base64 = base64() as shape string\n  view\n    text base64\n").unwrap();
        let mut remote = remote("test.pure", "");
        remote.bind(&plan).unwrap();
        assert!(remote.query("base64", &[]).is_err());
        remote.activate().unwrap();
        assert!(remote.bind(&plan).unwrap_err().contains("already active"));
        assert!(remote.query("base64", &[]).unwrap().as_str().is_some());
        remote.dispose();
        assert!(remote.query("base64", &[]).is_err());
        assert!(remote.activate().is_err());
        assert!(remote.bind(&plan).is_err());
        assert_eq!(remote.generation().seq, 7);
    }

    #[test]
    fn failed_identity_or_grants_disposes_the_remote() {
        for (app, grants) in [("wrong", ""), ("test.pure", "network example.com")] {
            let mut remote = remote(app, grants);
            assert!(remote.activate().is_err());
            assert!(remote.module.is_none());
            assert!(remote.activate().unwrap_err().contains("disposed"));
        }
    }
}
