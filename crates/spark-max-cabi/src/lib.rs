use libloading::{Library, Symbol};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub const DEFAULT_SO_PATH: &str =
    "/home/drakestapleton/workspace/aien-sovereign-core/crates/spark-max-cabi/mojo/libspark_max.so";

pub fn default_so_path() -> PathBuf {
    if let Ok(p) = std::env::var("SPARK_MAX_SO_PATH") {
        return PathBuf::from(p);
    }
    if let Some(p) = option_env!("SPARK_MAX_SO_BUILT") {
        let pb = PathBuf::from(p);
        if pb.exists() {
            return pb;
        }
    }
    let local = PathBuf::from("crates/spark-max-cabi/mojo/libspark_max.so");
    if local.exists() {
        return local;
    }
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        let candidate = PathBuf::from(manifest).join("mojo/libspark_max.so");
        if candidate.exists() {
            return candidate;
        }
    }
    PathBuf::from(DEFAULT_SO_PATH)
}

type FnVersion = unsafe extern "C" fn() -> i32;
type FnSessionCreate = unsafe extern "C" fn(i32) -> i32;
type FnSessionDestroy = unsafe extern "C" fn(i32) -> i32;
type FnComputeScalar = unsafe extern "C" fn(i32, f32) -> f32;

pub struct SparkMaxBindings {
    _lib: Library,
    pub version: FnVersion,
    pub session_create: FnSessionCreate,
    pub session_destroy: FnSessionDestroy,
    pub compute_scalar: FnComputeScalar,
}

static BINDINGS: OnceLock<Result<SparkMaxBindings, String>> = OnceLock::new();

impl SparkMaxBindings {
    pub fn load_from<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let path_ref = path.as_ref();
        if !path_ref.exists() {
            return Err(format!("Library does not exist at {:?}", path_ref));
        }

        unsafe {
            let lib = Library::new(path_ref).map_err(|e| format!("Failed to dlopen {:?}: {}", path_ref, e))?;

            let version: Symbol<FnVersion> = lib
                .get(b"spark_max_version\0")
                .map_err(|e| format!("Symbol spark_max_version not found: {}", e))?;
            let session_create: Symbol<FnSessionCreate> = lib
                .get(b"spark_max_session_create\0")
                .map_err(|e| format!("Symbol spark_max_session_create not found: {}", e))?;
            let session_destroy: Symbol<FnSessionDestroy> = lib
                .get(b"spark_max_session_destroy\0")
                .map_err(|e| format!("Symbol spark_max_session_destroy not found: {}", e))?;
            let compute_scalar: Symbol<FnComputeScalar> = lib
                .get(b"spark_max_compute_scalar\0")
                .map_err(|e| format!("Symbol spark_max_compute_scalar not found: {}", e))?;

            Ok(Self {
                version: *version,
                session_create: *session_create,
                session_destroy: *session_destroy,
                compute_scalar: *compute_scalar,
                _lib: lib,
            })
        }
    }

    pub fn global() -> Result<&'static Self, String> {
        let res = BINDINGS.get_or_init(|| {
            let path = default_so_path();
            Self::load_from(path)
        });

        match res {
            Ok(ref b) => Ok(b),
            Err(ref e) => Err(e.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_and_execute_mojo_c_abi() {
        let path = default_so_path();
        if !path.exists() {
            eprintln!("Notice: libspark_max.so not found at {:?}, skipping", path);
            return;
        }

        let bindings = match SparkMaxBindings::load_from(&path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Notice: dlopen failed for {:?}: {}, skipping", path, e);
                return;
            }
        };

        let ver = unsafe { (bindings.version)() };
        assert_eq!(ver, 1);

        let session_id = unsafe { (bindings.session_create)(0) };
        assert!(session_id > 1000);

        let res = unsafe { (bindings.compute_scalar)(session_id, 10.0) };
        let expected = 10.0 * 2.5 + session_id as f32;
        assert_eq!(res, expected);

        let destroyed = unsafe { (bindings.session_destroy)(session_id) };
        assert_eq!(destroyed, 0);
    }
}
