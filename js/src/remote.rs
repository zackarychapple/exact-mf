//! Isolated bytecode admitted from a host-selected update generation.

use crate::Module;
use exact_runner::{DataSource, Value};
use exact_update::{AssetSet, Generation};

/// A host-owned remote. Preparation pins signed bytes without executing JavaScript.
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
        self.module.as_mut().ok_or("remote is disposed")?.bind(plan);
        Ok(())
    }

    /// The signed update generation that supplied these bytes.
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
