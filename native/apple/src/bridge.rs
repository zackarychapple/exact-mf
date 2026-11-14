use exact_js::{
    Module, NativeModule, NativeStorage, RemoteCoordinator, RemoteDownloadedCheck, RemoteHandle,
    RemotePreparation,
};
use exact_runner::{DataSource, Value};
use exact_update::{Embedded, PreparedSelection, Store, Trust};
use sha2::Digest;
use std::{
    cell::RefCell,
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::{atomic::Ordering, mpsc},
};
static ROOT: std::sync::OnceLock<String> = std::sync::OnceLock::new();
static RESOURCES: std::sync::OnceLock<String> = std::sync::OnceLock::new();
fn root() -> &'static str {
    ROOT.get().expect("configure storage first")
}
fn resource(name: &str) -> PathBuf {
    PathBuf::from(RESOURCES.get().expect("configure resources first")).join(name)
}
#[no_mangle]
pub unsafe extern "C" fn joined_configure(
    storage: *const std::ffi::c_char,
    resources: *const std::ffi::c_char,
) {
    let storage = unsafe { std::ffi::CStr::from_ptr(storage) }
        .to_str()
        .unwrap()
        .to_owned();
    let resources = unsafe { std::ffi::CStr::from_ptr(resources) }
        .to_str()
        .unwrap()
        .to_owned();
    fs::create_dir_all(&storage).unwrap();
    let _ = ROOT.set(storage);
    let _ = RESOURCES.set(resources);
}
struct Pending {
    id: u64,
    token: RemotePreparation,
    key: String,
}
thread_local! {
 static UPDATE: RefCell<Option<RemoteCoordinator>> = const {RefCell::new(None)};
 static REMOTES: RefCell<HashMap<String,(RemoteHandle,PreparedSelection)>> = RefCell::default();
 static PENDING: RefCell<Option<Pending>> = const {RefCell::new(None)};
 static ACTIVE: RefCell<Option<String>> = const {RefCell::new(None)};
 static RETAINED: RefCell<Option<RemoteHandle>> = const {RefCell::new(None)};
 static HOST: RefCell<HashMap<String, Module>> = RefCell::default();
 static SERIAL: std::cell::Cell<u64> = const {std::cell::Cell::new(1000)};
 static RESPONSE: RefCell<Response> = RefCell::new(Response { bytes:vec![0],token:0 });
}
fn with_owner<T>(f: impl FnOnce(&mut RemoteCoordinator) -> Result<T, String>) -> Result<T, String> {
    UPDATE.with(|u| {
        let mut u = u.borrow_mut();
        f(u.as_mut().ok_or("owner absent")?)
    })
}
fn initialize() -> Result<(), String> {
    UPDATE.with(|u| {
        if u.borrow().is_some() {
            return Ok(());
        }
        let embedded = Embedded {
            app_id: "com.zephyr.exact.mf".into(),
            compatibility_id: "visual-remote-v1".into(),
            seq: 0,
            channel: "demo".into(),
            trust: Trust::Production,
            verification_keys: vec![(
                "demo".into(),
                fs::read(resource("public-key.bin"))
                    .map_err(|e| e.to_string())?
                    .try_into()
                    .map_err(|_| "key length")?,
            )],
            embedded_plan_sha256: None,
            embedded_assets: None,
            entry_digest: None,
        };
        let mut store = Store::open(&PathBuf::from(root()).join("store"), embedded)?;
        #[cfg(not(test))]
        let origin = fs::read_to_string(resource("origin.txt")).map_err(|e| e.to_string())?;
        #[cfg(not(test))]
        store.follow_environment(&format!("{}/.exact/demo/visual-remote-v1/exact.json", origin.trim().trim_end_matches('/')))?;
        #[cfg(test)]
        store.follow_environment("https://fixture.invalid/v1/.exact/demo/visual-remote-v1/exact.json")?;
        *u.borrow_mut() = Some(RemoteCoordinator::new(store));
        Ok(())
    })
}
static PROXY_CREATED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static PROXY_DROPPED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
struct Proxy;
impl Drop for Proxy {
    fn drop(&mut self) {
        PROXY_DROPPED.fetch_add(1, Ordering::SeqCst);
    }
}
impl NativeModule for Proxy {
    fn configure_storage(&mut self, _: NativeStorage) -> Result<(), String> {
        Ok(())
    }
    fn call(&mut self, r: &serde_json::Value) -> Result<serde_json::Value, String> {
        let key = r["generation"].as_str().ok_or("execution identity")?;
        if r["op"] == "get" && r["id"] != "./value" {
            return Err("unknown expose".into());
        }
        let handle = REMOTES
            .with(|m| m.borrow().get(key).map(|x| x.0.clone()))
            .ok_or("unknown execution")?;
        let v = with_owner(|o| {
            o.query(
                &handle,
                "remote",
                &[Value::str(r["op"].as_str().ok_or("op")?)],
            )
        })?;
        eprintln!("MF lifecycle execution={} op={} completed", key, r["op"]);
        Ok(if r["op"] == "factory" {
            serde_json::json!(key)
        } else {
            serde_json::json!(v.as_str().ok_or("remote string")?)
        })
    }
}
fn mf(key: &str) -> Result<String, String> {
    HOST.with(|h|{
 let mut h=h.borrow_mut();
 if !h.contains_key(key){
 let mut host=Module::new(fs::read(resource("host.hbc")).map_err(|e|e.to_string())?,"test.mf","").with_native(|_|{PROXY_CREATED.fetch_add(1, Ordering::SeqCst);Box::new(Proxy)});
 host.configure_storage(PathBuf::from(root()).join("data"),PathBuf::from(root()).join("cache"),PathBuf::from(root()).join("tmp")).map_err(|e|e.to_string())?;
 let p=contract::compile("component App\n  resource proof = proof(\"v1\") as shape string\n  view\n    text proof\n").map_err(|e|e.to_string())?;
 host.bind(&p);host.activate().map_err(|e|e.to_string())?;h.insert(key.to_owned(),host);}
 let mut store=exact_runner::Store::new("",Vec::<(String,Vec<u8>)>::new());
 let answer=h.get_mut(key).unwrap().answer(&mut store,"proof",&[Value::str(key)]).map_err(|e|e.to_string())?;
 let exact_runner::Answer::Now(v)=answer else{return Err("async MF".into())};
 let j:serde_json::Value=serde_json::from_str(v.as_str().ok_or("MF string")?).map_err(|e|e.to_string())?;
 eprintln!("real MF result={j}");
 if j["registrations"] != 1 || j["modules"] != 1 { return Err(format!("MF resource bound failed: {j}")); }
 Ok(j["value"].as_str().ok_or("MF handle")?.into())
 })
}
fn validate_metadata(
    signed: &serde_json::Value,
    remote: &serde_json::Value,
    digest: &str,
) -> Result<(), String> {
    for field in ["name", "version"] {
        let value = signed[field]
            .as_str()
            .filter(|v| !v.is_empty())
            .ok_or("signed release metadata absent")?;
        if remote[field].as_str() != Some(value) {
            return Err("remote metadata does not match signed generation".into());
        }
    }
    if signed["planSha256"].as_str() != Some(digest) {
        return Err("release plan digest does not match admitted plan".into());
    }
    Ok(())
}
fn resolve(key: &str) -> Result<serde_json::Value, String> {
    let (handle, selected) = REMOTES
        .with(|m| m.borrow().get(key).cloned())
        .ok_or("unknown execution")?;
    let result = with_owner(|o| o.query(&handle, "remote", &[Value::str("check")]))?;
    let metadata: serde_json::Value =
        serde_json::from_str(result.as_str().ok_or("remote metadata")?)
            .map_err(|e| e.to_string())?;
    let release = selected
        .assets
        .resolve("release.json")?
        .ok_or("release metadata absent")?;
    let signed: serde_json::Value = serde_json::from_slice(&release).map_err(|e| e.to_string())?;
    let plan_digest = format!("{:x}", sha2::Sha256::digest(&selected.plan));
    validate_metadata(&signed, &metadata, &plan_digest)?;

    let mut assets = serde_json::Map::new();
    for n in selected.assets.names() {
        let b = selected.assets.resolve(&n)?.ok_or("asset absent")?;
        assets.insert(n, serde_json::json!(b.as_ref()));
    }
    Ok(
        serde_json::json!({"release":signed,"entry":selected.generation.entry,"planDigest":plan_digest,"handle":key,"seq":selected.generation.seq,"plan":selected.plan.as_ref(),"assets":assets}),
    )
}
fn prepare() -> Result<serde_json::Value, String> {
    let selected = with_owner(|o| {
        if let Some(selected) = o.store().prepare_activation()? {
            return Ok(selected);
        }
        ACTIVE.with(|a| {
            let active = a.borrow();
            REMOTES.with(|m| {
                m.borrow()
                    .get(active.as_deref().unwrap_or(""))
                    .map(|entry| entry.1.clone())
                    .ok_or("no selected generation".into())
            })
        })
    })?;
    eprintln!("verified complete generation {:?}", selected.generation);
    let hbc = selected
        .assets
        .resolve("remote.hbc")?
        .ok_or("remote bytecode absent")?;
    eprintln!(
        "remote bytecode={:?} executor={} seam={} engine={:?}",
        hbc.get(8..12),
        exact_js::BYTECODE_VERSION,
        exact_js::ABI,
        exact_js::ENGINE_INPUTS
    );
    let current = ACTIVE.with(|a| a.borrow().clone());
    if current.as_ref().is_some_and(|key| {
        REMOTES.with(|m| {
            m.borrow()
                .get(key)
                .is_some_and(|entry| entry.1.generation == selected.generation)
        })
    }) {
        return Ok(serde_json::json!({"current":true,"seq":selected.generation.seq}));
    }
    let plan=contract::compile("component App\n  resource remote = remote(\"check\") as shape string\n  view\n    text remote\n").map_err(|e|e.to_string())?;
    let id = SERIAL.with(|s| {
        let next = s.get().checked_add(1).ok_or("response token exhausted")?;
        s.set(next);
        Ok::<_, String>(next)
    })?;
    let token = with_owner(|o| {
        o.prepare(
            &selected.generation,
            "remote.hbc",
            "remote.proof",
            "",
            &plan,
        )
    })?;
    let handle = match with_owner(|o| o.activate_after_first_pixel(&token)) {
        Ok(h) => h,
        Err(e) => {
            let _ = with_owner(|o| o.abandon(&token));
            return Err(e);
        }
    };
    let key = handle.execution_id().to_string();

    let selected = with_owner(|o| Ok(o.selection(&token)?.clone()))?;
    REMOTES.with(|m| m.borrow_mut().insert(key.clone(), (handle, selected)));
    PENDING.with(|p| {
        *p.borrow_mut() = Some(Pending {
            id,
            token,
            key: key.clone(),
        })
    });
    let result = mf(&key).and_then(|h| resolve(&h));
    match result {
        Ok(mut j) => {
            j["token"] = serde_json::json!(id);
            Ok(j)
        }
        Err(e) => {
            finish(id, false);
            Err(e)
        }
    }
}
fn finish(id: u64, commit: bool) -> bool {
    let result = PENDING.with(|p| {
        let mut p = p.borrow_mut();
        let current = p.as_ref().ok_or("no pending preparation")?;
        if current.id != id {
            return Err("foreign response token".into());
        }
        if commit {
            with_owner(|o| o.commit(&current.token)).map(|_| ())?;
        } else {
            with_owner(|o| o.abandon(&current.token))?;
        }
        let current = p.take().unwrap();
        if commit {
            let old = ACTIVE.with(|a| a.borrow_mut().replace(current.key.clone()));
            if RETAINED.with(|r| r.borrow().is_none()) {
                let handle =
                    REMOTES.with(|m| m.borrow().get(&current.key).map(|entry| entry.0.clone()));
                RETAINED.with(|r| *r.borrow_mut() = handle);
            }
            REMOTES.with(|m| m.borrow_mut().retain(|key, _| key == &current.key));
            HOST.with(|h| h.borrow_mut().retain(|key, _| key == &current.key));
            eprintln!("committed execution={} predecessor={old:?}", current.key);
        } else {
            REMOTES.with(|m| m.borrow_mut().remove(&current.key));
            HOST.with(|h| h.borrow_mut().remove(&current.key));
            eprintln!("abandoned matching response token={id}");
        }
        eprintln!(
            "resources remotes={} routing={} pending={} created={} dropped={}",
            REMOTES.with(|m| m.borrow().len()),
            HOST.with(|h| h.borrow().len()),
            usize::from(p.is_some()),
            PROXY_CREATED.load(Ordering::SeqCst),
            PROXY_DROPPED.load(Ordering::SeqCst)
        );
        Ok::<_, String>(())
    });
    if let Err(e) = &result {
        eprintln!("finish refused id={id} reason={e}")
    }
    result.is_ok()
}
struct Response {
    bytes: Vec<u8>,
    token: u64,
}
fn response(result: Result<serde_json::Value, String>) -> Response {
    let value = result.unwrap_or_else(|e| serde_json::json!({"error":e}));

    let token = value["token"].as_u64().unwrap_or(0);
    eprintln!("response token={token} error={}", value["error"]);
    let mut bytes = serde_json::to_vec(&value).unwrap();
    bytes.push(0);
    Response { bytes, token }
}
enum Work {
    Call(u32, mpsc::Sender<Response>),
    Downloaded(RemoteDownloadedCheck, mpsc::Sender<Response>),
    Finish(u64, bool, mpsc::Sender<bool>),
}
fn worker() -> &'static mpsc::Sender<Work> {
    static W: std::sync::OnceLock<mpsc::Sender<Work>> = std::sync::OnceLock::new();
    W.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        let back = tx.clone();
        std::thread::Builder::new()
            .name("exact-mf-owner".into())
            .spawn(move || {
                while let Ok(work) = rx.recv() {
                    match work {
                        Work::Finish(id, commit, done) => {
                            let _ = done.send(finish(id, commit));
                        }
                        Work::Downloaded(result, done) => {
                            let result = with_owner(|o| o.finish_check(result)).and_then(|check| {
                                eprintln!("production admission {check:?}");
                                prepare()
                            });
                            let _ = done.send(response(result));
                        }
                        Work::Call(op, done) => {
                            let started = (|| {
                                initialize()?;
                                let _ = op;
                                #[cfg(not(test))]
                                let origin = fs::read_to_string(resource("origin.txt"))
                                    .map_err(|e| e.to_string())?
                                    .trim()
                                    .trim_end_matches('/')
                                    .to_string();
                                #[cfg(test)]
                                let origin = fs::read_to_string(format!("{}/origin-{op}", root()))
                                    .map_err(|e| e.to_string())?;
                                let url =
                                    format!("{origin}/.exact/demo/visual-remote-v1/exact.json");
                                let request = with_owner(|o| {
                                    Ok(o.begin_check(&url, std::path::Path::new(root())))
                                })?;
                                let back = back.clone();
                                let done = done.clone();
                                std::thread::Builder::new()
                                    .name("exact-mf-network".into())
                                    .spawn(move || {
                                        let result = request.fetch(&mut |url| fetch(url, &origin));
                                        let _ = back.send(Work::Downloaded(result, done));
                                    })
                                    .map_err(|e| e.to_string())?;
                                Ok::<_, String>(())
                            })();
                            if let Err(error) = started {
                                let _ = done.send(response(Err(error)));
                            }
                        }
                    }
                }
            })
            .expect("executor");
        tx
    })
}
#[no_mangle]
pub extern "C" fn joined_call(op: u32) -> *const u8 {
    let (tx, rx) = mpsc::channel();
    worker().send(Work::Call(op, tx)).unwrap();
    let reply = rx.recv().unwrap();
    RESPONSE.with(|r| {
        *r.borrow_mut() = reply;
        r.borrow().bytes.as_ptr()
    })
}
#[no_mangle]
pub extern "C" fn joined_response_token() -> u64 {
    RESPONSE.with(|r| r.borrow().token)
}
#[no_mangle]
pub extern "C" fn joined_finish(id: u64, commit: u32) -> bool {
    let (tx, rx) = mpsc::channel();
    worker().send(Work::Finish(id, commit != 0, tx)).unwrap();
    rx.recv().unwrap_or(false)
}
fn fetch(url: &str, origin: &str) -> Result<Vec<u8>, String> {
    #[cfg(test)]
    if origin.starts_with("https://fixture.invalid/v") {
        let seq = origin.rsplit('/').next().unwrap();
        let app = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf();
        return fs::read(
            app.join(format!("public-{seq}")).join(
                url.strip_prefix(&format!("{origin}/"))
                    .ok_or("foreign fixture URL")?,
            ),
        )
        .map_err(|e| e.to_string());
    }

    if !origin.starts_with("https://") || !url.starts_with(&format!("{origin}/")) {
        return Err("unverified HTTPS origin".into());
    }

    eprintln!("HTTPS request pid={} url={url}", std::process::id());
    let mut request = ibex2::stdlib::fetch::Request::get(url);
    request.redirect = ibex2::stdlib::fetch::RedirectMode::Manual;
    request.timeout = Some(std::time::Duration::from_secs(12));
    request.max_body = Some(4 * 1024 * 1024);
    let response = ibex2::transport::default_transport()
        .send(&request)
        .map_err(|e| e.to_string())?;
    if response.status != 200 {
        return Err(format!("HTTP {}", response.status));
    }
    let bytes = response.body;
    eprintln!(
        "HTTPS response bytes={} sha256={:x}",
        bytes.len(),
        sha2::Sha256::digest(&bytes)
    );
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn call(op: u32) -> serde_json::Value {
        let p = joined_call(op);
        serde_json::from_slice(unsafe { std::ffi::CStr::from_ptr(p.cast()) }.to_bytes()).unwrap()
    }
    #[test]
    fn metadata_rejects_foreign_version_and_plan() {
        let signed = serde_json::json!({"name":"Blue Edition","version":"1.0.0","planSha256":"a"});
        assert!(validate_metadata(
            &signed,
            &serde_json::json!({"name":"Blue Edition","version":"2.0.0"}),
            "a"
        )
        .is_err());
        assert!(validate_metadata(&signed, &signed, "b").is_err());
        assert!(validate_metadata(&serde_json::json!({}), &serde_json::json!({}), "a").is_err());
    }
    #[test]
    fn signed_generation_metadata_and_mf_execution() {
        let app = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf();
        let storage = app.join(".exact/validation");
        let _ = fs::remove_dir_all(&storage);
        fs::create_dir_all(&storage).unwrap();
        ROOT.set(storage.to_str().unwrap().into()).unwrap();
        RESOURCES
            .set(app.join("resources").to_str().unwrap().into())
            .unwrap();
        for seq in [1, 2] {
            fs::write(
                storage.join(format!("origin-{seq}")),
                format!("https://fixture.invalid/v{seq}"),
            )
            .unwrap();
        }
        let a = call(1);
        assert!(a.get("error").is_none(), "{a}");
        assert_eq!(a["release"]["version"], "1.0.0");
        assert_eq!(a["release"]["name"], "Blue Edition");
        assert_eq!(a["release"]["planSha256"], a["planDigest"]);
        assert_eq!(a["entry"].as_str().unwrap().len(), 64);
        assert_eq!(a["seq"], 1);
        assert!(joined_finish(a["token"].as_u64().unwrap(), 1));
        assert_eq!(call(1)["current"], true);
        let b = call(2);
        assert!(b.get("error").is_none(), "{b}");
        assert_eq!(b["release"]["version"], "2.0.0");
        assert_eq!(b["release"]["name"], "Open House");
        assert_eq!(b["seq"], 2);
        assert_ne!(a["entry"], b["entry"]);
        assert_ne!(a["planDigest"], b["planDigest"]);
        assert!(joined_finish(b["token"].as_u64().unwrap(), 1));
        for seq in [1, 2, 1, 2] {
            let value = call(seq);
            assert!(value.get("error").is_none(), "{value}");
            assert_eq!(value["seq"], seq);
            assert!(joined_finish(value["token"].as_u64().unwrap(), 1));
        }
        assert_eq!(call(2)["current"], true);
    }
}
