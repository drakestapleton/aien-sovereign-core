use spark_max_cabi::SparkMaxBindings;
use std::fmt;

#[derive(Debug)]
pub enum SparkMaxError {
    LoadError(String),
    SessionInitFailed(i32),
    InferenceFailed(String),
}

impl fmt::Display for SparkMaxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SparkMaxError::LoadError(e) => write!(f, "Modular MAX load error: {}", e),
            SparkMaxError::SessionInitFailed(code) => write!(f, "Failed to create Modular MAX session: code {}", code),
            SparkMaxError::InferenceFailed(e) => write!(f, "Modular MAX inference failed: {}", e),
        }
    }
}

impl std::error::Error for SparkMaxError {}

pub struct SparkMaxSession {
    session_id: i32,
    bindings: &'static SparkMaxBindings,
}

impl SparkMaxSession {
    pub fn new(device_id: i32) -> Result<Self, SparkMaxError> {
        let bindings = SparkMaxBindings::global().map_err(SparkMaxError::LoadError)?;
        let session_id = unsafe { (bindings.session_create)(device_id) };
        if session_id <= 0 {
            return Err(SparkMaxError::SessionInitFailed(session_id));
        }
        Ok(Self { session_id, bindings })
    }

    pub fn version(&self) -> i32 {
        unsafe { (self.bindings.version)() }
    }

    pub fn session_id(&self) -> i32 {
        self.session_id
    }

    pub fn compute_scalar(&self, input_val: f32) -> f32 {
        unsafe { (self.bindings.compute_scalar)(self.session_id, input_val) }
    }
}

impl Drop for SparkMaxSession {
    fn drop(&mut self) {
        unsafe {
            let _ = (self.bindings.session_destroy)(self.session_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spark_max_safe_session() {
        let session = match SparkMaxSession::new(0) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Notice: SparkMaxSession::new failed ({}), skipping test", e);
                return;
            }
        };
        assert_eq!(session.version(), 1);
        assert!(session.session_id() > 1000);

        let out = session.compute_scalar(4.0);
        let expected = 4.0 * 2.5 + session.session_id() as f32;
        assert_eq!(out, expected);
    }
}
