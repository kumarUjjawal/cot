//! Authentication system for Cot.
//!
//! This module provides the authentication system for Cot. It includes
//! traits for user objects and backends, as well as password hashing and
//! verification.
//!
//! For the default way to store users in the database, see the [`db`] module.

#[cfg(feature = "db")]
pub mod db;

use std::any::Any;
use std::borrow::Cow;
use std::sync::{Arc, Mutex, MutexGuard};

/// backwards compatible shim for form Password type.
use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use derive_more::with_trait::Debug;
#[cfg(test)]
use mockall::automock;
use password_auth::VerifyError;
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;
use thiserror::Error;

use crate::config::SecretKey;
#[cfg(feature = "db")]
use crate::db::{ColumnType, DatabaseField, DbValue, FromDbValue, SqlxValueRef, ToDbValue};
use crate::request::{Request, RequestExt};
use crate::session::Session;

/// An error that occurs during authentication.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AuthError {
    /// The password hash that is passed to [`PasswordHash::new`] is invalid.
    #[error("Password hash is invalid")]
    PasswordHashInvalid,
    /// An error occurred while accessing the session object.
    #[error("Error while accessing the session object")]
    SessionAccess(#[from] tower_sessions::session::Error),
    /// An error occurred while accessing the user object.
    #[error("Error while accessing the user object")]
    UserBackend(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
    /// The credentials type provided to [`AuthBackend::authenticate`] is not
    /// supported.
    #[error("Tried to authenticate with an unsupported credentials type")]
    CredentialsTypeNotSupported,
    /// The [`UserId`] type provided to [`AuthBackend::get_by_id`] is not
    /// supported.
    #[error("Tried to get a user by an unsupported user ID type")]
    UserIdTypeNotSupported,
}

impl AuthError {
    /// Creates a new [`AuthError::UserBackend`] error from a backend error.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::auth::AuthError;
    ///
    /// let error = AuthError::backend_error(std::io::Error::new(std::io::ErrorKind::Other, "test"));
    /// ```
    pub fn backend_error(error: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::UserBackend(Box::new(error))
    }
}

/// The result type for authentication operations.
pub type Result<T> = std::result::Result<T, AuthError>;

/// A user object that can be authenticated.
///
/// This trait is used to represent a user object that can be authenticated and
/// is a core of the authentication system. A `User` object is returned by
/// [`Auth::user()`] and is used to check if a user is authenticated and to
/// access user data. If there is no active user session, the `User` object
/// returned by [`Auth::user()`] is an [`AnonymousUser`] object.
///
/// A concrete instance of a `User` object is returned by a backend that
/// implements the [`AuthBackend`] trait. The default backend is the
/// [`DatabaseUserBackend`](db::DatabaseUserBackend), which stores user data in
/// the database using Cot ORM.
#[cfg_attr(test, automock)]
pub trait User {
    /// Returns the user's ID.
    ///
    /// The ID is used to identify the user in the database or other storage.
    /// Can also be `None` if the user is not authenticated.
    ///
    /// [`AnonymousUser`] always returns `None`.
    fn id(&self) -> Option<UserId> {
        None
    }

    /// Returns the user's username.
    ///
    /// The username can be `None` if the user is not authenticated.
    ///
    /// [`AnonymousUser`] always returns `None`.
    // mockall requires lifetimes to be specified here
    // (see related issue: https://github.com/asomers/mockall/issues/571)
    #[expect(clippy::elidable_lifetime_names)]
    fn username<'a>(&'a self) -> Option<Cow<'a, str>> {
        None
    }

    /// Returns whether the user is active.
    ///
    /// An active user is one that has been authenticated and is not banned or
    /// otherwise disabled. In other words, a user can be authenticated but
    /// unable to access the system.
    ///
    /// [`AnonymousUser`] always returns `false`.
    fn is_active(&self) -> bool {
        false
    }

    /// Returns whether the user is authenticated.
    ///
    /// An authenticated user is one that has been logged in and has an active
    /// session.
    ///
    /// [`AnonymousUser`] always returns `false`.
    fn is_authenticated(&self) -> bool {
        false
    }

    /// Returns the user's last login time.
    ///
    /// This is the time when the user last logged in to the system. Can be
    /// [`None`] if the user has never logged in.
    ///
    /// [`AnonymousUser`] always returns [`None`].
    fn last_login(&self) -> Option<DateTime<FixedOffset>> {
        None
    }

    /// Returns the user's join time.
    ///
    /// This is the time when the user joined the system. Can be [`None`] if the
    /// user hasn't been authenticated.
    ///
    /// [`AnonymousUser`] always returns [`None`].
    fn joined(&self) -> Option<DateTime<FixedOffset>> {
        None
    }

    /// Returns the user's session authentication hash.
    ///
    /// This used to verify that the session hash stored in the session
    /// object is valid. If the session hash is not valid, the user is
    /// logged out. For instance,
    /// [`DatabaseUser`](db::DatabaseUser) implements this method
    /// to generate a session hash using the user's password hash.
    /// Therefore, when a user changes their password, the session hash is
    /// also changed, and all their sessions are invalidated.
    ///
    /// The session auth hash should always be the same for the same secret key,
    /// unless something has changed in the user's data that should invalidate
    /// the session (e.g. password change). Moreover, if a user implementation
    /// returns [`Some`] session hash for some secret key A, it should also
    /// return [`Some`] session hash for any other secret key B.
    ///
    /// If this method returns `None`, the session hash is not checked.
    ///
    /// [`AnonymousUser`] always returns `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::borrow::Cow;
    ///
    /// use cot::auth::{SessionAuthHash, User, UserId};
    /// use cot::common_types::Password;
    /// use cot::config::SecretKey;
    /// use hmac::{Hmac, Mac};
    /// use sha2::Sha512;
    ///
    /// struct MyUser {
    ///     id: i64,
    ///     password: Password,
    /// }
    ///
    /// type SessionAuthHmac = Hmac<Sha512>;
    ///
    /// impl User for MyUser {
    ///     fn id(&self) -> Option<UserId> {
    ///         Some(UserId::Int(self.id))
    ///     }
    ///
    ///     fn username(&self) -> Option<Cow<'_, str>> {
    ///         Some(Cow::from(format!("user{}", self.id)))
    ///     }
    ///
    ///     fn is_active(&self) -> bool {
    ///         true
    ///     }
    ///
    ///     fn is_authenticated(&self) -> bool {
    ///         true
    ///     }
    ///
    ///     fn session_auth_hash(&self, secret_key: &SecretKey) -> Option<SessionAuthHash> {
    ///         // thanks to this, the session hash is invalidated when the user changes their password
    ///         // and the user is automatically logged out
    ///
    ///         let mut mac = SessionAuthHmac::new_from_slice(secret_key.as_bytes())
    ///             .expect("HMAC can take key of any size");
    ///         mac.update(self.password.as_str().as_bytes());
    ///         let hmac_data = mac.finalize().into_bytes();
    ///
    ///         Some(SessionAuthHash::new(&hmac_data))
    ///     }
    /// }
    /// ```
    #[expect(unused_variables)]
    fn session_auth_hash(&self, secret_key: &SecretKey) -> Option<SessionAuthHash> {
        None
    }
}

