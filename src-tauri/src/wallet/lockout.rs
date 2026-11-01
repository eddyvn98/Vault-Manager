use std::{
    fs,
    path::PathBuf,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
use serde::{Deserialize, Serialize};

const LOCKOUT_FILE: &str = "wallet-lockout.json";

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct LockoutData {
    pub failed_attempts: u32,
    pub lockout_until_unix: u64,
}

pub struct LockoutManager {
    dir: PathBuf,
    data: Mutex<LockoutData>,
}

impl LockoutManager {
    pub fn new(app_data_dir: PathBuf) -> Self {
        let file_path = app_data_dir.join(LOCKOUT_FILE);
        let data = if file_path.exists() {
            fs::read_to_string(&file_path)
                .ok()
                .and_then(|s| serde_json::from_str::<LockoutData>(&s).ok())
                .unwrap_or_default()
        } else {
            LockoutData::default()
        };
        Self {
            dir: app_data_dir,
            data: Mutex::new(data),
        }
    }

    pub fn now_unix() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    pub fn check_lockout(&self) -> Result<(), String> {
        let data = self.data.lock().map_err(|_| "Lockout state unavailable")?;
        let now = Self::now_unix();
        if now < data.lockout_until_unix {
            let remaining = data.lockout_until_unix - now;
            return Err(format!(
                "Too many failed attempts. Device locked for {} second(s). Please wait before trying again.",
                remaining
            ));
        }
        Ok(())
    }

    pub fn record_failure(&self) -> u64 {
        let mut data = match self.data.lock() {
            Ok(d) => d,
            Err(_) => return 0,
        };
        data.failed_attempts += 1;
        let delay_secs = match data.failed_attempts {
            0..=2 => 0,
            3 => 2,
            4 => 5,
            5 => 15,
            _ => 30,
        };
        let now = Self::now_unix();
        data.lockout_until_unix = if delay_secs > 0 { now + delay_secs } else { 0 };
        self.persist(&data);
        delay_secs
    }

    pub fn record_success(&self) {
        if let Ok(mut data) = self.data.lock() {
            data.failed_attempts = 0;
            data.lockout_until_unix = 0;
            self.persist(&data);
        }
    }

    pub fn get_status(&self) -> (u32, u64) {
        if let Ok(data) = self.data.lock() {
            let now = Self::now_unix();
            let remaining = if now < data.lockout_until_unix {
                data.lockout_until_unix - now
            } else {
                0
            };
            (data.failed_attempts, remaining)
        } else {
            (0, 0)
        }
    }

    fn persist(&self, data: &LockoutData) {
        let _ = fs::create_dir_all(&self.dir);
        let path = self.dir.join(LOCKOUT_FILE);
        if let Ok(json) = serde_json::to_string(data) {
            let _ = fs::write(path, json);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persistent_lockout_across_instances() {
        let temp_dir = std::env::temp_dir().join(format!("lockout_test_{}", LockoutManager::now_unix()));
        let _ = fs::remove_dir_all(&temp_dir);

        let mgr1 = LockoutManager::new(temp_dir.clone());
        assert!(mgr1.check_lockout().is_ok());

        mgr1.record_failure();
        mgr1.record_failure();
        assert!(mgr1.check_lockout().is_ok());

        // 3rd failure triggers backoff
        mgr1.record_failure();
        assert!(mgr1.check_lockout().is_err());

        // Recreate manager to simulate app restart
        let mgr2 = LockoutManager::new(temp_dir.clone());
        assert!(mgr2.check_lockout().is_err(), "Lockout must survive app restart");

        mgr2.record_success();
        assert!(mgr2.check_lockout().is_ok());

        let mgr3 = LockoutManager::new(temp_dir.clone());
        assert!(mgr3.check_lockout().is_ok(), "Reset must survive app restart");
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
