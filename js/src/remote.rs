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

/// An opaque execution incarnation. Adapters must use this identity for cached proxies.
/// Clones do not extend execution lifetime or bypass revocation.
#[derive(Clone)]
pub struct RemoteHandle(std::rc::Rc<u64>);

impl RemoteHandle {
    /// Process-local cache key. Never reuse this key for another execution incarnation.
    /// This key is not an authorization token or a durable generation identity.
    pub fn execution_id(&self) -> u64 {
        *self.0
    }
}

/// An opaque, single-use whole-owner preparation token.
pub struct RemotePreparation {
    handle: RemoteHandle,
}

struct Candidate {
    handle: RemoteHandle,
    remote: PreparedRemote,
    selection: exact_update::PreparedSelection,
    staged: bool,
}

/// Host-owned admission, execution, and durable selection for one update store.
/// MF cache eviction remains the adapter's responsibility. Each retry gets a distinct handle.
/// Abandonment preserves Store's pending candidate and authenticated anti-rollback floor.
pub struct RemoteCoordinator {
    store: exact_update::Store,
    pending: Option<Candidate>,
    active: Vec<Candidate>,
    revoked: Vec<Generation>,
}

impl RemoteCoordinator {
    /// Take exclusive Store ownership and hold candidates until explicit commit.
    pub fn new(mut store: exact_update::Store) -> Self {
        store.hold_staged(true);
        Self {
            store,
            pending: None,
            active: Vec::new(),
            revoked: Vec::new(),
        }
    }

    /// Inspect admission status without exposing mutable selection APIs.
    pub fn store(&self) -> &exact_update::Store {
        &self.store
    }

    /// Admit an update through Store while keeping durable selection unchanged.
    pub fn check(
        &mut self,
        url: &str,
        fetch: &mut dyn FnMut(&str) -> Result<Vec<u8>, String>,
        embedded: &mut dyn FnMut(&str) -> Option<String>,
    ) -> Result<exact_update::Check, String> {
        self.store.check(url, fetch, embedded)
    }

    /// Pin Store bytes and host-selected execution policy without running code.
    /// `expected` must be the complete generation resolved by the host, never an alias.
    pub fn prepare(
        &mut self,
        expected: &Generation,
        asset: &str,
        app_id: &str,
        grants: &str,
        plan: &exact_plan::Plan,
    ) -> Result<RemotePreparation, String> {
        if self.pending.is_some() {
            return Err("remote preparation is pending".into());
        }
        if self.revoked.contains(expected) {
            return Err("remote generation is revoked".into());
        }
        let staged = self.store.prepare_activation()?;
        let (selection, staged) = match staged {
            Some(selection) => (selection, true),
            None => (
                self.store
                    .prepare_selected()
                    .map_err(|e| e.reason)?
                    .ok_or("no selected remote")?,
                false,
            ),
        };
        if &selection.generation != expected || selection.assets.generation() != expected {
            return Err("remote generation differs from requested generation".into());
        }
        let mut remote = PreparedRemote::new(&selection.assets, asset, app_id, grants)?;
        remote.bind(plan)?;
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = NEXT
            .fetch_update(
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
                |value| value.checked_add(1),
            )
            .map_err(|_| "remote execution identities exhausted")?;
        let handle = RemoteHandle(std::rc::Rc::new(id));
        self.pending = Some(Candidate {
            handle: handle.clone(),
            remote,
            selection,
            staged,
        });
        Ok(RemotePreparation { handle })
    }

    fn validate(&self, token: &RemotePreparation) -> Result<(), String> {
        let candidate = self.pending.as_ref().ok_or("stale remote preparation")?;
        if !std::rc::Rc::ptr_eq(&candidate.handle.0, &token.handle.0) {
            return Err("foreign or stale remote preparation".into());
        }
        if self.revoked.contains(&candidate.selection.generation) {
            return Err("remote generation is revoked".into());
        }
        let current = if candidate.staged {
            self.store
                .prepare_activation()?
                .ok_or("remote candidate is no longer staged")?
                .generation
        } else {
            self.store.generation()
        };
        if current != candidate.selection.generation {
            return Err("remote preparation is stale".into());
        }
        Ok(())
    }

