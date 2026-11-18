use std::fmt;

use argon2::{Algorithm, Argon2, Params, PasswordHash, PasswordHasher, PasswordVerifier, Version};
use hmac::{Hmac, Mac};
use password_hash::{SaltString, rand_core::OsRng};
use sha2::Sha256;
use surrealdb::Surreal;
use surrealdb::engine::any::{Any, connect};

pub mod fonts;
pub mod throttle;

pub use fonts::{CatalogFont, FontCatalog};
pub use throttle::{Admission, LoginThrottle};

pub const MIN_SECRET_KEY_BYTES: usize = 32;
pub const MAX_CREDENTIAL_BYTES: usize = 1024;
pub const MAX_CONCURRENT_VERIFICATIONS: usize = 2;

const DB_ENDPOINT: &str = "mem://";
const PEPPER_LABEL: &[u8] = b"garden-engine/admin/password-pepper";
const USERNAME_LABEL: &[u8] = b"garden-engine/admin/username";
const SESSION_LABEL: &[u8] = b"garden-engine/admin/session-cookie";
const SCHEMA: &str = "
    DEFINE TABLE admin SCHEMAFULL PERMISSIONS NONE;
    DEFINE FIELD username_digest ON admin TYPE string;
    DEFINE FIELD password_hash ON admin TYPE string;
    DEFINE INDEX admin_username ON admin FIELDS username_digest UNIQUE;
";

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone)]
pub struct AdminCredentials {
    pub username: String,
    pub password: String,
    pub secret_key: String,
}

impl fmt::Debug for AdminCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AdminCredentials")
            .field("username", &self.username)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub enum AdminError {
    WeakSecretKey,
    InvalidCredentials,
    Hashing(String),
    Database(String),
}

impl fmt::Display for AdminError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AdminError::WeakSecretKey => write!(
                f,
                "ADMIN_SECRET_KEY must be at least {MIN_SECRET_KEY_BYTES} bytes long"
            ),
            AdminError::InvalidCredentials => write!(
                f,
                "ADMIN_USERNAME and ADMIN_PASSWORD must be non-empty and at most {MAX_CREDENTIAL_BYTES} bytes"
            ),
            AdminError::Hashing(e) => write!(f, "could not hash the admin password: {e}"),
            AdminError::Database(e) => write!(f, "admin database error: {e}"),
        }
    }
}

impl std::error::Error for AdminError {}

impl From<surrealdb::Error> for AdminError {
    fn from(e: surrealdb::Error) -> Self {
        AdminError::Database(e.to_string())
    }
}

pub struct AdminStore {
    db: Surreal<Any>,
    pepper: [u8; 32],
    username_key: [u8; 32],
    session_key_material: [u8; 32],
    dummy_hash: String,
    verifications: tokio::sync::Semaphore,
}

impl AdminStore {
    pub async fn new(credentials: AdminCredentials) -> Result<AdminStore, AdminError> {
        let secret = credentials.secret_key.as_bytes();
        if secret.len() < MIN_SECRET_KEY_BYTES {
            return Err(AdminError::WeakSecretKey);
        }
        if !is_acceptable_credential(&credentials.username)
            || !is_acceptable_credential(&credentials.password)
        {
            return Err(AdminError::InvalidCredentials);
        }

        let pepper = derive_key(secret, PEPPER_LABEL);
        let password_hash = hash_password(pepper, &credentials.password)?;
        let dummy_hash = hash_password(pepper, &hex::encode(derive_key(secret, b"dummy")))?;

        let db = connect(DB_ENDPOINT).await?;
        db.use_ns("garden_engine").use_db("admin").await?;
        db.query(SCHEMA).await?.check()?;

        let store = AdminStore {
            db,
            pepper,
            username_key: derive_key(secret, USERNAME_LABEL),
            session_key_material: derive_key(secret, SESSION_LABEL),
            dummy_hash,
            verifications: tokio::sync::Semaphore::new(MAX_CONCURRENT_VERIFICATIONS),
        };

        store
            .db
            .query("CREATE admin SET username_digest = $username, password_hash = $hash")
            .bind(("username", store.username_digest(&credentials.username)))
            .bind(("hash", password_hash))
            .await?
            .check()?;

        Ok(store)
    }

    pub fn session_key_material(&self) -> &[u8] {
        &self.session_key_material
    }

    pub async fn verify(&self, username: &str, password: &str) -> bool {
        if !is_acceptable_credential(username) || !is_acceptable_credential(password) {
            return false;
        }

        let stored = self.stored_hash(username).await;
        let known_user = stored.is_some();
        let hash = stored.unwrap_or_else(|| self.dummy_hash.clone());
        let pepper = self.pepper;
        let password = password.to_string();
        let Ok(_permit) = self.verifications.acquire().await else {
            return false;
        };

        let matches =
            tokio::task::spawn_blocking(move || verify_password(pepper, &hash, &password))
                .await
                .unwrap_or(false);

        known_user && matches
    }

    async fn stored_hash(&self, username: &str) -> Option<String> {
        self.db
            .query("SELECT VALUE password_hash FROM admin WHERE username_digest = $username")
            .bind(("username", self.username_digest(username)))
            .await
            .and_then(|mut response| response.take::<Vec<String>>(0))
            .ok()
            .and_then(|hashes| hashes.into_iter().next())
    }

    fn username_digest(&self, username: &str) -> String {
        hex::encode(derive_key(&self.username_key, username.as_bytes()))
    }

    #[cfg(test)]
    async fn raw_records(&self) -> Vec<(String, String)> {
        self.db
            .query("SELECT VALUE [username_digest, password_hash] FROM admin")
            .await
            .unwrap()
            .take::<Vec<Vec<String>>>(0)
            .unwrap()
            .into_iter()
            .map(|pair| (pair[0].clone(), pair[1].clone()))
            .collect()
    }
}

