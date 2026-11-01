import React, { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { ShieldCheck, Lock, Trash2, ShieldAlert } from 'lucide-react';
import { getErrorMessage } from '../types';

interface DashboardProps {
  onLock: () => void;
  onResetSuccess: () => void;
  onError: (msg: string) => void;
}

export default function Dashboard({ onLock, onResetSuccess, onError }: DashboardProps) {
  const [busy, setBusy] = useState(false);
  const [showResetModal, setShowResetModal] = useState(false);
  const [resetPassword, setResetPassword] = useState('');
  const [resetBusy, setResetBusy] = useState(false);
  const [resetError, setResetError] = useState('');

  const handleLock = async () => {
    setBusy(true);
    try {
      await invoke('lock_wallet');
      onLock();
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
    <div className="unlocked-dashboard">
      <div className="success">
        <ShieldCheck size={20} />
        <span>Vault unlocked in native memory. Auto-lock triggers after 5 minutes of inactivity.</span>
      </div>

      <div className="assets">
        <div className="asset-card">
          <div className="asset-title">
            <span className="asset-symbol">BTC</span>
            <strong>Bitcoin (SegWit Native / P2WPKH)</strong>
          </div>
          <span className="asset-sub">Derivation path: m/84'/0'/0'/0/0 (Mainnet) · m/84'/1'/0'/0/0 (Signet)</span>
        </div>

        <div className="asset-card">
          <div className="asset-title">
            <span className="asset-symbol">USDT</span>
            <strong>Tether USD (Binance-Peg BSC-USD · BEP-20)</strong>
          </div>
          <span className="asset-sub">
            Contract: 0x55d398326f99059fF775485246999027B3197955 (18 Decimals · Chain ID 56 / 97)
          </span>
        </div>
      </div>

      <div className="dashboard-actions">
        <button disabled={busy} onClick={() => void handleLock()}>
          <Lock size={16} /> Lock Vault
        </button>

        <button
          type="button"
          className="danger-btn"
          onClick={() => {
            setShowResetModal(true);
            setResetError('');
          }}
          disabled={busy}
        >
          <Trash2 size={16} /> Authenticated Reset
        </button>
      </div>

      {showResetModal && (
        <div className="modal-backdrop">
          <div className="modal-card">
            <header className="modal-header">
              <ShieldAlert size={22} className="text-red-400" />
              <h3>Confirm Vault Deletion</h3>
            </header>
            <p className="muted small">
              This will permanently delete the encrypted vault from this device.
              Enter your master password to authenticate.
            </p>

            {resetError && (
              <div className="error" role="alert">
                <span>{resetError}</span>
              </div>
            )}

            <form onSubmit={handleAuthenticatedReset}>
              <label>
                Master Password
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
                  {resetBusy ? 'Authenticating…' : 'Wipe Vault'}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
}
