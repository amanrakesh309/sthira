use crate::store::{ct_eq_str, sha256_hex};
use async_trait::async_trait;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use reqwest::Client;
use ring::signature::{RsaPublicKeyComponents, RSA_PKCS1_2048_8192_SHA256};
use serde::Deserialize;
use std::{collections::HashMap, fmt, time::Duration};
use thiserror::Error;
use tokio::sync::RwLock;

pub const GOOGLE_JWKS_URL: &str = "https://www.googleapis.com/oauth2/v3/certs";
pub const FIREBASE_SIGN_IN_URL: &str =
    "https://identitytoolkit.googleapis.com/v1/accounts:signInWithIdp";
pub const FIREBASE_LOOKUP_URL: &str = "https://identitytoolkit.googleapis.com/v1/accounts:lookup";

const EXPIRY_LEEWAY_MS: i64 = 30_000;

#[derive(Clone)]
pub struct VerifiedIdentity {
    pub uid: String,
    pub email: String,
    pub display_name: String,
    pub raw_nonce: String,
}

impl fmt::Debug for VerifiedIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VerifiedIdentity")
            .field("uid", &self.uid)
            .field("email", &"[redacted]")
            .field("display_name", &self.display_name)
            .field("raw_nonce", &"[redacted]")
            .finish()
    }
}

#[derive(Debug, Error)]
pub enum VerifyError {
    #[error("The Google sign-in proof is malformed.")]
    InvalidProofFormat,
    #[error("The Google sign-in proof signature or key could not be verified.")]
    SignatureRejected,
    #[error("The Google sign-in proof did not match the expected OAuth client.")]
    AudienceMismatch,
    #[error("The Google sign-in proof has expired.")]
    ProofExpired,
    #[error("The Google sign-in proof did not match the server challenge.")]
    ChallengeMismatch,
    #[error("Google did not return a verified email account.")]
    UnverifiedGoogleAccount,
    #[error("Firebase rejected the authenticated Google account.")]
    FirebaseRejected,
    #[error("Could not reach Google or Firebase identity verification.")]
    UpstreamUnavailable,
}

#[derive(Clone, Debug)]
pub enum NonceExpectation {
    Exact(String),
    Sha256Hex(String),
}

#[async_trait]
pub trait IdentityVerifier: Send + Sync {
    async fn verify_enrollment(
        &self,
        google_id_token: &str,
        expected_nonce: Option<NonceExpectation>,
        now_ms: i64,
    ) -> Result<VerifiedIdentity, VerifyError>;
}

#[derive(Clone)]
struct CachedJwk {
    n: Vec<u8>,
    e: Vec<u8>,
}

pub struct GoogleFirebaseVerifier {
    google_client_id: String,
    firebase_api_key: String,
    jwks_url: String,
    firebase_sign_in_url: String,
    firebase_lookup_url: String,
    http: Client,
    jwks_cache: RwLock<HashMap<String, CachedJwk>>,
}

#[derive(Deserialize)]
struct JwtHeader {
    alg: String,
    #[serde(default)]
    kid: String,
}

#[derive(Deserialize)]
struct GoogleIdClaims {
    iss: String,
    aud: String,
    #[serde(default)]
    sub: String,
    #[serde(default)]
    email: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    email_verified: bool,
    #[serde(default)]
    nonce: String,
    exp: i64,
}

#[derive(Deserialize)]
struct JwksDocument {
    keys: Vec<JwkEntry>,
}

#[derive(Deserialize)]
struct JwkEntry {
    kid: String,
    kty: String,
    #[serde(default)]
    alg: Option<String>,
    #[serde(default, rename = "use")]
    key_use: Option<String>,
    #[serde(default)]
    n: String,
    #[serde(default)]
    e: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FirebaseSignInResponse {
    local_id: String,
    email: String,
    #[serde(default)]
    email_verified: bool,
    id_token: String,
    provider_id: String,
}

#[derive(Deserialize)]
struct FirebaseLookupResponse {
    users: Vec<FirebaseLookupUser>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FirebaseLookupUser {
    local_id: String,
    email: Option<String>,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    email_verified: bool,
    #[serde(default)]
    disabled: bool,
}

impl GoogleFirebaseVerifier {
    pub fn new(google_client_id: String, firebase_api_key: String) -> Result<Self, String> {
        Self::with_endpoints(
            google_client_id,
            firebase_api_key,
            GOOGLE_JWKS_URL.to_owned(),
            FIREBASE_SIGN_IN_URL.to_owned(),
            FIREBASE_LOOKUP_URL.to_owned(),
        )
    }

