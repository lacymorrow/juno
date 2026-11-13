//! Juno's secrets: the AI provider keys, the voice engines' account keys and
//! the cloud device key.
//!
//! They live in the OS credential store (the Keychain on macOS, Credential
//! Manager on Windows) through the `keyring` crate, and nowhere else: not in
//! the settings file, not in a log line, not in argv, and not on the page.
//! Settings learns that a key is saved, never the key. This module is the only
//! caller of `keyring`.
//!
//! A credential store read blocks, and every request builds a brain that wants
//! its key. So every secret is read once, at launch ([`load`]), into a cache in
//! this process. A write goes to the store first, on a blocking thread, and
//! reaches the cache only once the store has taken it. Every read after launch
//! is a map lookup.
//!
//! Environment variables (`ANTHROPIC_API_KEY` and the rest) are not owned
//! here. Each reader asks this module first and then falls back to its own
//! variable, exactly as it did when the key sat in settings.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, PoisonError, RwLock, RwLockWriteGuard};

use crate::agent::providers::Provider;

/// What every entry is filed under in the credential store: the bundle
/// identifier, so Keychain Access lists them as Juno's.
#[cfg(not(test))]
const SERVICE: &str = "com.juno.desktop";

/// A secret Juno keeps, by name. The value is never part of this type, so
/// logging one says which secret, not what it is.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Secret {
    /// An AI provider's API key. For Copilot it is a GitHub token.
    Provider(Provider),
    /// The ElevenLabs account key.
    ElevenLabs,
    /// The Replicate account key, which Chatterbox speaks with too.
    Replicate,
    /// The key this device signs its cloud connection with.
    CloudDevice,
}

/// The providers that take a key. The CLIs carry their own sign-in.
const KEYED_PROVIDERS: [Provider; 5] = [
    Provider::Anthropic,
    Provider::OpenAI,
    Provider::Rig,
    Provider::Gemini,
    Provider::Copilot,
];

impl Secret {
    /// Every secret, for loading at launch and for Reset.
    pub fn all() -> Vec<Secret> {
        let mut all: Vec<Secret> = KEYED_PROVIDERS.into_iter().map(Secret::Provider).collect();
        all.extend([Secret::ElevenLabs, Secret::Replicate, Secret::CloudDevice]);
        all
    }

    /// The secret for a provider id from settings, when that provider takes
    /// a key at all.
    pub fn for_provider_id(id: &str) -> Option<Secret> {
        Provider::from_str(id)
            .filter(|provider| KEYED_PROVIDERS.contains(provider))
            .map(Secret::Provider)
    }

    /// The entry's name in the credential store.
    fn account(&self) -> String {
        match self {
            Secret::Provider(provider) => format!("provider.{}", provider.id()),
            Secret::ElevenLabs => "voice.elevenlabs".to_string(),
            Secret::Replicate => "voice.replicate".to_string(),
            Secret::CloudDevice => "cloud.device".to_string(),
        }
    }
}

/// Where the values are kept: the OS credential store in the app, memory in
/// tests, so no test ever reads or writes a real Keychain. Every call blocks.
trait Store: Send + Sync {
    fn read(&self, account: &str) -> Result<Option<String>, String>;
    fn write(&self, account: &str, value: &str) -> Result<(), String>;
    fn remove(&self, account: &str) -> Result<(), String>;
}

#[cfg(not(test))]
struct OsStore;

#[cfg(not(test))]
impl Store for OsStore {
    fn read(&self, account: &str) -> Result<Option<String>, String> {
        match keyring::Entry::new(SERVICE, account).and_then(|entry| entry.get_password()) {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(describe(&e)),
        }
    }

    fn write(&self, account: &str, value: &str) -> Result<(), String> {
        keyring::Entry::new(SERVICE, account)
            .and_then(|entry| entry.set_password(value))
            .map_err(|e| describe(&e))
    }

    fn remove(&self, account: &str) -> Result<(), String> {
        match keyring::Entry::new(SERVICE, account).and_then(|entry| entry.delete_credential()) {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(describe(&e)),
        }
    }
}

/// A store error in words. Two variants carry the raw bytes they could not
/// decode, which would be the secret, so those are described, not printed.
#[cfg(not(test))]
fn describe(error: &keyring::Error) -> String {
    match error {
        keyring::Error::BadEncoding(_) | keyring::Error::BadDataFormat(..) => {
            "the saved value is not readable text".to_string()
        }
        other => other.to_string(),
    }
}

/// Set by [`keep_in_memory`].
static IN_MEMORY: AtomicBool = AtomicBool::new(false);