/// A user ID that uniquely identifies a user in a backend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum UserId {
    /// A user ID that is an integer.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::auth::UserId;
    ///
    /// let user_id = UserId::Int(42);
    /// ```
    Int(i64),
    /// A user ID that is a string.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::auth::UserId;
    ///
    /// let user_id = UserId::String("forty_two@exmaple.com".to_string());
    /// ```
    String(String),
}

impl UserId {
    /// Returns the user ID as an integer.
    ///
    /// Returns [`None`] if the user ID is not an integer.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::auth::UserId;
    ///
    /// let user_id = UserId::Int(42);
    /// assert_eq!(user_id.as_int(), Some(42));
    /// ```
    #[must_use]
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Self::Int(id) => Some(*id),
            Self::String(_) => None,
        }
    }

    /// Returns the user ID as a string.
    ///
    /// Returns [`None`] if the user ID is not a string.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::auth::UserId;
    ///
    /// let user_id = UserId::String("42".to_string());
    /// assert_eq!(user_id.as_string(), Some("42"));
    /// ```
    #[must_use]
    pub fn as_string(&self) -> Option<&str> {
        match self {
            Self::Int(_) => None,
            Self::String(id) => Some(id),
        }
    }
}

/// A helper wrapper over `Arc<dyn User>` to provide a `Debug` implementation.
#[repr(transparent)]
struct UserWrapper(Arc<dyn User + Send + Sync>);

impl Debug for UserWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Arc<dyn User + Send + Sync>")
            .field("id", &self.0.id())
            .field("username", &self.0.username())
            .field("is_active", &self.0.is_active())
            .field("is_authenticated", &self.0.is_authenticated())
            .field("last_login", &self.0.last_login())
            .field("joined", &self.0.joined())
            .finish()
    }
}

/// An anonymous, unauthenticated user.
///
/// This is used to represent a user that is not authenticated. It is returned
/// by the [`Auth::user()`] method when there is no active user session.
#[derive(Debug, Copy, Clone, Default)]
pub struct AnonymousUser;

impl PartialEq for AnonymousUser {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl User for AnonymousUser {}

/// A session authentication hash.
///
/// This is used to verify that the session hash stored in the session object is
/// valid. If the session hash is not valid, the user is logged out. More
/// details can be found in the [`User::session_auth_hash`] method.
///
/// # Security
///
/// The implementation of the [`PartialEq`] trait for this type is constant-time
/// to prevent timing attacks.
///
/// The implementation of the [`Debug`] trait for this type hides the session
/// auth hash value to prevent it from being leaked in logs or other debug
/// output.
///
/// # Examples
///
/// ```
/// use std::borrow::Cow;
///
/// use cot::auth::{SessionAuthHash, User, UserId};
/// use cot::common_types::Password;
/// use cot::config::SecretKey;
/// use hmac::{Hmac, Mac};
/// use sha2::Sha512;
///
/// struct MyUser {
///     id: i64,
///     password: Password,
/// }
///
/// type SessionAuthHmac = Hmac<Sha512>;
///
/// impl User for MyUser {
///     fn id(&self) -> Option<UserId> {
///         Some(UserId::Int(self.id))
///     }
///
///     fn username(&self) -> Option<Cow<'_, str>> {
///         Some(Cow::from(format!("user{}", self.id)))
///     }
///
///     fn is_active(&self) -> bool {
///         true
///     }
///
///     fn is_authenticated(&self) -> bool {
///         true
///     }
///
///     fn session_auth_hash(&self, secret_key: &SecretKey) -> Option<SessionAuthHash> {
///         // thanks to this, the session hash is invalidated when the user changes their password
///         // and the user is automatically logged out
///
///         let mut mac = SessionAuthHmac::new_from_slice(secret_key.as_bytes())
///             .expect("HMAC can take key of any size");
///         mac.update(self.password.as_str().as_bytes());
///         let hmac_data = mac.finalize().into_bytes();
///
///         Some(SessionAuthHash::new(&hmac_data))
///     }
/// }
/// ```
#[repr(transparent)]
#[derive(Clone)]
pub struct SessionAuthHash(Box<[u8]>);

impl SessionAuthHash {
    /// Creates a new session authentication hash object from a byte slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::auth::SessionAuthHash;
    ///
    /// let hash = SessionAuthHash::new(&[1, 2, 3, 4]);
    /// ```
    #[must_use]
    pub fn new(hash: &[u8]) -> Self {
        Self(Box::from(hash))
    }

