import { useEffect, useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { ShieldCheck, AlertTriangle, CheckCircle2 } from 'lucide-react';
import { WalletStatus, Action, offlineStatus, getErrorMessage } from './types';
import CreateFlow from './components/CreateFlow';
import RestoreFlow from './components/RestoreFlow';
import UnlockForm from './components/UnlockForm';
import Dashboard from './components/Dashboard';

export default function WalletApp() {
  const [status, setStatus] = useState<WalletStatus>(offlineStatus);
  const [loading, setLoading] = useState(true);
  const [action, setAction] = useState<Action>('create');
  const [error, setError] = useState('');
  const [successMsg, setSuccessMsg] = useState('');
  const [cooldown, setCooldown] = useState(0);

  const refresh = useCallback(async () => {
    try {
      const next = await invoke<WalletStatus>('wallet_status');
      setStatus(next);
      setCooldown(next.lockoutSeconds);
      if (next.exists && !next.unlocked) {
        setAction('unlock');
      } else if (!next.exists) {
        setAction('create');
      }
    } catch (err) {
      setError(getErrorMessage(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  // Lockout countdown timer
  useEffect(() => {
    if (cooldown <= 0) return;
    const interval = window.setInterval(() => {
      setCooldown((prev) => {
        if (prev <= 1) {
          clearInterval(interval);
          void refresh();
          return 0;
        }
        return prev - 1;
      });
    }, 1000);
    return () => clearInterval(interval);
  }, [cooldown, refresh]);

  // Window visibility / background lock
  useEffect(() => {
    const handleVisibilityChange = () => {
      if (document.visibilityState === 'hidden' && status.unlocked) {
        void invoke('lock_wallet').then(refresh).catch(() => {});
      }
    };
    document.addEventListener('visibilitychange', handleVisibilityChange);
    return () => document.removeEventListener('visibilitychange', handleVisibilityChange);
  }, [status.unlocked, refresh]);

  // Rust session touch & activity listener
  useEffect(() => {
    if (!status.unlocked) return;
    const handleActivity = () => {
      void invoke<boolean>('touch_session').then((active) => {
        if (!active) {
          void refresh();
          setError('Session expired due to inactivity. Vault locked.');
        }
      }).catch(() => {});
    };

    const events = ['mousedown', 'keydown', 'touchstart'];
    events.forEach((ev) => window.addEventListener(ev, handleActivity, { passive: true }));
    return () => {
      events.forEach((ev) => window.removeEventListener(ev, handleActivity));
    };
  }, [status.unlocked, refresh]);

  return (
    <main className="wallet-shell">
      <section className="wallet-card">
        <header className="wallet-header">
          <div className="brand">
            <ShieldCheck size={28} /> <strong>Vault Manager</strong>
          </div>
          <div className="chips">
            <span className="chip">P0–P1 · LOCAL ONLY</span>
            {status.unlocked ? (
              <span className="chip status-unlocked">UNLOCKED</span>
            ) : status.exists ? (
              <span className="chip status-locked">LOCKED</span>
            ) : (
              <span className="chip status-new">NEW DEVICE</span>
            )}
          </div>
        </header>

        <h1>Personal Native Wallet</h1>
        <p className="muted">
          Isolated local cryptographic vault. Seed and private keys are never transmitted over the network
          or shared between devices.
        </p>

        {error && (
          <div className="error" role="alert">
            <AlertTriangle size={18} /> <span>{error}</span>
          </div>
        )}

        {successMsg && (
          <div className="success">
            <CheckCircle2 size={18} /> <span>{successMsg}</span>
          </div>
        )}

        {loading ? (
          <p className="muted">Initializing local secure vault…</p>
        ) : status.unlocked ? (
          <Dashboard
            onLock={() => {
              void refresh();
              setSuccessMsg('Vault locked safely.');
            }}
            onResetSuccess={() => {
              void refresh();
              setSuccessMsg('Vault has been deleted from this device.');
            }}
            onError={setError}
          />
        ) : status.exists ? (
          <UnlockForm
            status={status}
            cooldown={cooldown}
            onSuccess={() => {
              void refresh();
              setSuccessMsg('');
            }}
            onResetSuccess={() => {
              void refresh();
              setSuccessMsg('Vault has been deleted from this device.');
            }}
            onError={setError}
          />
        ) : (
          <div>
            <nav className="tabs">
              <button
                type="button"
                className={action === 'create' ? 'active' : ''}
                onClick={() => {
                  setAction('create');
                  setError('');
                }}
              >
                Create New Wallet
              </button>
              <button
                type="button"
                className={action === 'restore' ? 'active' : ''}
                onClick={() => {
                  setAction('restore');
                  setError('');
                }}
              >
                Restore from Phrase
              </button>
            </nav>

            {action === 'create' ? (
              <CreateFlow
                onSuccess={() => {
                  void refresh();
                  setSuccessMsg('Wallet encrypted and saved. Please unlock to begin.');
                }}
                onError={setError}
              />
            ) : (
              <RestoreFlow
                onSuccess={() => {
                  void refresh();
                  setSuccessMsg('Wallet restored successfully. Please unlock to begin.');
                }}
                onError={setError}
              />
            )}
          </div>
        )}
      </section>
    </main>
  );
}