    /// Execute only after the caller has presented its first pixel.
    /// Failure consumes the preparation and leaves all active remotes unchanged.
    pub fn activate_after_first_pixel(
        &mut self,
        token: &RemotePreparation,
    ) -> Result<RemoteHandle, String> {
        self.validate(token)?;
        let candidate = self.pending.as_mut().ok_or("stale remote preparation")?;
        if let Err(error) = candidate.remote.activate() {
            self.pending = None;
            return Err(error);
        }
        Ok(candidate.handle.clone())
    }

    /// Inspect immutable candidate bytes for whole-owner preparation.
    pub fn selection(
        &self,
        token: &RemotePreparation,
    ) -> Result<&exact_update::PreparedSelection, String> {
        self.validate(token)?;
        Ok(&self
            .pending
            .as_ref()
            .ok_or("stale remote preparation")?
            .selection)
    }

    /// Commit this generation after the caller accepts its whole owner.
    /// A refusal preserves the token for abandonment. Success rejects subsequent replay.
    pub fn commit(&mut self, token: &RemotePreparation) -> Result<RemoteHandle, String> {
        self.validate(token)?;
        let candidate = self.pending.as_ref().ok_or("stale remote preparation")?;
        if !candidate.remote.active {
            return Err("remote is not active".into());
        }
        if candidate.staged {
            self.store
                .commit_activation(&candidate.selection.generation)?;
        }
        let candidate = self.pending.take().ok_or("stale remote preparation")?;
        let handle = candidate.handle.clone();
        self.active.push(candidate);
        Ok(handle)
    }

    /// Dispose this candidate without changing selected UI or unrelated execution state.
    pub fn abandon(&mut self, token: &RemotePreparation) -> Result<(), String> {
        let candidate = self.pending.as_ref().ok_or("stale remote preparation")?;
        if !std::rc::Rc::ptr_eq(&candidate.handle.0, &token.handle.0) {
            return Err("foreign or stale remote preparation".into());
        }
        if let Some(mut candidate) = self.pending.take() {
            candidate.remote.dispose();
        }
        Ok(())
    }

    /// Revoke every execution incarnation of this complete generation.
    pub fn revoke(&mut self, generation: &Generation) {
        if !self.revoked.contains(generation) {
            self.revoked.push(generation.clone());
        }
        self.active.retain_mut(|c| {
            if &c.selection.generation == generation {
                c.remote.dispose();
                false
            } else {
                true
            }
        });
        if let Some(c) = self.pending.as_mut() {
            if &c.selection.generation == generation {
                c.remote.dispose();
            }
        }
    }