    pub fn with_endpoints(
        google_client_id: String,
        firebase_api_key: String,
        jwks_url: String,
        firebase_sign_in_url: String,
        firebase_lookup_url: String,
    ) -> Result<Self, String> {
        let http = Client::builder()
            .timeout(Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| "Could not initialize identity verification client.".to_owned())?;
        Ok(Self {
            google_client_id,
            firebase_api_key,
            jwks_url,
            firebase_sign_in_url,
            firebase_lookup_url,
            http,
            jwks_cache: RwLock::new(HashMap::new()),
        })
    }

    async fn get_jwk(&self, kid: &str) -> Result<Option<CachedJwk>, VerifyError> {
        if let Some(found) = self.jwks_cache.read().await.get(kid).cloned() {
            return Ok(Some(found));
        }
        let doc: JwksDocument = self
            .http
            .get(&self.jwks_url)
            .send()
            .await
            .map_err(|_| VerifyError::UpstreamUnavailable)?
            .error_for_status()
            .map_err(|_| VerifyError::UpstreamUnavailable)?
            .json()
            .await
            .map_err(|_| VerifyError::UpstreamUnavailable)?;

        let mut next = HashMap::new();
        for key in doc.keys {
            if key.kty != "RSA"
                || key.kid.is_empty()
                || key.alg.as_deref().is_some_and(|a| a != "RS256")
                || key.key_use.as_deref().is_some_and(|u| u != "sig")
            {
                continue;
            }
            let Ok(n) = URL_SAFE_NO_PAD.decode(key.n.as_bytes()) else {
                continue;
            };
            let Ok(e) = URL_SAFE_NO_PAD.decode(key.e.as_bytes()) else {
                continue;
            };
            if !n.is_empty() && !e.is_empty() {
                next.insert(key.kid, CachedJwk { n, e });
            }
        }
        let result = next.get(kid).cloned();
        *self.jwks_cache.write().await = next;
        Ok(result)
    }