    /// Returns the session authentication hash as a byte slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::auth::SessionAuthHash;
    ///
    /// let hash = SessionAuthHash::new(&[1, 2, 3, 4]);
    /// assert_eq!(hash.as_bytes(), &[1, 2, 3, 4]);
    /// ```
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Returns the session authentication hash as a byte slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::auth::SessionAuthHash;
    ///
    /// let hash = SessionAuthHash::new(&[1, 2, 3, 4]);
    /// assert_eq!(hash.into_bytes(), Box::from([1, 2, 3, 4]));
    /// ```
    #[must_use]
    pub fn into_bytes(self) -> Box<[u8]> {
        self.0
    }
}

impl From<&[u8]> for SessionAuthHash {
    fn from(hash: &[u8]) -> Self {
        Self::new(hash)
    }
}

impl PartialEq for SessionAuthHash {
    fn eq(&self, other: &Self) -> bool {
        self.0.ct_eq(&other.0).into()
    }
}

impl Eq for SessionAuthHash {}

impl Debug for SessionAuthHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("SessionAuthHash")
            .field(&"**********")
            .finish()
    }
}

/// A hashed password.
///
/// This is used to store a hashed user password in the database. The password
/// hash is created using the `password_auth` crate internally, so by default,
/// it generates the hash using the latest recommended algorithm.
///
/// # Security
///
/// The implementation of the [`Debug`] trait for this type hides the password
/// hash value to prevent it from being leaked in logs or other debug output.
///
/// There is no [`PartialEq`] implementation for this type, as it should never
/// be needed to compare password hashes directly. Instead, use the
/// [`verify`](Self::verify) method to verify a password against the hash.
#[repr(transparent)]
#[derive(Clone)]
pub struct PasswordHash(String);

impl PasswordHash {
    /// Creates a new password hash object from a string.
    ///
    /// Note that this method takes the hash directly. If you need to hash a
    /// password, use [`PasswordHash::from_password`] instead.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::auth::PasswordHash;
    /// use cot::common_types::Password;
    ///
    /// let hash = PasswordHash::from_password(&Password::new("password"));
    /// let stored_hash = hash.into_string();
    /// let hash = PasswordHash::new(stored_hash).unwrap();
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if the password hash is invalid.
    pub fn new<T: Into<String>>(hash: T) -> Result<Self> {
        let hash = hash.into();

        if hash.len() > MAX_PASSWORD_HASH_LENGTH as usize {
            return Err(AuthError::PasswordHashInvalid);
        }
        password_auth::is_hash_obsolete(&hash).map_err(|_| AuthError::PasswordHashInvalid)?;

        Ok(Self(hash))
    }

    /// Creates a new password hash from a password.
    ///
    /// The password is hashed using the latest recommended algorithm.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::auth::PasswordHash;
    /// use cot::common_types::Password;
    ///
    /// let hash = PasswordHash::from_password(&Password::new("password"));
    /// ```
    #[must_use]
    pub fn from_password(password: &crate::common_types::Password) -> Self {
        let hash = password_auth::generate_hash(password.as_str());

        if hash.len() > MAX_PASSWORD_HASH_LENGTH as usize {
            unreachable!("password hash should never exceed {MAX_PASSWORD_HASH_LENGTH} bytes");
        }
        Self(hash)
    }

    /// Verifies a password against the hash.
    ///
    /// * If the password is valid, returns [`PasswordVerificationResult::Ok`].
    /// * If the password is valid but the hash is obsolete, returns
    ///   [`PasswordVerificationResult::OkObsolete`] with the new hash
    ///   calculated with the currently preferred algorithm. The new hash should
    ///   be saved to the database.
    /// * If the password is invalid, returns
    ///   [`PasswordVerificationResult::Invalid`].
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::auth::{PasswordHash, PasswordVerificationResult};
    /// use cot::common_types::Password;
    ///
    /// let password = Password::new("password");
    /// let hash = PasswordHash::from_password(&password);
    ///
    /// match hash.verify(&password) {
    ///     PasswordVerificationResult::Ok => println!("Password is valid"),
    ///     PasswordVerificationResult::OkObsolete(new_hash) => println!(
    ///         "Password is valid, but the hash is obsolete. Save the new hash: {}",
    ///         new_hash.as_str()
    ///     ),
    ///     PasswordVerificationResult::Invalid => println!("Password is invalid"),
    /// }
    /// ```
    pub fn verify(&self, password: &crate::common_types::Password) -> PasswordVerificationResult {
        const VALID_ERROR_STR: &str = "password hash should always be valid if created with `PasswordHash::new` or `PasswordHash::from_password`";

        match password_auth::verify_password(password.as_str(), &self.0) {
            Ok(()) => {
                let Ok(is_obsolete) = password_auth::is_hash_obsolete(&self.0) else {
                    unreachable!("{VALID_ERROR_STR}");
                };
                if is_obsolete {
                    PasswordVerificationResult::OkObsolete(PasswordHash::from_password(password))
                } else {
                    PasswordVerificationResult::Ok
                }
            }
            Err(error) => match error {
                VerifyError::PasswordInvalid => PasswordVerificationResult::Invalid,
                VerifyError::Parse(_) => unreachable!("{VALID_ERROR_STR}"),
            },
        }
    }

