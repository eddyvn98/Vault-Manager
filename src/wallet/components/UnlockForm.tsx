import React, { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { KeyRound, Clock, RotateCcw, ShieldAlert } from 'lucide-react';
import { WalletStatus, getErrorMessage } from '../types';

interface UnlockFormProps {
  status: WalletStatus;
  cooldown: number;
  onSuccess: () => void;
  onResetSuccess: () => void;
  onError: (msg: string) => void;
}

export default function UnlockForm({
  status,
  cooldown,
  onSuccess,
  onResetSuccess,
  onError,
}: UnlockFormProps) {
  const [password, setPassword] = useState('');
  const [busy, setBusy] = useState(false);

  // Authenticated Reset Modal
  const [showResetModal, setShowResetModal] = useState(false);
  const [resetPassword, setResetPassword] = useState('');
  const [resetBusy, setResetBusy] = useState(false);
  const [resetError, setResetError] = useState('');

  const handleUnlock = async (e: React.FormEvent) => {
    e.preventDefault();
    if (cooldown > 0) return;

    setBusy(true);
    onError('');
    try {
      await invoke('unlock_wallet', { password });
      setPassword('');
      onSuccess();
    } catch (err) {
      onError(getErrorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const handleAuthenticatedReset = async (e: React.FormEvent) => {
    e.preventDefault();
    setResetBusy(true);
    setResetError('');
    try {
      await invoke('reset_wallet', { password: resetPassword });
      setResetPassword('');
      setShowResetModal(false);
      onResetSuccess();
    } catch (err) {
      setResetError(getErrorMessage(err));
    } finally {
      setResetBusy(false);
    }
  };

  return (
    <div className="unlock-container">
      <form onSubmit={handleUnlock}>
        <div className="unlock-header">
          <span>
            <KeyRound size={16} /> Device vault encrypted
          </span>
          {status.failedAttempts > 0 && (
            <span className="failed-badge">
              {status.failedAttempts} failed attempt{status.failedAttempts > 1 ? 's' : ''}
            </span>
          )}
        </div>

        {cooldown > 0 && (
          <div className="warning">
            <Clock size={18} />
            <span>
              Device rate-limited. Cooldown: <strong>{cooldown}s</strong>
            </span>
          </div>
        )}

        <label>
          Master Password
          <input
            type="password"
            autoComplete="current-password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            placeholder="Enter master password"
            disabled={cooldown > 0 || busy}
            required
          />
        </label>

        <button
          type="submit"
          disabled={busy || password.length < 12 || cooldown > 0}
        >
          {busy
            ? 'Verifying Argon2id…'
            : cooldown > 0
            ? `Cooldown active (${cooldown}s)`
            : 'Unlock Vault'}
        </button>

        <div className="reset-row">
          <button
            type="button"
            className="link-btn"
            onClick={() => {
              setShowResetModal(true);
              setResetError('');
            }}
            disabled={busy || cooldown > 0}
          >
            <RotateCcw size={14} /> Reset or remove vault on this device
          </button>
        </div>

        <p className="muted small">
          Lockout backoff is persisted across application restarts to protect against offline brute force attacks.
        </p>
      </form>

      {showResetModal && (
        <div className="modal-backdrop">
          <div className="modal-card">
            <header className="modal-header">
              <ShieldAlert size={22} className="text-red-400" />
              <h3>Authenticated Vault Reset</h3>
            </header>
            <p className="muted small">
              To prevent unauthorized erasure, you must authenticate with the master password.
              Ensure your 24-word recovery phrase is safely written down before proceeding.
            </p>

            {resetError && (
              <div className="error" role="alert">
                <span>{resetError}</span>
              </div>
            )}

            <form onSubmit={handleAuthenticatedReset}>
              <label>
                Confirm Master Password
                <input
                  type="password"
                  value={resetPassword}
                  onChange={(e) => setResetPassword(e.target.value)}
                  placeholder="Master password"
                  required
                />
              </label>

              <div className="modal-actions">
                <button
                  type="button"
                  className="subtle"
                  onClick={() => setShowResetModal(false)}
                  disabled={resetBusy}
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  className="danger-btn"
                  disabled={resetBusy || resetPassword.length < 12}
                >
                  {resetBusy ? 'Authenticating…' : 'Authenticate & Wipe Vault'}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
}