    async fn verify_google_jwt(
        &self,
        google_id_token: &str,
        expected_nonce: Option<&NonceExpectation>,
        now_ms: i64,
    ) -> Result<GoogleIdClaims, VerifyError> {
        let mut parts = google_id_token.split('.');
        let (Some(header_b64), Some(payload_b64), Some(sig_b64), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(VerifyError::InvalidProofFormat);
        };
        if header_b64.is_empty() || payload_b64.is_empty() || sig_b64.is_empty() {
            return Err(VerifyError::InvalidProofFormat);
        }

        let header_bytes = URL_SAFE_NO_PAD
            .decode(header_b64.as_bytes())
            .map_err(|_| VerifyError::InvalidProofFormat)?;
        let header: JwtHeader =
            serde_json::from_slice(&header_bytes).map_err(|_| VerifyError::InvalidProofFormat)?;
        if header.alg != "RS256" || header.kid.is_empty() {
            return Err(VerifyError::SignatureRejected);
        }

        let sig = URL_SAFE_NO_PAD
            .decode(sig_b64.as_bytes())
            .map_err(|_| VerifyError::InvalidProofFormat)?;
        let Some(jwk) = self.get_jwk(&header.kid).await? else {
            return Err(VerifyError::SignatureRejected);
        };
        let signed = format!("{header_b64}.{payload_b64}");
        RsaPublicKeyComponents {
            n: jwk.n.as_slice(),
            e: jwk.e.as_slice(),
        }
        .verify(&RSA_PKCS1_2048_8192_SHA256, signed.as_bytes(), &sig)
        .map_err(|_| VerifyError::SignatureRejected)?;

        let payload_bytes = URL_SAFE_NO_PAD
            .decode(payload_b64.as_bytes())
            .map_err(|_| VerifyError::InvalidProofFormat)?;
        let claims: GoogleIdClaims =
            serde_json::from_slice(&payload_bytes).map_err(|_| VerifyError::InvalidProofFormat)?;

        if !matches!(
            claims.iss.as_str(),
            "https://accounts.google.com" | "accounts.google.com"
        ) {
            return Err(VerifyError::SignatureRejected);
        }
        if claims.aud != self.google_client_id {
            return Err(VerifyError::AudienceMismatch);
        }
        if claims
            .exp
            .saturating_mul(1000)
            .saturating_add(EXPIRY_LEEWAY_MS)
            <= now_ms
        {
            return Err(VerifyError::ProofExpired);
        }
        if !claims.email_verified || claims.email.trim().is_empty() || claims.sub.is_empty() {
            return Err(VerifyError::UnverifiedGoogleAccount);
        }
        if claims.nonce.trim().is_empty() {
            return Err(VerifyError::ChallengeMismatch);
        }
        if let Some(expected) = expected_nonce {
            let matched = match expected {
                NonceExpectation::Exact(raw) => ct_eq_str(&claims.nonce, raw),
                NonceExpectation::Sha256Hex(digest) => {
                    ct_eq_str(&sha256_hex(&claims.nonce), digest)
                }
            };
            if !matched {
                return Err(VerifyError::ChallengeMismatch);
            }
        }
        Ok(claims)
    }
}

#[async_trait]
impl IdentityVerifier for GoogleFirebaseVerifier {
    async fn verify_enrollment(
        &self,
        google_id_token: &str,
        expected_nonce: Option<NonceExpectation>,
        now_ms: i64,
    ) -> Result<VerifiedIdentity, VerifyError> {
        let claims = self
            .verify_google_jwt(google_id_token, expected_nonce.as_ref(), now_ms)
            .await?;

        let post_body = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("id_token", google_id_token)
            .append_pair("providerId", "google.com")
            .finish();

        let sign_in_resp = self
            .http
            .post(&self.firebase_sign_in_url)
            .query(&[("key", self.firebase_api_key.as_str())])
            .json(&serde_json::json!({
                "requestUri": "http://localhost",
                "postBody": post_body,
                "returnSecureToken": true,
            }))
            .send()
            .await
            .map_err(|_| VerifyError::UpstreamUnavailable)?;
        if sign_in_resp.status().is_server_error() {
            return Err(VerifyError::UpstreamUnavailable);
        }
        if !sign_in_resp.status().is_success() {
            return Err(VerifyError::FirebaseRejected);
        }
        let fb: FirebaseSignInResponse = sign_in_resp
            .json()
            .await
            .map_err(|_| VerifyError::FirebaseRejected)?;
        if !fb.email_verified
            || fb.provider_id != "google.com"
            || fb.local_id.is_empty()
            || fb.id_token.is_empty()
            || !fb.email.eq_ignore_ascii_case(claims.email.trim())
        {
            return Err(VerifyError::FirebaseRejected);
        }

        // Confirm account status via Firebase accounts:lookup so disabled accounts fail cleanly.
        let lookup_resp = self
            .http
            .post(&self.firebase_lookup_url)
            .query(&[("key", self.firebase_api_key.as_str())])
            .json(&serde_json::json!({ "idToken": fb.id_token }))
            .send()
            .await
            .map_err(|_| VerifyError::UpstreamUnavailable)?;
        if lookup_resp.status().is_server_error() {
            return Err(VerifyError::UpstreamUnavailable);
        }
        if !lookup_resp.status().is_success() {
            return Err(VerifyError::FirebaseRejected);
        }
        let lookup: FirebaseLookupResponse = lookup_resp
            .json()
            .await
            .map_err(|_| VerifyError::FirebaseRejected)?;
        let Some(user) = lookup.users.into_iter().find(|u| u.local_id == fb.local_id) else {
            return Err(VerifyError::FirebaseRejected);
        };
        let Some(lookup_email) = user.email.filter(|e| {
            user.email_verified
                && !user.disabled
                && !e.trim().is_empty()
                && e.eq_ignore_ascii_case(claims.email.trim())
        }) else {
            return Err(VerifyError::FirebaseRejected);
        };

        let display_name = resolve_display_name(
            user.display_name.as_deref(),
            claims.name.as_deref(),
            &lookup_email,
        );

        Ok(VerifiedIdentity {
            uid: fb.local_id,
            email: lookup_email,
            display_name,
            raw_nonce: claims.nonce,
        })
    }
}

pub fn resolve_display_name(primary: Option<&str>, fallback: Option<&str>, email: &str) -> String {
    for candidate in [primary, fallback].into_iter().flatten() {
        let cleaned = candidate
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if !cleaned.is_empty() {
            return cleaned.chars().take(64).collect();
        }
    }
    let local = email.trim().split('@').next().unwrap_or("Member").trim();
    if local.is_empty() {
        "Member".to_owned()
    } else {
        local.chars().take(64).collect()
    }
}