    /// Returns the password hash as a string.
    ///
    /// For security reasons, you should avoid using this method as much as
    /// possible. Typically, you should use the [`PasswordHash::verify()`]
    /// method to verify a password against the hash. This method is mostly
    /// useful for persisting the password hash externally.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::auth::PasswordHash;
    /// use cot::common_types::Password;
    ///
    /// let hash = PasswordHash::from_password(&Password::new("password"));
    /// assert!(!hash.as_str().is_empty());
    /// ```
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the object and returns the password hash as a string.
    ///
    /// For security reasons, you should avoid using this method as much as
    /// possible. Typically, you should use the [`PasswordHash::verify()`]
    /// method to verify a password against the hash. This method is mostly
    /// useful for persisting the password hash externally.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::auth::PasswordHash;
    /// use cot::common_types::Password;
    ///
    /// let hash = PasswordHash::from_password(&Password::new("password"));
    /// assert!(!hash.into_string().is_empty());
    /// ```
    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl TryFrom<String> for PasswordHash {
    type Error = AuthError;

    fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// The result returned by [`PasswordHash::verify()`].
#[derive(Debug, Clone)]
#[must_use]
pub enum PasswordVerificationResult {
    /// The password is valid.
    Ok,
    /// The password is valid, but the hash is obsolete. The new hash calculated
    /// with the currently preferred algorithm is provided, and it should be
    /// saved to the database.
    OkObsolete(PasswordHash),
    /// The password is invalid.
    Invalid,
}

impl Debug for PasswordHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("PasswordHash")
            .field(&format!("{}**********", &self.0[..10]))
            .finish()
    }
}

const MAX_PASSWORD_HASH_LENGTH: u32 = 128;

#[cfg(feature = "db")]
impl DatabaseField for PasswordHash {
    const TYPE: ColumnType = ColumnType::String(MAX_PASSWORD_HASH_LENGTH);
}

#[cfg(feature = "db")]
impl FromDbValue for PasswordHash {
    #[cfg(feature = "sqlite")]
    fn from_sqlite(value: crate::db::impl_sqlite::SqliteValueRef<'_>) -> cot::db::Result<Self> {
        PasswordHash::new(value.get::<String>()?).map_err(cot::db::DatabaseError::value_decode)
    }

    #[cfg(feature = "postgres")]
    fn from_postgres(
        value: crate::db::impl_postgres::PostgresValueRef<'_>,
    ) -> cot::db::Result<Self> {
        PasswordHash::new(value.get::<String>()?).map_err(cot::db::DatabaseError::value_decode)
    }