/// Keep secrets in memory for the rest of this process instead of the
/// credential store. For the test harness: integration tests link the library
/// without `cfg(test)`, and no test may touch a real Keychain. Has no effect
/// once anything has read a secret.
pub fn keep_in_memory() {
    IN_MEMORY.store(true, Ordering::Relaxed);
}

#[cfg(not(test))]
fn platform_store() -> Box<dyn Store> {
    if IN_MEMORY.load(Ordering::Relaxed) {
        Box::new(MemoryStore::default())
    } else {
        Box::new(OsStore)
    }
}

#[derive(Default)]
struct MemoryStore(std::sync::Mutex<HashMap<String, String>>);

impl MemoryStore {
    fn entries(&self) -> std::sync::MutexGuard<'_, HashMap<String, String>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Store for MemoryStore {
    fn read(&self, account: &str) -> Result<Option<String>, String> {
        Ok(self.entries().get(account).cloned())
    }

    fn write(&self, account: &str, value: &str) -> Result<(), String> {
        self.entries()
            .insert(account.to_string(), value.to_string());
        Ok(())
    }

    fn remove(&self, account: &str) -> Result<(), String> {
        self.entries().remove(account);
        Ok(())
    }
}

#[cfg(test)]
fn platform_store() -> Box<dyn Store> {
    Box::new(MemoryStore::default())
}

/// A pasted key with its surrounding whitespace gone, or `None` for a blank.
fn usable(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// A store and the copy of it this process reads from.
struct Vault {
    store: Box<dyn Store>,
    cache: RwLock<HashMap<Secret, String>>,
}

impl Vault {
    /// Read every secret out of `store`. Blocks.
    fn open(store: Box<dyn Store>) -> Self {
        let mut cache = HashMap::new();
        for secret in Secret::all() {
            match store.read(&secret.account()) {
                Ok(Some(value)) => {
                    if let Some(value) = usable(&value) {
                        cache.insert(secret, value);
                    }
                }
                Ok(None) => {}
                Err(e) => tracing::warn!("[Secrets] Could not read {:?}: {}", secret, e),
            }
        }
        Self {
            store,
            cache: RwLock::new(cache),
        }
    }

    fn get(&self, secret: &Secret) -> Option<String> {
        self.cache
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(secret)
            .cloned()
    }

    fn provider_key(&self, provider: &Provider) -> Option<String> {
        self.get(&Secret::Provider(provider.clone()))
            .or_else(|| match provider {
                // Rig speaks to OpenAI, so OpenAI's key serves it unless Rig
                // has one of its own.
                Provider::Rig => self.get(&Secret::Provider(Provider::OpenAI)),
                _ => None,
            })
    }

    fn cache_mut(&self) -> RwLockWriteGuard<'_, HashMap<Secret, String>> {
        self.cache.write().unwrap_or_else(PoisonError::into_inner)
    }

    /// Blocks. The cache lock is taken only after the store has answered.
    fn set(&self, secret: Secret, value: &str) -> Result<(), String> {
        let value = usable(value).ok_or("An empty key cannot be saved")?;
        self.store.write(&secret.account(), &value)?;
        self.cache_mut().insert(secret, value);
        Ok(())
    }

    /// Blocks. Deleting a secret that was never saved succeeds.
    fn delete(&self, secret: &Secret) -> Result<(), String> {
        self.store.remove(&secret.account())?;
        self.cache_mut().remove(secret);
        Ok(())
    }

    /// Blocks. Tries every secret even when one fails.
    fn delete_all(&self) -> Result<(), String> {
        let failed: Vec<String> = Secret::all()
            .iter()
            .filter_map(|secret| {
                self.delete(secret)
                    .err()
                    .map(|e| format!("{:?} ({})", secret, e))
            })
            .collect();
        if failed.is_empty() {
            Ok(())
        } else {
            Err(format!("Could not delete {}", failed.join(", ")))
        }
    }
}

static VAULT: LazyLock<Vault> = LazyLock::new(|| Vault::open(platform_store()));

/// Read every secret into the cache. Blocks, so it runs once at the very start
/// of launch on the main thread, before anything asks for a key. A read before
/// it would do the same work on whatever thread asked.
pub fn load() {
    LazyLock::force(&VAULT);
}

/// The saved value of `secret`, from the cache.
pub fn get(secret: &Secret) -> Option<String> {
    VAULT.get(secret)
}

/// Whether `secret` has a saved value. What the page is told instead of it.
pub fn is_saved(secret: &Secret) -> bool {
    VAULT.get(secret).is_some()
}

/// The saved key for `provider`: its own, or for Rig, OpenAI's.
pub fn provider_key(provider: &Provider) -> Option<String> {
    VAULT.provider_key(provider)
}

