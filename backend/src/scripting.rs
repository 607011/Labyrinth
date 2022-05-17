use rlua;
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::{Arc, Mutex};
use warp::Filter;

pub struct ScriptEnv {
    pub lua: rlua::Lua,
}

impl ScriptEnv {
    #[inline]
    pub fn new() -> ScriptEnv {
        ScriptEnv {
            lua: rlua::Lua::new(),
        }
    }
}

pub type ScriptEnvMap = HashMap<String, ScriptEnv>;

pub fn with_script_env(
    env: Arc<Mutex<ScriptEnvMap>>,
) -> impl Filter<Extract = (Arc<Mutex<ScriptEnvMap>>,), Error = Infallible> + Clone {
    warp::any().map(move || env.clone())
}