    #[cfg(feature = "mysql")]
    fn from_mysql(value: crate::db::impl_mysql::MySqlValueRef<'_>) -> crate::db::Result<Self>
    where
        Self: Sized,
    {
        PasswordHash::new(value.get::<String>()?).map_err(cot::db::DatabaseError::value_decode)
    }
}

#[cfg(feature = "db")]
impl ToDbValue for PasswordHash {
    fn to_db_value(&self) -> DbValue {
        self.0.clone().into()
    }
}

/// Authentication helper structure.
///
/// This is an object that provides methods to sign users in and out, by using
/// the authentication backend defined in the project configuration. It is
/// constructed by [`AuthMiddleware`](crate::middleware::AuthMiddleware) and
/// included in every [`Request`] object as long as the middleware is active.
///
/// # Security
///
/// This is a wrapper around a reference-counted inner object, which contains
/// the actual user data. It's safe and cheap to clone it and authenticate the
/// user, as the change will be reflected in all clones.
#[derive(Debug, Clone)]
pub struct Auth {
    inner: Arc<AuthInner>,
}

impl Auth {
    // only currently used in TestRequestBuilder if database is enabled
    #[cfg(all(feature = "db", feature = "test"))]
    pub(crate) async fn new(
        session: Session,
        backend: Arc<dyn AuthBackend>,
        secret_key: SecretKey,
        fallback_secret_keys: &[SecretKey],
    ) -> cot::Result<Self> {
        let inner = AuthInner::new(session, backend, secret_key, fallback_secret_keys).await?;
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    pub(crate) async fn from_request(request: &mut Request) -> cot::Result<Self> {
        let inner = AuthInner::from_request(request).await?;
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    /// Returns the current user.
    ///
    /// This uses the auth backend configured in
    /// [`ProjectConfig::auth_backend`](crate::config::ProjectConfig::auth_backend).
    /// If the user is not authenticated, the [`AnonymousUser`] object is
    /// returned.
    #[must_use]
    pub fn user(&self) -> Arc<dyn User + Send + Sync> {
        self.inner.user()
    }

    /// Authenticates a user with the given credentials.
    ///
    /// This uses the auth backend configured in
    /// [`ProjectConfig::auth_backend`](crate::config::ProjectConfig::auth_backend).
    /// If the authentication is successful, the user object is returned. If the
    /// authentication fails, [`None`] is returned.
    ///
    /// Note that this doesn't log the user in, it only checks if the
    /// credentials are valid and returns the user object. To log the user
    /// in the current session, use the [`login`](Self::login) method.
    ///
    /// # Errors
    ///
    /// Returns an error if the [`AuthBackend`] accepts the credentials but
    /// fails to fetch the user object.
    pub async fn authenticate(
        &self,
        credentials: &(dyn Any + Send + Sync),
    ) -> Result<Option<Box<dyn User + Send + Sync>>> {
        self.inner.authenticate(credentials).await
    }

    /// Logs in a user.
    ///
    /// This logs in the user in the current session. The user object is stored
    /// in the session object and can be accessed using the [`user`](Self::user)
    /// method.
    ///
    /// # Errors
    ///
    /// Returns an error if the user object cannot be stored in the session
    /// object.
    pub async fn login(&self, user: Box<dyn User + Send + Sync + 'static>) -> Result<()> {
        self.inner.login(user).await
    }

    /// Logs out the current user.
    ///
    /// This removes the user object from the session object and logs the user
    /// out. Subsequent calls to [`user`](Self::user) will return the
    /// [`AnonymousUser`] object, unless a user is logged in again.
    ///
    /// # Errors
    ///
    /// Returns an error if the user object cannot be removed from the session
    /// object.
    pub async fn logout(&self) -> Result<()> {
        self.inner.logout().await
    }
}

#[derive(Debug)]
struct AuthInner {
    session: Session,
    #[debug("..")]
    backend: Arc<dyn AuthBackend>,
    secret_key: SecretKey,
    // Using a standard Mutex object instead of an async version - it's unlikely this will ever be
    // accessed from multiple threads, since it's tied to a single request. The mutex is used
    // mostly to allow the `Auth` object to be cloned and passed around while maintaining the
    // reference to the same `AuthInner` object with a mutable `user`.
    #[debug("..")]
    user: Mutex<UserWrapper>,
}

impl AuthInner {
    async fn new(
        session: Session,
        backend: Arc<dyn AuthBackend>,
        secret_key: SecretKey,
        fallback_secret_keys: &[SecretKey],
    ) -> cot::Result<Self> {
        #[expect(trivial_casts)] // cast to Arc<dyn User + Send + Sync>
        let user = get_user_with_saved_id(&session, &*backend, &secret_key, fallback_secret_keys)
            .await?
            .map_or_else(
                || Arc::new(AnonymousUser) as Arc<dyn User + Send + Sync>,
                Arc::from,
            );

        Ok(Self {
            session,
            backend,
            secret_key,
            user: Mutex::new(UserWrapper(user)),
        })
    }

    async fn from_request(request: &mut Request) -> cot::Result<Self> {
        let config = request.context().config();

        let session = Session::from_request(request).clone();
        let backend = request.context().auth_backend().clone();
        let secret_key = config.secret_key.clone();

        Self::new(session, backend, secret_key, &config.fallback_secret_keys).await
    }

    fn user(&self) -> Arc<dyn User + Send + Sync> {
        Arc::clone(&self.user_lock().0)
    }

    async fn authenticate(
        &self,
        credentials: &(dyn Any + Send + Sync),
    ) -> Result<Option<Box<dyn User + Send + Sync>>> {
        self.backend.authenticate(credentials).await
    }

    async fn login(&self, user: Box<dyn User + Send + Sync + 'static>) -> Result<()> {
        // Mitigate the session fixation attack by changing the session ID:
        // https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html#renew-the-session-id-after-any-privilege-level-change
        self.session.cycle_id().await?;

        if let Some(user_id) = user.id() {
            self.session.insert(USER_ID_SESSION_KEY, user_id).await?;
        }
        let secret_key = &self.secret_key;
        if let Some(session_auth_hash) = user.session_auth_hash(secret_key) {
            self.session
                .insert(SESSION_HASH_SESSION_KEY, session_auth_hash.as_bytes())
                .await?;
        }
        *self.user_lock() = UserWrapper(Arc::from(user));

        Ok(())
    }

    async fn logout(&self) -> Result<()> {
        self.session.flush().await?;
        *self.user_lock() = UserWrapper(Arc::new(AnonymousUser));

        Ok(())
    }

    fn user_lock(&self) -> MutexGuard<'_, UserWrapper> {
        self.user.lock().unwrap_or_else(|poison_error| {
            // We don't have any invariants about the structure of the UserWrapper object,
            // so we can safely clear the poison.
            self.user.clear_poison();
            poison_error.into_inner()
        })
    }
}

const USER_ID_SESSION_KEY: &str = "__cot_auth_user_id";
const SESSION_HASH_SESSION_KEY: &str = "__cot_auth_session_hash";

async fn get_user_with_saved_id(
    session: &Session,
    auth_backend: &dyn AuthBackend,
    secret_key: &SecretKey,
    fallback_secret_keys: &[SecretKey],
) -> Result<Option<Box<dyn User + Send + Sync>>> {
    let Some(user_id) = session.get::<UserId>(USER_ID_SESSION_KEY).await? else {
        return Ok(None);
    };

    let Some(user) = auth_backend.get_by_id(user_id).await? else {
        return Ok(None);
    };

    if session_auth_hash_valid(&*user, session, secret_key, fallback_secret_keys).await? {
        Ok(Some(user))
    } else {
        Ok(None)
    }
}

async fn session_auth_hash_valid(
    user: &(dyn User + Send + Sync),
    session: &Session,
    secret_key: &SecretKey,
    fallback_secret_keys: &[SecretKey],
) -> Result<bool> {
    let Some(user_hash) = user.session_auth_hash(secret_key) else {
        return Ok(true);
    };

    let stored_hash = session
        .get::<Vec<u8>>(SESSION_HASH_SESSION_KEY)
        .await?
        .expect("Session hash should be present in the session object");
    let stored_hash = SessionAuthHash::new(&stored_hash);

    if user_hash == stored_hash {
        return Ok(true);
    }

    // If the primary secret key doesn't match, try the fallback keys (in other
    // words, check if the session hash was generated with an old secret key)
    // and update the session hash with the new key if a match is found.
    for fallback_key in fallback_secret_keys {
        let user_hash_fallback = user
            .session_auth_hash(fallback_key)
            .expect("User should have a session hash for each secret key");
        if user_hash_fallback == stored_hash {
            session
                .insert(SESSION_HASH_SESSION_KEY, user_hash.as_bytes())
                .await?;

            return Ok(true);
        }
    }

    Ok(false)
}

/// An authentication backend.
#[async_trait]
pub trait AuthBackend: Send + Sync {
    /// Authenticates a user with the given credentials.
    ///
    /// This method returns a user object if the authentication is successful.
    /// If the authentication fails, it returns `None`.
    ///
    /// # Errors
    ///
    /// Returns an error if the user object cannot be fetched.
    ///
    /// Returns an error if the credentials type is not supported.
    async fn authenticate(
        &self,
        credentials: &(dyn Any + Send + Sync),
    ) -> Result<Option<Box<dyn User + Send + Sync>>>;

