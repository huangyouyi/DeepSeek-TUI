use std::collections::HashMap;
use std::fmt;
use std::time::{Duration, Instant};

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PairingCode {
    value: String,
    expires_at: Instant,
}

impl PairingCode {
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    #[must_use]
    pub fn expires_at(&self) -> Instant {
        self.expires_at
    }
}

impl fmt::Debug for PairingCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PairingCode")
            .field("value", &"<redacted>")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct PairingToken {
    value: String,
    descriptor: PairingTokenDescriptor,
}

impl PairingToken {
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    #[must_use]
    pub fn descriptor(&self) -> &PairingTokenDescriptor {
        &self.descriptor
    }
}

impl fmt::Debug for PairingToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PairingToken")
            .field("value", &"<redacted>")
            .field("descriptor", &self.descriptor)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingTokenDescriptor {
    label: String,
}

impl PairingTokenDescriptor {
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairingError {
    Unknown,
    Expired,
    AlreadyUsed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PairingCodeState {
    Pending,
    Used,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PairingRecord {
    expires_at: Instant,
    state: PairingCodeState,
}

pub struct PairingManager<'a> {
    code_ttl: Duration,
    records: HashMap<String, PairingRecord>,
    code_generator: Box<dyn FnMut() -> String + Send + 'a>,
    token_generator: Box<dyn FnMut() -> String + Send + 'a>,
}

impl PairingManager<'static> {
    #[must_use]
    pub fn deterministic(code_ttl: Duration) -> Self {
        let mut next_code = 0_u64;
        let mut next_token = 0_u64;

        Self::with_generators(
            code_ttl,
            move || {
                next_code += 1;
                format!("{next_code:06}")
            },
            move || {
                next_token += 1;
                format!("pairing-token-{next_token:06}")
            },
        )
    }
}

impl<'a> PairingManager<'a> {
    #[must_use]
    pub fn with_generators<C, T>(
        code_ttl: Duration,
        code_generator: C,
        token_generator: T,
    ) -> PairingManager<'a>
    where
        C: FnMut() -> String + Send + 'a,
        T: FnMut() -> String + Send + 'a,
    {
        PairingManager {
            code_ttl,
            records: HashMap::new(),
            code_generator: Box::new(code_generator),
            token_generator: Box::new(token_generator),
        }
    }

    pub fn create_at(&mut self, now: Instant) -> PairingCode {
        let value = (self.code_generator)();
        let expires_at = now + self.code_ttl;
        self.records.insert(
            value.clone(),
            PairingRecord {
                expires_at,
                state: PairingCodeState::Pending,
            },
        );

        PairingCode { value, expires_at }
    }

    pub fn redeem_at(&mut self, code: &str, now: Instant) -> Result<PairingToken, PairingError> {
        let record = self.records.get_mut(code).ok_or(PairingError::Unknown)?;

        if record.state == PairingCodeState::Used {
            return Err(PairingError::AlreadyUsed);
        }

        if now > record.expires_at {
            return Err(PairingError::Expired);
        }

        record.state = PairingCodeState::Used;

        Ok(PairingToken {
            value: (self.token_generator)(),
            descriptor: PairingTokenDescriptor {
                label: "pairing-token".to_string(),
            },
        })
    }
}

impl PairingManager<'_> {
    pub fn create(&mut self) -> PairingCode {
        self.create_at(Instant::now())
    }

    pub fn redeem(&mut self, code: &str) -> Result<PairingToken, PairingError> {
        self.redeem_at(code, Instant::now())
    }
}

impl fmt::Debug for PairingManager<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PairingManager")
            .field("code_ttl", &self.code_ttl)
            .field("records", &self.records)
            .finish_non_exhaustive()
    }
}
