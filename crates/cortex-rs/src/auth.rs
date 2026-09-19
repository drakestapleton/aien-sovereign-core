use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::fs;
use std::path::Path;

const TOKEN_PATH: &str = "/home/drakestapleton/.config/cortex/token";

pub fn load_cortex_token() -> String {
    if let Ok(env_tok) = std::env::var("CORTEX_TOKEN") {
        if !env_tok.trim().is_empty() {
            return env_tok.trim().to_string();
        }
    }
    let p = Path::new(TOKEN_PATH);
    if p.exists() {
        fs::read_to_string(p).unwrap_or_default().trim().to_string()
    } else {
        String::new()
    }
}

pub fn validate_auth_header(auth_header: Option<&str>, expected_token: &str) -> Result<(), StatusCode> {
    if expected_token.is_empty() {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }
    match auth_header {
        Some(header) if header.starts_with("Bearer ") => {
            let token = header.strip_prefix("Bearer ").unwrap().trim();
            if token == expected_token {
                Ok(())
            } else {
                Err(StatusCode::UNAUTHORIZED)
            }
        }
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

pub async fn auth_middleware(req: Request, next: Next) -> Result<Response, Response> {
    let expected = load_cortex_token();
    let auth_header = req.headers().get("Authorization").and_then(|h| h.to_str().ok());

    match validate_auth_header(auth_header, &expected) {
        Ok(()) => Ok(next.run(req).await),
        Err(StatusCode::SERVICE_UNAVAILABLE) => {
            let err_resp = (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error": "Service unavailable: Cortex authentication token unconfigured on host"})),
            ).into_response();
            Err(err_resp)
        }
        Err(_) => {
            let err_resp = (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Unauthorized: invalid or missing Cortex bearer token"})),
            ).into_response();
            Err(err_resp)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_cortex_token_env_override() {
        std::env::set_var("CORTEX_TOKEN", "test_token_12345");
        let token = load_cortex_token();
        assert_eq!(token, "test_token_12345");
        std::env::remove_var("CORTEX_TOKEN");
    }

    #[test]
    fn test_validate_auth_header_success() {
        let expected = "secret_vault_token_42";
        let res = validate_auth_header(Some("Bearer secret_vault_token_42"), expected);
        assert_eq!(res, Ok(()));
    }

    #[test]
    fn test_validate_auth_header_unauthenticated() {
        let expected = "secret_vault_token_42";
        let res_none = validate_auth_header(None, expected);
        assert_eq!(res_none, Err(StatusCode::UNAUTHORIZED));

        let res_empty = validate_auth_header(Some(""), expected);
        assert_eq!(res_empty, Err(StatusCode::UNAUTHORIZED));

        let res_no_bearer = validate_auth_header(Some("Basic 12345"), expected);
        assert_eq!(res_no_bearer, Err(StatusCode::UNAUTHORIZED));
    }

    #[test]
    fn test_validate_auth_header_invalid_token() {
        let expected = "secret_vault_token_42";
        let res = validate_auth_header(Some("Bearer wrong_secret"), expected);
        assert_eq!(res, Err(StatusCode::UNAUTHORIZED));
    }

    #[test]
    fn test_validate_auth_header_unconfigured_host() {
        let res = validate_auth_header(Some("Bearer any_token"), "");
        assert_eq!(res, Err(StatusCode::SERVICE_UNAVAILABLE));
    }
}