    /// Get a user by ID.
    ///
    /// This method returns a user object by its ID. If the user is not found,
    /// it should return `None`.
    ///
    /// # Errors
    ///
    /// Returns an error if the user object cannot be fetched.
    ///
    /// Returns an error if the user ID type is not supported.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::auth::UserId;
    /// use cot::request::{Request, RequestExt};
    ///
    /// async fn view_user_profile(request: &Request) {
    ///     let user = request
    ///         .context()
    ///         .auth_backend()
    ///         .get_by_id(UserId::Int(1))
    ///         .await;
    ///
    ///     match user {
    ///         Ok(Some(user)) => {
    ///             println!("User ID: {:?}", user.id());
    ///             println!("Username: {:?}", user.username());
    ///         }
    ///         Ok(None) => {
    ///             println!("User not found");
    ///         }
    ///         Err(error) => {
    ///             eprintln!("Error: {}", error);
    ///         }
    ///     }
    /// }
    /// ```
    async fn get_by_id(&self, id: UserId) -> Result<Option<Box<dyn User + Send + Sync>>>;
}

/// A no-op authentication backend.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct NoAuthBackend;

#[async_trait]
impl AuthBackend for NoAuthBackend {
    async fn authenticate(
        &self,
        _credentials: &(dyn Any + Send + Sync),
    ) -> Result<Option<Box<dyn User + Send + Sync>>> {
        Ok(None)
    }

    async fn get_by_id(&self, _id: UserId) -> Result<Option<Box<dyn User + Send + Sync>>> {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use mockall::predicate::eq;

    use super::*;
    use crate::common_types::Password;
    use crate::config::ProjectConfig;
    use crate::test::TestRequestBuilder;

    struct MockAuthBackend<F> {
        return_user: F,
    }

    #[async_trait]
    impl<F: Fn() -> MockUser + Send + Sync + 'static> AuthBackend for MockAuthBackend<F> {
        async fn authenticate(
            &self,
            _credentials: &(dyn Any + Send + Sync),
        ) -> Result<Option<Box<dyn User + Send + Sync>>> {
            Ok(Some(Box::new((self.return_user)())))
        }

        async fn get_by_id(&self, _id: UserId) -> Result<Option<Box<dyn User + Send + Sync>>> {
            Ok(Some(Box::new((self.return_user)())))
        }
    }

    const TEST_KEY_1: &[u8] = b"key1";
    const TEST_KEY_2: &[u8] = b"key2";
    const TEST_KEY_3: &[u8] = b"key3";

    fn test_request<T: Fn() -> MockUser + Send + Sync + 'static>(return_user: T) -> Request {
        test_request_with_auth_backend(MockAuthBackend { return_user })
    }

    fn test_request_with_auth_backend<T: AuthBackend + 'static>(auth_backend: T) -> Request {
        TestRequestBuilder::get("/")
            .with_session()
            .config(test_project_config(SecretKey::new(TEST_KEY_1), vec![]))
            .auth_backend(auth_backend)
            .build()
    }