    /// Query an admitted incarnation through isolated values and the existing turn budget.
    pub fn query(
        &mut self,
        handle: &RemoteHandle,
        source: &str,
        args: &[Value],
    ) -> Result<Value, String> {
        if let Some(c) = self.pending.as_ref() {
            if std::rc::Rc::ptr_eq(&c.handle.0, &handle.0) {
                let token = RemotePreparation {
                    handle: handle.clone(),
                };
                self.validate(&token)?;
                return self
                    .pending
                    .as_mut()
                    .ok_or("stale remote preparation")?
                    .remote
                    .query(source, args);
            }
        }
        let c = self
            .active
            .iter_mut()
            .find(|c| std::rc::Rc::ptr_eq(&c.handle.0, &handle.0))
            .ok_or("stale or foreign remote handle")?;
        if c.selection.generation != self.store.generation() {
            return Err("stale remote generation".into());
        }
        c.remote.query(source, args)
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

    fn source_plan() -> exact_plan::Plan {
        contract::compile("component App\n  resource base64 = base64() as shape string\n  view\n    text base64\n").unwrap()
    }

    #[test]
    fn coordinator_preserves_selection_on_refusal_and_commits_exact_generation() {
        let dir = Directory::new();
        let mut store = dir.store();
        store.hold_staged(true);
        let first = stage(&mut store, 1);
        store.commit_activation(&first.generation).unwrap();
        let mut owner = RemoteCoordinator::new(store);
        let plan = source_plan();
        let first_token = owner
            .prepare(&first.generation, "remote.hbc", "test.pure", "", &plan)
            .unwrap();
        let first_handle = owner.activate_after_first_pixel(&first_token).unwrap();
        owner.commit(&first_token).unwrap();
        assert!(owner.commit(&first_token).is_err());
        let second = stage(&mut owner.store, 2);
        assert!(owner
            .prepare(&first.generation, "remote.hbc", "test.pure", "", &plan)
            .is_err());
        let mut wrong_digest = second.generation.clone();
        wrong_digest.entry = first.generation.entry.clone();
        assert!(owner
            .prepare(&wrong_digest, "remote.hbc", "test.pure", "", &plan)
            .is_err());
        let failed = owner
            .prepare(&second.generation, "remote.hbc", "wrong", "", &plan)
            .unwrap();
        assert!(owner.activate_after_first_pixel(&failed).is_err());
        assert!(owner.commit(&failed).is_err());
        assert_eq!(owner.store().generation(), first.generation);
        assert!(owner.query(&first_handle, "base64", &[]).is_ok());
        drop(owner);
        let mut owner = RemoteCoordinator::new(dir.store());
        assert_eq!(owner.store().generation(), first.generation);
        let token = owner
            .prepare(&second.generation, "remote.hbc", "test.pure", "", &plan)
            .unwrap();
        assert!(owner.commit(&token).is_err());
        let abandoned = owner.activate_after_first_pixel(&token).unwrap();
        owner.abandon(&token).unwrap();
        assert!(owner.query(&abandoned, "base64", &[]).is_err());
        assert!(owner.activate_after_first_pixel(&token).is_err());
        assert_eq!(owner.store().generation(), first.generation);
        drop(owner);
        let mut owner = RemoteCoordinator::new(dir.store());
        assert_eq!(owner.store().generation(), first.generation);
        let retry = owner
            .prepare(&second.generation, "remote.hbc", "test.pure", "", &plan)
            .unwrap();
        let handle = owner.activate_after_first_pixel(&retry).unwrap();
        assert!(!std::rc::Rc::ptr_eq(&abandoned.0, &handle.0));
        assert!(owner.commit(&token).is_err());
        owner.commit(&retry).unwrap();
        assert!(owner.commit(&retry).is_err());
        assert!(owner.query(&first_handle, "base64", &[]).is_err());
        assert!(owner.query(&handle, "base64", &[]).is_ok());
        let third = stage(&mut owner.store, 3);
        let stale = owner
            .prepare(&third.generation, "remote.hbc", "test.pure", "", &plan)
            .unwrap();
        let stale_handle = owner.activate_after_first_pixel(&stale).unwrap();
        stage(&mut owner.store, 4);
        assert!(owner.commit(&stale).is_err());
        assert!(owner.query(&stale_handle, "base64", &[]).is_err());
        assert_eq!(owner.store().generation(), second.generation);
        owner.abandon(&stale).unwrap();
        assert!(owner.query(&handle, "base64", &[]).is_ok());
        owner.revoke(&second.generation);
        assert!(owner.query(&handle, "base64", &[]).is_err());
        assert!(owner
            .prepare(&second.generation, "remote.hbc", "test.pure", "", &plan)
            .is_err());
        drop(owner);
        let mut reopened = dir.store();
        assert_eq!(reopened.generation(), second.generation);
        assert_eq!(
            reopened.prepare_selected().unwrap().unwrap().generation,
            second.generation
        );
    }

    #[test]
    fn coordinator_rejects_foreign_stale_and_revoked_tokens_before_execution() {
        let a = Directory::new();
        let b = Directory::new();
        let mut left = RemoteCoordinator::new(a.store());
        let mut right = RemoteCoordinator::new(b.store());
        let one = stage(&mut left.store, 1);
        stage(&mut right.store, 1);
        let plan = source_plan();
        let token = left
            .prepare(&one.generation, "remote.hbc", "test.pure", "", &plan)
            .unwrap();
        let foreign = right
            .prepare(&one.generation, "remote.hbc", "test.pure", "", &plan)
            .unwrap();
        assert!(right.activate_after_first_pixel(&token).is_err());
        assert!(right.commit(&token).is_err());
        assert!(right.abandon(&token).is_err());
        right.revoke(&one.generation);
        assert!(right.activate_after_first_pixel(&foreign).is_err());
        right.abandon(&foreign).unwrap();
        stage(&mut left.store, 2);
        assert!(left.activate_after_first_pixel(&token).is_err());
        assert!(left.commit(&token).is_err());
        left.abandon(&token).unwrap();
        drop(left);
        drop(right);
        assert_eq!(a.store().generation().seq, 0);
        assert_eq!(b.store().generation().seq, 0);
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