fn is_acceptable_credential(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_CREDENTIAL_BYTES
}

fn derive_key(secret: &[u8], label: &[u8]) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC accepts keys of any size");
    mac.update(label);
    mac.finalize().into_bytes().into()
}

fn argon2(pepper: &[u8; 32]) -> Argon2<'_> {
    Argon2::new_with_secret(
        pepper,
        Algorithm::Argon2id,
        Version::V0x13,
        Params::default(),
    )
    .expect("a 32 byte Argon2 secret is valid")
}

fn hash_password(pepper: [u8; 32], password: &str) -> Result<String, AdminError> {
    let salt = SaltString::generate(&mut OsRng);
    argon2(&pepper)
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| AdminError::Hashing(e.to_string()))
}

fn verify_password(pepper: [u8; 32], hash: &str, password: &str) -> bool {
    PasswordHash::new(hash)
        .map(|parsed| {
            argon2(&pepper)
                .verify_password(password.as_bytes(), &parsed)
                .is_ok()
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "0123456789abcdef0123456789abcdef";

    fn credentials() -> AdminCredentials {
        AdminCredentials {
            username: "gus".to_string(),
            password: "correct horse battery staple".to_string(),
            secret_key: SECRET.to_string(),
        }
    }

    #[tokio::test]
    async fn the_right_credentials_are_accepted() {
        let store = AdminStore::new(credentials()).await.unwrap();

        assert!(store.verify("gus", "correct horse battery staple").await);
    }

    #[tokio::test]
    async fn wrong_usernames_and_passwords_are_rejected() {
        let store = AdminStore::new(credentials()).await.unwrap();

        assert!(!store.verify("gus", "correct horse battery").await);
        assert!(!store.verify("Gus", "correct horse battery staple").await);
        assert!(
            !store
                .verify("someone", "correct horse battery staple")
                .await
        );
        assert!(!store.verify("", "").await);
        assert!(
            !store
                .verify("gus", &"a".repeat(MAX_CREDENTIAL_BYTES + 1))
                .await
        );
    }

    #[tokio::test]
    async fn the_database_never_holds_plaintext_credentials() {
        let store = AdminStore::new(credentials()).await.unwrap();

        let records = store.raw_records().await;
        assert_eq!(records.len(), 1);
        let (username_digest, password_hash) = &records[0];
        assert_eq!(username_digest.len(), 64);
        assert!(!username_digest.contains("gus"));
        assert!(password_hash.starts_with("$argon2id$v=19$"));
        assert!(!password_hash.contains("correct horse"));
    }

    #[tokio::test]
    async fn hashes_depend_on_the_secret_key() {
        let store = AdminStore::new(credentials()).await.unwrap();
        let (_, hash) = &store.raw_records().await[0];

        let other_pepper = derive_key(b"another secret key that is long enough", PEPPER_LABEL);
        assert!(verify_password(
            store.pepper,
            hash,
            "correct horse battery staple"
        ));
        assert!(!verify_password(
            other_pepper,
            hash,
            "correct horse battery staple"
        ));
    }

    #[tokio::test]
    async fn many_parallel_checks_still_give_the_right_answers() {
        let store = std::sync::Arc::new(AdminStore::new(credentials()).await.unwrap());

        let checks: Vec<_> = (0..8)
            .map(|attempt| {
                let store = store.clone();
                tokio::spawn(async move {
                    let password = if attempt == 0 {
                        "correct horse battery staple"
                    } else {
                        "guess"
                    };
                    store.verify("gus", password).await
                })
            })
            .collect();

        let mut results = Vec::new();
        for check in checks {
            results.push(check.await.unwrap());
        }
        assert_eq!(results.iter().filter(|ok| **ok).count(), 1);
        assert!(results[0]);
        assert_eq!(
            store.verifications.available_permits(),
            MAX_CONCURRENT_VERIFICATIONS
        );
    }

    #[tokio::test]
    async fn short_secret_keys_are_refused() {
        let error = AdminStore::new(AdminCredentials {
            secret_key: "too short".to_string(),
            ..credentials()
        })
        .await
        .err()
        .unwrap();

        assert!(matches!(error, AdminError::WeakSecretKey));
        assert!(error.to_string().contains("32 bytes"));
    }

    #[tokio::test]
    async fn empty_credentials_are_refused() {
        for credentials in [
            AdminCredentials {
                username: String::new(),
                ..credentials()
            },
            AdminCredentials {
                password: String::new(),
                ..credentials()
            },
        ] {
            assert!(matches!(
                AdminStore::new(credentials).await,
                Err(AdminError::InvalidCredentials)
            ));
        }
    }

    #[test]
    fn derived_keys_are_separated_by_purpose() {
        let secret = SECRET.as_bytes();

        assert_ne!(
            derive_key(secret, PEPPER_LABEL),
            derive_key(secret, USERNAME_LABEL)
        );
        assert_ne!(
            derive_key(secret, PEPPER_LABEL),
            derive_key(secret, SESSION_LABEL)
        );
        assert_eq!(
            derive_key(secret, PEPPER_LABEL),
            derive_key(secret, PEPPER_LABEL)
        );
    }

    #[test]
    fn credentials_never_print_their_secrets() {
        let printed = format!("{:?}", credentials());

        assert!(printed.contains("gus"));
        assert!(!printed.contains("correct horse"));
        assert!(!printed.contains(SECRET));
    }

    #[test]
    fn malformed_hashes_never_verify() {
        assert!(!verify_password([0; 32], "not a hash", "password"));
    }
}