    fn test_request_with_auth_config_and_session<T: AuthBackend + 'static>(
        auth_backend: T,
        config: ProjectConfig,
        session_source: &Request,
    ) -> Request {
        TestRequestBuilder::get("/")
            .with_session_from(session_source)
            .config(config)
            .auth_backend(auth_backend)
            .build()
    }

    fn test_project_config(secret_key: SecretKey, fallback_keys: Vec<SecretKey>) -> ProjectConfig {
        ProjectConfig::builder()
            .secret_key(secret_key)
            .fallback_secret_keys(fallback_keys)
            .clone()
            .build()
    }

    #[test]
    fn anonymous_user() {
        let anonymous_user = AnonymousUser;
        assert_eq!(anonymous_user.id(), None);
        assert_eq!(anonymous_user.username(), None);
        assert!(!anonymous_user.is_active());
        assert!(!anonymous_user.is_authenticated());
        assert_eq!(anonymous_user.last_login(), None);
        assert_eq!(anonymous_user.joined(), None);
        assert_eq!(
            anonymous_user.session_auth_hash(&SecretKey::new(b"key")),
            None
        );

        let anonymous_user2 = AnonymousUser;
        assert_eq!(anonymous_user, anonymous_user2);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn password_hash() {
        let password = Password::new("password".to_string());
        let hash = PasswordHash::from_password(&password);
        match hash.verify(&password) {
            PasswordVerificationResult::Ok => {}
            _ => panic!("Password hash verification failed"),
        }
    }

    #[test]
    fn session_auth_hash_debug() {
        let hash = SessionAuthHash::from([1, 2, 3].as_ref());
        assert_eq!(format!("{hash:?}"), "SessionAuthHash(\"**********\")");
    }

    const TEST_PASSWORD_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$QAAI3EMU1eTLT9NzzBhQjg$khq4zuHsEyk9trGjuqMBFYnTbpqkmn0wXGxFn1nkPBc";

    #[test]
    #[cfg_attr(miri, ignore)]
    fn password_hash_debug() {
        let hash = PasswordHash::new(TEST_PASSWORD_HASH).unwrap();
        assert_eq!(
            format!("{hash:?}"),
            "PasswordHash(\"$argon2id$**********\")"
        );
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn password_hash_verify() {
        let password = Password::new("password");
        let hash = PasswordHash::from_password(&password);
        match hash.verify(&password) {
            PasswordVerificationResult::Ok => {}
            _ => panic!("Password hash verification failed"),
        }

        let wrong_password = Password::new("wrongpassword");
        match hash.verify(&wrong_password) {
            PasswordVerificationResult::Invalid => {}
            _ => panic!("Password hash verification failed"),
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn password_hash_str() {
        let hash = PasswordHash::new(TEST_PASSWORD_HASH).unwrap();
        assert_eq!(hash.as_str(), TEST_PASSWORD_HASH);
        assert_eq!(hash.into_string(), TEST_PASSWORD_HASH);

        let hash = PasswordHash::try_from(TEST_PASSWORD_HASH.to_string()).unwrap();
        assert_eq!(hash.as_str(), TEST_PASSWORD_HASH);
        assert_eq!(hash.into_string(), TEST_PASSWORD_HASH);
    }

    #[cot::test]
    async fn user_anonymous() {
        let mut request = test_request_with_auth_backend(NoAuthBackend {});

        let auth = Auth::from_request(&mut request).await.unwrap();
        let user = auth.user();
        assert!(!user.is_authenticated());
        assert!(!user.is_active());
    }

    #[cot::test]
    async fn user() {
        let mut request = test_request(|| {
            let mut mock_user = MockUser::new();
            mock_user.expect_id().return_const(UserId::Int(1));
            mock_user.expect_session_auth_hash().return_const(None);
            mock_user
                .expect_username()
                .return_const(Some(Cow::from("mockuser")));
            mock_user
        });

        Session::from_request(&request)
            .insert(USER_ID_SESSION_KEY, UserId::Int(1))
            .await
            .unwrap();
        let auth = Auth::from_request(&mut request).await.unwrap();

        assert_eq!(auth.user().username(), Some(Cow::from("mockuser")));
    }

    #[cot::test]
    async fn authenticate() {
        let mut request = test_request(|| {
            let mut mock_user = MockUser::new();
            mock_user
                .expect_username()
                .return_const(Some(Cow::from("mockuser")));
            mock_user
        });
        let auth = Auth::from_request(&mut request).await.unwrap();

        let credentials: &(dyn Any + Send + Sync) = &();
        let user = auth.authenticate(credentials).await.unwrap().unwrap();
        assert_eq!(user.username(), Some(Cow::from("mockuser")));
    }

    #[cot::test]
    async fn login_logout() {
        let mut request = test_request(MockUser::new);
        let session = Session::from_request(&request).clone();
        let auth = Auth::from_request(&mut request).await.unwrap();

        let mut mock_user = MockUser::new();
        mock_user.expect_id().return_const(UserId::Int(1));
        mock_user.expect_session_auth_hash().return_const(None);
        mock_user
            .expect_username()
            .return_const(Some(Cow::from("mockuser")));

        auth.login(Box::new(mock_user)).await.unwrap();
        assert_eq!(auth.user().username(), Some(Cow::from("mockuser")));
        assert!(!session.is_empty().await);

        auth.logout().await.unwrap();
        assert!(auth.user().username().is_none());

        assert!(session.is_empty().await);
    }

    /// Test the session fixation attack mitigation
    #[cot::test]
    async fn login_cycle_id() {
        let mut request = test_request(MockUser::new);
        let session = Session::from_request(&request).clone();
        let auth = Auth::from_request(&mut request).await.unwrap();

        let mut mock_user = MockUser::new();
        mock_user.expect_id().return_const(UserId::Int(1));
        mock_user.expect_session_auth_hash().return_const(None);
        mock_user
            .expect_username()
            .return_const(Some(Cow::from("mockuser_1")));
        auth.login(Box::new(mock_user)).await.unwrap();

        session.save().await.unwrap();
        let id_1 = session.id();
        assert!(id_1.is_some());

        let mut mock_user = MockUser::new();
        mock_user.expect_id().return_const(UserId::Int(2));
        mock_user.expect_session_auth_hash().return_const(None);
        mock_user
            .expect_username()
            .return_const(Some(Cow::from("mockuser_2")));
        auth.login(Box::new(mock_user)).await.unwrap();

        session.save().await.unwrap();
        let id_2 = session.id();

        assert!(id_2.is_some());
        assert!(id_1 != id_2);
    }

    /// Test that the user is logged out when there is an invalid user ID in the
    /// session (can happen if the user is deleted from the database)
    #[cot::test]
    async fn logout_on_invalid_user_id_in_session() {
        let mut request = test_request_with_auth_backend(NoAuthBackend {});

        Session::from_request(&request)
            .insert(USER_ID_SESSION_KEY, UserId::Int(1))
            .await
            .unwrap();

        let auth = Auth::from_request(&mut request).await.unwrap();
        let user = auth.user();
        assert_eq!(user.username(), None);
        assert!(!user.is_authenticated());
    }

    #[cot::test]
    async fn logout_on_session_hash_change() {
        let session_auth_hash = Arc::new(Mutex::new(SessionAuthHash::new(&[1, 2, 3])));
        let session_auth_hash_clone = Arc::clone(&session_auth_hash);
        let create_user = move || {
            let session_auth_hash_clone = Arc::clone(&session_auth_hash_clone);
            let mut mock_user = MockUser::new();
            mock_user.expect_id().return_const(UserId::Int(1));
            mock_user
                .expect_session_auth_hash()
                .returning(move |_| Some(session_auth_hash_clone.lock().unwrap().clone()));
            mock_user
                .expect_username()
                .return_const(Some(Cow::from("mockuser")));
            mock_user
        };

        let mut request = test_request(create_user.clone());
        let auth = Auth::from_request(&mut request).await.unwrap();

        auth.login(Box::new(create_user())).await.unwrap();
        assert_eq!(auth.user().username(), Some(Cow::from("mockuser")));

        // Check the user can be retrieved again
        let auth = Auth::from_request(&mut request).await.unwrap();
        assert_eq!(auth.user().username(), Some(Cow::from("mockuser")));

        // Verify the user is logged out when the session hash changes
        *session_auth_hash.lock().unwrap() = SessionAuthHash::new(&[4, 5, 6]);
        let auth = Auth::from_request(&mut request).await.unwrap();
        let user = auth.user();
        assert!(!user.is_authenticated());
        assert_eq!(user.username(), None);
    }

    #[cot::test]
    async fn user_secret_key_change() {
        let create_user = move || {
            let mut mock_user = MockUser::new();
            mock_user.expect_id().return_const(UserId::Int(1));
            mock_user
                .expect_session_auth_hash()
                .with(eq(SecretKey::new(TEST_KEY_1)))
                .returning(move |_| Some(SessionAuthHash::new(&[1, 2, 3])));
            mock_user
                .expect_session_auth_hash()
                .with(eq(SecretKey::new(TEST_KEY_2)))
                .returning(move |_| Some(SessionAuthHash::new(&[4, 5, 6])));
            mock_user
                .expect_session_auth_hash()
                .with(eq(SecretKey::new(TEST_KEY_3)))
                .returning(move |_| Some(SessionAuthHash::new(&[7, 8, 9])));
            mock_user
                .expect_username()
                .return_const(Some(Cow::from("mockuser")));
            mock_user
        };

        let mut request = test_request(create_user);
        let auth = Auth::from_request(&mut request).await.unwrap();

        auth.login(Box::new(create_user())).await.unwrap();
        let user = auth.user();
        assert_eq!(user.username(), Some(Cow::from("mockuser")));

        let replace_keys = move |request: &mut Request, secret_key, fallback_keys| {
            let auth_backend = MockAuthBackend {
                return_user: create_user,
            };
            let new_config = test_project_config(secret_key, fallback_keys);
            *request = test_request_with_auth_config_and_session(auth_backend, new_config, request);
        };

        // Change the secret key and verify the user is still logged in with the
        // fallback key
        replace_keys(
            &mut request,
            SecretKey::new(TEST_KEY_2),
            vec![SecretKey::new(TEST_KEY_1)],
        );
        let auth = Auth::from_request(&mut request).await.unwrap();
        let user = auth.user();
        assert_eq!(user.username(), Some(Cow::from("mockuser")));

        // Remove fallback key and verify the user is still logged in
        replace_keys(&mut request, SecretKey::new(TEST_KEY_2), vec![]);
        let auth = Auth::from_request(&mut request).await.unwrap();
        let user = auth.user();
        assert_eq!(user.username(), Some(Cow::from("mockuser")));

        // Remove both keys and verify the user is logged out
        replace_keys(&mut request, SecretKey::new(TEST_KEY_3), vec![]);
        let auth = Auth::from_request(&mut request).await.unwrap();
        let user = auth.user();
        assert_eq!(user.username(), None);
        assert!(!user.is_authenticated());
    }

    #[test]
    fn user_wrapper_with_anonymous_user() {
        let anon_user = AnonymousUser;
        let user_wrapper = UserWrapper(Arc::new(anon_user));

        let debug_output = format!("{user_wrapper:?}");

        assert!(debug_output.contains("Arc<dyn User + Send + Sync>"));
        assert!(debug_output.contains("id: None"));
        assert!(debug_output.contains("username: None"));
        assert!(debug_output.contains("is_active: false"));
        assert!(debug_output.contains("is_authenticated: false"));
        assert!(debug_output.contains("last_login: None"));
        assert!(debug_output.contains("joined: None"));
    }

    #[test]
    fn test_user_wrapper_debug() {
        let mut mock_user = MockUser::new();
        mock_user.expect_id().return_const(Some(UserId::Int(42)));
        mock_user
            .expect_username()
            .return_const(Some(Cow::from("test_user")));
        mock_user.expect_is_active().return_const(true);
        mock_user.expect_is_authenticated().return_const(true);
        let now = DateTime::parse_from_rfc3339("2023-01-01T12:00:00+00:00").unwrap();
        mock_user.expect_last_login().return_const(Some(now));
        mock_user.expect_joined().return_const(Some(now));

        let user_wrapper = UserWrapper(Arc::new(mock_user));

        let debug_output = format!("{user_wrapper:?}");

        assert!(debug_output.contains("Arc<dyn User + Send + Sync>"));
        assert!(debug_output.contains("id: Some(Int(42))"));
        assert!(debug_output.contains("username: Some(\"test_user\")"));
        assert!(debug_output.contains("is_active: true"));
        assert!(debug_output.contains("is_authenticated: true"));
        assert!(debug_output.contains("2023-01-01T12:00:00+00:00"));
    }
}
