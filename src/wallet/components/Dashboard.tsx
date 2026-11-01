import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { ShieldCheck, Lock, Trash2, ShieldAlert, Copy, Check, Radio } from 'lucide-react';
import { PublicAddresses, getErrorMessage } from '../types';

interface DashboardProps {
  onLock: () => void;
  onResetSuccess: () => void;
  onError: (msg: string) => void;
}

export default function Dashboard({ onLock, onResetSuccess, onError }: DashboardProps) {
  const [addresses, setAddresses] = useState<PublicAddresses | null>(null);
  const [loadingAddrs, setLoadingAddrs] = useState(true);
  const [copiedKey, setCopiedKey] = useState<string | null>(null);
  const [btcNetwork, setBtcNetwork] = useState<'mainnet' | 'signet'>('signet');
  const [busy, setBusy] = useState(false);

  // Authenticated Reset Modal
  const [showResetModal, setShowResetModal] = useState(false);
  const [resetPassword, setResetPassword] = useState('');
  const [resetBusy, setResetBusy] = useState(false);
  const [resetError, setResetError] = useState('');

  useEffect(() => {
    async function loadAddresses() {
      try {
        const addrs = await invoke<PublicAddresses>('get_public_addresses');
        setAddresses(addrs);
      } catch (err) {
        onError(getErrorMessage(err));
      } finally {
        setLoadingAddrs(false);
      }
    }
    void loadAddresses();
  }, [onError]);

  const copyToClipboard = (text: string, key: string) => {
    void navigator.clipboard.writeText(text);
    setCopiedKey(key);
    setTimeout(() => setCopiedKey(null), 2000);
  };

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

  const activeBtcAddress =
    btcNetwork === 'mainnet' ? addresses?.btcMainnet : addresses?.btcSignet;

  return (
    <div className="unlocked-dashboard">
      <div className="success">
        <ShieldCheck size={20} />
        <span>Vault unlocked in native memory (5-minute inactivity auto-lock active).</span>
      </div>

      <div className="assets">
        {/* Bitcoin Card */}
        <div className="asset-card">
          <div className="asset-header-row">
            <div className="asset-title">
              <span className="asset-symbol btc-badge">BTC</span>
              <strong>Bitcoin Native SegWit (BIP84 · P2WPKH)</strong>
            </div>
            <div className="network-toggle">
              <button
                type="button"
                className={`toggle-btn ${btcNetwork === 'signet' ? 'active' : ''}`}
                onClick={() => setBtcNetwork('signet')}
              >
                Signet
              </button>
              <button
                type="button"
                className={`toggle-btn ${btcNetwork === 'mainnet' ? 'active' : ''}`}
                onClick={() => setBtcNetwork('mainnet')}
              >
                Mainnet
              </button>
            </div>
          </div>

          <span className="asset-sub">
            Derivation path:{' '}
            <code>{btcNetwork === 'mainnet' ? "m/84'/0'/0'/0/0" : "m/84'/1'/0'/0/0"}</code>
          </span>

          <div className="address-box">
            <span className="address-label">Receiving Address:</span>
            {loadingAddrs ? (
              <span className="address-value muted">Deriving Native SegWit address…</span>
            ) : (
              <div className="address-display">
                <code className="address-code">{activeBtcAddress}</code>
                <button
                  type="button"
                  className="icon-copy-btn"
                  onClick={() => activeBtcAddress && copyToClipboard(activeBtcAddress, 'btc')}
                  title="Copy address"
                >
                  {copiedKey === 'btc' ? <Check size={14} className="text-emerald-400" /> : <Copy size={14} />}
                </button>
              </div>
            )}
          </div>
        </div>

        {/* BSC & USDT Card */}
        <div className="asset-card">
          <div className="asset-header-row">
            <div className="asset-title">
              <span className="asset-symbol bsc-badge">BSC</span>
              <strong>BNB Smart Chain & USDT (BEP-20)</strong>
            </div>
            <span className="tag-chip">Chain ID 56 / 97</span>
          </div>

          <span className="asset-sub">
            EVM Derivation path: <code>m/44'/60'/0'/0/0</code> (EIP-55 checksum)
          </span>

          <div className="address-box">
            <span className="address-label">Account Address:</span>
            {loadingAddrs ? (
              <span className="address-value muted">Deriving EVM address…</span>
            ) : (
              <div className="address-display">
                <code className="address-code">{addresses?.bscAddress}</code>
                <button
                  type="button"
                  className="icon-copy-btn"
                  onClick={() => addresses?.bscAddress && copyToClipboard(addresses.bscAddress, 'bsc')}
                  title="Copy address"
                >
                  {copiedKey === 'bsc' ? <Check size={14} className="text-emerald-400" /> : <Copy size={14} />}
                </button>
              </div>
            )}
          </div>

          <div className="contract-box">
            <span className="contract-title">Pegged Token:</span>
            <div className="contract-info">
              <span>Binance-Peg BSC-USD (18 Decimals)</span>
              <code className="contract-code">{addresses?.usdtBep20Contract}</code>
            </div>
          </div>
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
