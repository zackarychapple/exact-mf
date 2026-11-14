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

/// An opaque, single-use remote preparation token.
pub struct RemotePreparation {
    handle: RemoteHandle,
}

/// Owned download inputs. Only this value and its result may cross to a network worker.
pub struct RemoteCheckRequest {
    owner: std::sync::Arc<()>,
    generation: Generation,
    request: exact_update::client::CheckRequest,
}

/// A single-use download result. It contains no executor objects.
pub struct RemoteDownloadedCheck {
    owner: std::sync::Arc<()>,
    generation: Generation,
    downloaded: exact_update::client::DownloadedCheck,
}

impl RemoteCheckRequest {
    /// Fetch verified bytes without access to the coordinator or its executor.
    pub fn fetch(
        self,
        fetch: &mut dyn FnMut(&str) -> Result<Vec<u8>, String>,
    ) -> RemoteDownloadedCheck {
        RemoteDownloadedCheck {
            owner: self.owner,
            generation: self.generation,
            downloaded: self.request.fetch(fetch),
        }
    }
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
/// Each successful commit replaces and disposes the previous execution, including the same generation.
pub struct RemoteCoordinator {
    store: exact_update::Store,
    download_owner: std::sync::Arc<()>,
    pending: Option<Candidate>,
    active: Option<Candidate>,
    revoked: Vec<Generation>,
}

impl RemoteCoordinator {
    /// Take exclusive Store ownership and hold candidates until explicit commit.
    pub fn new(mut store: exact_update::Store) -> Self {
        store.hold_staged(true);
        Self {
            store,
            download_owner: std::sync::Arc::new(()),
            pending: None,
            active: None,
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

    /// Create owned worker inputs using the existing Store download protocol.
    pub fn begin_check(&self, url: &str, assets_root: &std::path::Path) -> RemoteCheckRequest {
        RemoteCheckRequest {
            owner: self.download_owner.clone(),
            generation: self.store.generation(),
            request: self.store.begin_check(url, assets_root),
        }
    }

    /// Accept a result only for this owner and the complete generation that requested it.
    /// Store then revalidates signatures, digests, and the live anti-rollback floor.
    pub fn finish_check(
        &mut self,
        downloaded: RemoteDownloadedCheck,
    ) -> Result<exact_update::Check, String> {
        if !std::sync::Arc::ptr_eq(&self.download_owner, &downloaded.owner) {
            return Err("foreign remote download".into());
        }
        if self.store.generation() != downloaded.generation {
            return Err("stale remote download generation".into());
        }
        self.store.finish_download(downloaded.downloaded)
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
    /// Remote activation failure consumes the preparation. Validation failure leaves it available for abandonment.
    /// Either failure leaves the committed execution unchanged.
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
        if let Some(mut previous) = self.active.replace(candidate) {
            previous.remote.dispose();
        }
        Ok(handle)
    }

    /// Count a boot only for the committed execution that the presenter accepted.
    pub fn boot_started(&mut self, handle: &RemoteHandle) -> Result<(), String> {
        self.active_generation(handle)?;
        self.store.boot_started()
    }

    /// Record successful rendering only for the current committed execution.
    /// Pending, foreign, and replaced handles cannot bless a selected generation.
    pub fn boot_succeeded(&mut self, handle: &RemoteHandle) -> Result<(), String> {
        let generation = self.active_generation(handle)?;
        self.store.boot_succeeded(&generation)
    }

    fn active_generation(&self, handle: &RemoteHandle) -> Result<Generation, String> {
        let active = self
            .active
            .as_ref()
            .filter(|candidate| std::rc::Rc::ptr_eq(&candidate.handle.0, &handle.0))
            .ok_or("stale or foreign remote handle")?;
        if active.selection.generation != self.store.generation() {
            return Err("stale remote generation".into());
        }
        Ok(active.selection.generation.clone())
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
    /// Revocation lasts only for this coordinator. Reopening Store does not restore it.
    pub fn revoke(&mut self, generation: &Generation) {
        if !self.revoked.contains(generation) {
            self.revoked.push(generation.clone());
        }
        if self
            .active
            .as_ref()
            .is_some_and(|c| &c.selection.generation == generation)
        {
            if let Some(mut candidate) = self.active.take() {
                candidate.remote.dispose();
            }
        }
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
            .as_mut()
            .filter(|c| std::rc::Rc::ptr_eq(&c.handle.0, &handle.0))
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

    fn download(owner: &RemoteCoordinator, seq: u64) -> RemoteDownloadedCheck {
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
        owner
            .begin_check(
                "https://remote.test/exact.json",
                std::path::Path::new("/nonexistent"),
            )
            .fetch(&mut |url| match url {
                "https://remote.test/exact.json" => Ok(head.clone()),
                "https://remote.test/app.plan" => Ok(plan.to_vec()),
                "https://remote.test/remote.hbc" => Ok(BYTECODE.to_vec()),
                "https://remote.test/version.txt" => Ok(marker.clone()),
                _ => Err(format!("unexpected URL: {url}")),
            })
    }

    fn stage(owner: &mut RemoteCoordinator, seq: u64) -> PreparedSelection {
        let result = download(owner, seq);
        let outcome = owner.finish_check(result);
        assert!(matches!(outcome, Ok(Check::Staged { seq: found, .. }) if found == seq));
        owner.store().prepare_activation().unwrap().unwrap()
    }

    fn source_plan() -> exact_plan::Plan {
        contract::compile("component App\n  resource base64 = base64() as shape string\n  view\n    text base64\n").unwrap()
    }

    #[test]
    fn boot_marks_require_the_current_committed_execution() {
        let dir = Directory::new();
        let mut owner = RemoteCoordinator::new(dir.store());
        let plan = source_plan();
        let record = || std::fs::read(dir.0.join("record.json")).unwrap();
        let mut previous = None;
        for seq in [1, 2] {
            let selected = stage(&mut owner, seq);
            let token = owner
                .prepare(&selected.generation, "remote.hbc", "test.pure", "", &plan)
                .unwrap();
            let handle = owner.activate_after_first_pixel(&token).unwrap();
            let before = record();
            assert!(owner.boot_started(&handle).is_err());
            assert!(owner.boot_succeeded(&handle).is_err());
            assert_eq!(record(), before);
            owner.commit(&token).unwrap();
            if let Some(old) = previous {
                let before = record();
                assert!(owner.boot_started(&old).is_err());
                assert!(owner.boot_succeeded(&old).is_err());
                assert_eq!(record(), before);
            }
            let foreign = RemoteHandle(std::rc::Rc::new(handle.execution_id()));
            assert!(owner.boot_succeeded(&foreign).is_err());
            owner.boot_started(&handle).unwrap();
            let started: serde_json::Value = serde_json::from_slice(&record()).unwrap();
            assert_eq!(started["failures"], 1);
            owner.boot_succeeded(&handle).unwrap();
            let succeeded: serde_json::Value = serde_json::from_slice(&record()).unwrap();
            assert_eq!(succeeded["failures"], 0);
            assert_eq!(succeeded["lastGood"], selected.generation.entry.unwrap());
            previous = Some(handle);
        }
    }

    #[test]
    fn concurrent_downloads_can_finish_in_sequence() {
        let dir = Directory::new();
        let mut owner = RemoteCoordinator::new(dir.store());
        let older = download(&owner, 1);
        let newer = download(&owner, 2);
        assert!(matches!(
            owner.finish_check(older),
            Ok(Check::Staged { seq: 1, .. })
        ));
        assert!(matches!(
            owner.finish_check(newer),
            Ok(Check::Staged { seq: 2, .. })
        ));
        let selected = owner.store().prepare_activation().unwrap().unwrap();
        assert_eq!(selected.generation.seq, 2);
        assert_eq!(
            selected
                .assets
                .resolve("remote.hbc")
                .unwrap()
                .unwrap()
                .as_ref(),
            BYTECODE
        );
    }

    #[test]
    fn downloads_revalidate_live_floor_and_generation_without_blocking_queries() {
        let dir = Directory::new();
        let mut owner = RemoteCoordinator::new(dir.store());
        let first = stage(&mut owner, 1);
        let plan = source_plan();
        let token = owner
            .prepare(&first.generation, "remote.hbc", "test.pure", "", &plan)
            .unwrap();
        let active = owner.activate_after_first_pixel(&token).unwrap();
        owner.commit(&token).unwrap();

        let request = owner.begin_check(
            "https://remote.test/exact.json",
            std::path::Path::new("/nonexistent"),
        );
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            request.fetch(&mut |_| {
                entered_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                Err("held fetch released".into())
            })
        });
        entered_rx.recv().unwrap();
        assert!(owner.query(&active, "base64", &[]).is_ok());
        // The worker cannot complete until this owner explicitly releases the barrier.
        assert!(!worker.is_finished());
        release_tx.send(()).unwrap();
        assert!(owner
            .finish_check(worker.join().unwrap())
            .unwrap_err()
            .contains("held fetch"));

        let older = download(&owner, 2);
        let newer = download(&owner, 3);
        assert!(matches!(
            owner.finish_check(newer),
            Ok(Check::Staged { seq: 3, .. })
        ));
        assert!(owner
            .finish_check(older)
            .unwrap_err()
            .contains("below the accepted seq 3"));
        let delayed = download(&owner, 3);
        let selected = owner.store().prepare_activation().unwrap().unwrap();
        let token = owner
            .prepare(&selected.generation, "remote.hbc", "test.pure", "", &plan)
            .unwrap();
        let abandoned = owner.activate_after_first_pixel(&token).unwrap();
        owner.abandon(&token).unwrap();
        assert!(owner.query(&abandoned, "base64", &[]).is_err());
        let retry = owner
            .prepare(&selected.generation, "remote.hbc", "test.pure", "", &plan)
            .unwrap();
        let next = owner.activate_after_first_pixel(&retry).unwrap();
        owner.commit(&retry).unwrap();
        assert!(owner
            .finish_check(delayed)
            .unwrap_err()
            .contains("stale remote download generation"));
        assert!(owner.query(&active, "base64", &[]).is_err());
        assert!(owner.query(&next, "base64", &[]).is_ok());

        let other_dir = Directory::new();
        let other = RemoteCoordinator::new(other_dir.store());
        assert!(owner
            .finish_check(download(&other, 4))
            .unwrap_err()
            .contains("foreign remote download"));
        // Dropping a result abandons only that download. The same generation can be fetched again.
        drop(download(&owner, 4));
        assert!(matches!(
            owner.finish_check(download(&owner, 4)),
            Ok(Check::Staged { seq: 4, .. })
        ));
    }

    #[test]
    fn coordinator_disposes_replaced_abandoned_and_failed_realms() {
        let dir = Directory::new();
        let mut owner = RemoteCoordinator::new(dir.store());
        let plan = source_plan();
        let mut previous = None;
        for seq in 1..=3 {
            let selected = stage(&mut owner, seq);
            for _ in 0..2 {
                let token = owner
                    .prepare(&selected.generation, "remote.hbc", "test.pure", "", &plan)
                    .unwrap();
                assert!(owner.commit(&token).is_err());
                if let Some((handle, watch)) = &previous {
                    assert!(std::sync::Weak::upgrade(watch).is_some());
                    assert!(owner.query(handle, "base64", &[]).is_ok());
                }
                let handle = owner.activate_after_first_pixel(&token).unwrap();
                let module = owner
                    .pending
                    .as_ref()
                    .unwrap()
                    .remote
                    .module
                    .as_ref()
                    .unwrap();
                assert!(module.is_loaded());
                // Both Module and its live engine own this watch. Its release proves both were dropped.
                let watch = std::sync::Arc::downgrade(&module.watch);
                owner.commit(&token).unwrap();
                if let Some((old_handle, old_watch)) = previous.take() {
                    assert!(old_watch.upgrade().is_none());
                    assert!(owner.query(&old_handle, "base64", &[]).is_err());
                }
                assert!(watch.upgrade().is_some());
                assert!(owner.query(&handle, "base64", &[]).is_ok());
                previous = Some((handle, watch));
            }
        }
        let (current, watch) = previous.unwrap();
        let generation = owner.store().generation();
        let selected = stage(&mut owner, 4);
        let token = owner
            .prepare(&selected.generation, "remote.hbc", "test.pure", "", &plan)
            .unwrap();
        let abandoned = owner.activate_after_first_pixel(&token).unwrap();
        let abandoned_watch = std::sync::Arc::downgrade(
            &owner
                .pending
                .as_ref()
                .unwrap()
                .remote
                .module
                .as_ref()
                .unwrap()
                .watch,
        );
        let selected = stage(&mut owner, 5);
        assert!(owner.commit(&token).is_err());
        assert!(abandoned_watch.upgrade().is_some());
        assert!(owner.query(&abandoned, "base64", &[]).is_err());
        owner.abandon(&token).unwrap();
        assert!(abandoned_watch.upgrade().is_none());
        assert!(owner.query(&abandoned, "base64", &[]).is_err());
        assert_eq!(owner.store().generation(), generation);
        assert!(watch.upgrade().is_some());
        assert!(owner.query(&current, "base64", &[]).is_ok());

        let failed = owner
            .prepare(&selected.generation, "remote.hbc", "wrong", "", &plan)
            .unwrap();
        let failed_watch = std::sync::Arc::downgrade(
            &owner
                .pending
                .as_ref()
                .unwrap()
                .remote
                .module
                .as_ref()
                .unwrap()
                .watch,
        );
        assert!(owner.activate_after_first_pixel(&failed).is_err());
        assert!(failed_watch.upgrade().is_none());
        assert!(owner.commit(&failed).is_err());
        assert_eq!(owner.store().generation(), generation);
        assert!(watch.upgrade().is_some());
        assert!(owner.query(&current, "base64", &[]).is_ok());
        owner.revoke(&owner.store().generation());
        assert!(watch.upgrade().is_none());
    }

    #[test]
    fn coordinator_preserves_selection_on_refusal_and_commits_exact_generation() {
        let dir = Directory::new();
        let mut owner = RemoteCoordinator::new(dir.store());
        let first = stage(&mut owner, 1);
        let plan = source_plan();
        let first_token = owner
            .prepare(&first.generation, "remote.hbc", "test.pure", "", &plan)
            .unwrap();
        let first_handle = owner.activate_after_first_pixel(&first_token).unwrap();
        owner.commit(&first_token).unwrap();
        assert!(owner.commit(&first_token).is_err());
        let second = stage(&mut owner, 2);
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
        let third = stage(&mut owner, 3);
        let stale = owner
            .prepare(&third.generation, "remote.hbc", "test.pure", "", &plan)
            .unwrap();
        let stale_handle = owner.activate_after_first_pixel(&stale).unwrap();
        stage(&mut owner, 4);
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
        let one = stage(&mut left, 1);
        stage(&mut right, 1);
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
        stage(&mut left, 2);
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
            let mut owner = RemoteCoordinator::new(dir.store());
            let selected = stage(&mut owner, 1);
            assert!(PreparedRemote::new(&selected.assets, "absent.hbc", "test.pure", "").is_err());
            let path = owner
                .store()
                .staged()
                .unwrap()
                .assets_dir
                .join("remote.hbc");
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
        let mut owner = RemoteCoordinator::new(dir.store());
        let first = stage(&mut owner, 1);
        let first_path = owner
            .store()
            .staged()
            .unwrap()
            .assets_dir
            .join("remote.hbc");
        let mut remote = PreparedRemote::new(&first.assets, "remote.hbc", "test.pure", "").unwrap();
        assert_eq!(remote.generation(), &first.generation);
        owner.store.commit_activation(&first.generation).unwrap();
        let second = stage(&mut owner, 2);
        let next = PreparedRemote::new(&second.assets, "remote.hbc", "test.pure", "").unwrap();
        owner.store.commit_activation(&second.generation).unwrap();
        assert_eq!(next.generation(), &second.generation);
        assert_ne!(remote.generation(), next.generation());
        assert_eq!(owner.store().generation(), second.generation);
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