/// Save `value` (trimmed) as `secret`. A blank value is refused; use
/// [`delete`] to clear one.
pub async fn save(secret: Secret, value: String) -> Result<(), String> {
    blocking(move || VAULT.set(secret, &value)).await
}

/// Remove `secret` from the store and the cache.
pub async fn delete(secret: Secret) -> Result<(), String> {
    blocking(move || VAULT.delete(&secret)).await
}

/// Remove every secret. Settings Reset calls this.
pub async fn delete_all() -> Result<(), String> {
    blocking(|| VAULT.delete_all()).await
}

async fn blocking<F>(work: F) -> Result<(), String>
where
    F: FnOnce() -> Result<(), String> + Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|e| format!("The credential store task failed: {}", e))?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vault_with(entries: &[(&Secret, &str)]) -> Vault {
        let store = MemoryStore::default();
        for (secret, value) in entries {
            store.write(&secret.account(), value).unwrap();
        }
        Vault::open(Box::new(store))
    }

    #[test]
    fn what_the_store_holds_at_launch_is_what_reads_see() {
        let anthropic = Secret::Provider(Provider::Anthropic);
        let vault = vault_with(&[(&anthropic, "sk-ant-example"), (&Secret::Replicate, "  ")]);
        assert_eq!(vault.get(&anthropic).as_deref(), Some("sk-ant-example"));
        // A blank entry is no key.
        assert_eq!(vault.get(&Secret::Replicate), None);
        assert_eq!(vault.get(&Secret::ElevenLabs), None);
    }

    #[test]
    fn a_saved_key_is_trimmed_and_read_back() {
        let vault = vault_with(&[]);
        vault.set(Secret::ElevenLabs, "  el-key \n").unwrap();
        assert_eq!(vault.get(&Secret::ElevenLabs).as_deref(), Some("el-key"));
        assert_eq!(
            vault.store.read(&Secret::ElevenLabs.account()).unwrap(),
            Some("el-key".to_string())
        );
    }

    #[test]
    fn a_blank_key_is_refused_and_changes_nothing() {
        let vault = vault_with(&[(&Secret::CloudDevice, "device-key")]);
        assert!(vault.set(Secret::CloudDevice, "   ").is_err());
        assert_eq!(
            vault.get(&Secret::CloudDevice).as_deref(),
            Some("device-key")
        );
    }

    #[test]
    fn delete_clears_the_store_and_the_cache() {
        let vault = vault_with(&[(&Secret::Replicate, "r8_abc")]);
        vault.delete(&Secret::Replicate).unwrap();
        assert_eq!(vault.get(&Secret::Replicate), None);
        assert_eq!(
            vault.store.read(&Secret::Replicate.account()).unwrap(),
            None
        );
        // Deleting what is not there is not an error.
        vault.delete(&Secret::Replicate).unwrap();
    }

    #[test]
    fn delete_all_leaves_nothing() {
        let vault = vault_with(&[
            (&Secret::Provider(Provider::Copilot), "gho_example"),
            (&Secret::CloudDevice, "device-key"),
        ]);
        vault.delete_all().unwrap();
        for secret in Secret::all() {
            assert_eq!(vault.get(&secret), None, "{:?}", secret);
            assert_eq!(vault.store.read(&secret.account()).unwrap(), None);
        }
    }

    #[test]
    fn rig_borrows_the_openai_key_unless_it_has_its_own() {
        let vault = vault_with(&[(&Secret::Provider(Provider::OpenAI), "sk-proj-example")]);
        assert_eq!(
            vault.provider_key(&Provider::Rig).as_deref(),
            Some("sk-proj-example")
        );
        vault
            .set(Secret::Provider(Provider::Rig), "sk-rig-example")
            .unwrap();
        assert_eq!(
            vault.provider_key(&Provider::Rig).as_deref(),
            Some("sk-rig-example")
        );
        assert_eq!(vault.provider_key(&Provider::Gemini), None);
    }

    #[test]
    fn only_providers_that_take_a_key_have_a_secret() {
        assert_eq!(
            Secret::for_provider_id("anthropic"),
            Some(Secret::Provider(Provider::Anthropic))
        );
        assert_eq!(
            Secret::for_provider_id("copilot"),
            Some(Secret::Provider(Provider::Copilot))
        );
        assert_eq!(Secret::for_provider_id("claude_cli"), None);
        assert_eq!(Secret::for_provider_id("codex_cli"), None);
        assert_eq!(Secret::for_provider_id("nonsense"), None);
    }

    #[test]
    fn every_secret_has_its_own_entry() {
        let accounts: std::collections::HashSet<String> =
            Secret::all().iter().map(Secret::account).collect();
        assert_eq!(accounts.len(), Secret::all().len());
    }
}
