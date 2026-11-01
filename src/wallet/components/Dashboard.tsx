import React, { useEffect, useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  ShieldCheck,
  Lock,
  Trash2,
  ShieldAlert,
  Copy,
  Check,
  RotateCw,
  Coins,
  AlertCircle,
} from 'lucide-react';
import {
  PublicAddresses,
  BtcBalance,
  BscBalance,
  getErrorMessage,
} from '../types';

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
  const [bscNetwork, setBscNetwork] = useState<'mainnet' | 'testnet'>('mainnet');
  const [busy, setBusy] = useState(false);

  // Balance states
  const [btcBalance, setBtcBalance] = useState<BtcBalance | null>(null);
  const [loadingBtcBalance, setLoadingBtcBalance] = useState(false);
  const [btcBalanceError, setBtcBalanceError] = useState<string | null>(null);

  const [bscBalance, setBscBalance] = useState<BscBalance | null>(null);
  const [loadingBscBalance, setLoadingBscBalance] = useState(false);
  const [bscBalanceError, setBscBalanceError] = useState<string | null>(null);

  // Authenticated Reset Modal
  const [showResetModal, setShowResetModal] = useState(false);
  const [resetPassword, setResetPassword] = useState('');
  const [resetBusy, setResetBusy] = useState(false);
  const [resetError, setResetError] = useState('');

  // Fetch BTC Balance
  const fetchBtcBalance = useCallback(
    async (addr: string, isSignet: boolean) => {
      setLoadingBtcBalance(true);
      setBtcBalanceError(null);
      try {
        const bal = await invoke<BtcBalance>('query_btc_balance', {
          address: addr,
          signet: isSignet,
        });
        setBtcBalance(bal);
      } catch (err) {
        setBtcBalanceError(getErrorMessage(err));
      } finally {
        setLoadingBtcBalance(false);
      }
    },
    []
  );

  // Fetch BSC Balance
  const fetchBscBalance = useCallback(
    async (addr: string, isTestnet: boolean) => {
      setLoadingBscBalance(true);
      setBscBalanceError(null);
      try {
        const bal = await invoke<BscBalance>('query_bsc_balance', {
          address: addr,
          testnet: isTestnet,
        });
        setBscBalance(bal);
      } catch (err) {
        setBscBalanceError(getErrorMessage(err));
      } finally {
        setLoadingBscBalance(false);
      }
    },
    []
  );

  // Initial load of addresses and balances
  useEffect(() => {
    async function loadAddresses() {
      try {
        const addrs = await invoke<PublicAddresses>('get_public_addresses');
        setAddresses(addrs);

        const initBtcAddr = btcNetwork === 'mainnet' ? addrs.btcMainnet : addrs.btcSignet;
        void fetchBtcBalance(initBtcAddr, btcNetwork === 'signet');
        void fetchBscBalance(addrs.bscAddress, bscNetwork === 'testnet');
      } catch (err) {
        onError(getErrorMessage(err));
      } finally {
        setLoadingAddrs(false);
      }
    }
    void loadAddresses();
  }, [onError, fetchBtcBalance, fetchBscBalance, btcNetwork, bscNetwork]);

  const copyToClipboard = (text: string, key: string) => {
    void navigator.clipboard.writeText(text);
    setCopiedKey(key);
    setTimeout(() => setCopiedKey(null), 2000);
  };

  const handleBtcNetworkChange = (network: 'mainnet' | 'signet') => {
    setBtcNetwork(network);
    if (!addresses) return;
    const addr = network === 'mainnet' ? addresses.btcMainnet : addresses.btcSignet;
    void fetchBtcBalance(addr, network === 'signet');
  };

  const handleBscNetworkChange = (network: 'mainnet' | 'testnet') => {
    setBscNetwork(network);
    if (!addresses) return;
    void fetchBscBalance(addresses.bscAddress, network === 'testnet');
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
                onClick={() => handleBtcNetworkChange('signet')}
              >
                Signet
              </button>
              <button
                type="button"
                className={`toggle-btn ${btcNetwork === 'mainnet' ? 'active' : ''}`}
                onClick={() => handleBtcNetworkChange('mainnet')}
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

          {/* BTC Live Balance Section */}
          <div className="balance-box">
            <div className="balance-header">
              <span className="balance-title">
                <Coins size={14} /> On-Chain Balance ({btcNetwork.toUpperCase()})
              </span>
              <button
                type="button"
                className="mini-action-btn"
                disabled={loadingBtcBalance || !activeBtcAddress}
                onClick={() => activeBtcAddress && fetchBtcBalance(activeBtcAddress, btcNetwork === 'signet')}
                title="Refresh balance via Esplora API"
              >
                <RotateCw size={12} className={loadingBtcBalance ? 'spin' : ''} />
                Refresh
              </button>
            </div>

            {btcBalanceError ? (
              <div className="error text-xs flex items-center justify-between">
                <span>{btcBalanceError}</span>
                <button
                  type="button"
                  className="mini-action-btn"
                  onClick={() => activeBtcAddress && fetchBtcBalance(activeBtcAddress, btcNetwork === 'signet')}
                >
                  Retry
                </button>
              </div>
            ) : (
              <div className="balance-values">
                <div className="balance-primary">
                  {loadingBtcBalance && !btcBalance ? 'Loading…' : btcBalance?.formattedBtc ?? '0.00000000'}
                  <span className="balance-currency">BTC</span>
                </div>
                <div className="balance-secondary">
                  {btcBalance ? `${btcBalance.totalSatoshis.toLocaleString()} sats` : '0 sats'}
                  {btcBalance && btcBalance.unconfirmedSatoshis !== 0 && (
                    <span className="balance-pending ml-2">
                      {btcBalance.unconfirmedSatoshis > 0 ? '+' : ''}
                      {btcBalance.unconfirmedSatoshis.toLocaleString()} mempool
                    </span>
                  )}
                </div>
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
            <div className="network-toggle">
              <button
                type="button"
                className={`toggle-btn ${bscNetwork === 'mainnet' ? 'active' : ''}`}
                onClick={() => handleBscNetworkChange('mainnet')}
              >
                Mainnet
              </button>
              <button
                type="button"
                className={`toggle-btn ${bscNetwork === 'testnet' ? 'active' : ''}`}
                onClick={() => handleBscNetworkChange('testnet')}
              >
                Testnet
              </button>
            </div>
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

          {/* BSC Live Balance Section */}
          <div className="balance-box">
            <div className="balance-header">
              <span className="balance-title">
                <Coins size={14} /> Balances ({bscNetwork === 'mainnet' ? 'BSC Mainnet' : 'BSC Testnet'})
              </span>
              <button
                type="button"
                className="mini-action-btn"
                disabled={loadingBscBalance || !addresses?.bscAddress}
                onClick={() => addresses?.bscAddress && fetchBscBalance(addresses.bscAddress, bscNetwork === 'testnet')}
                title="Refresh BNB & USDT via JSON-RPC"
              >
                <RotateCw size={12} className={loadingBscBalance ? 'spin' : ''} />
                Refresh
              </button>
            </div>

            {bscBalanceError ? (
              <div className="error text-xs flex items-center justify-between">
                <span>{bscBalanceError}</span>
                <button
                  type="button"
                  className="mini-action-btn"
                  onClick={() => addresses?.bscAddress && fetchBscBalance(addresses.bscAddress, bscNetwork === 'testnet')}
                >
                  Retry
                </button>
              </div>
            ) : (
              <div className="balance-item-grid">
                <div className="token-subcard">
                  <span className="token-label">Binance-Peg BSC-USD (USDT)</span>
                  <div className="token-amount">
                    {loadingBscBalance && !bscBalance ? 'Loading…' : bscBalance?.formattedUsdt ?? '0.0000'}
                    <span className="balance-currency">USDT</span>
                  </div>
                </div>

                <div className="token-subcard">
                  <span className="token-label">Gas Token (BNB)</span>
                  <div className="token-amount bnb-color">
                    {loadingBscBalance && !bscBalance ? 'Loading…' : bscBalance?.formattedBnb ?? '0.0000'}
                    <span className="balance-currency">BNB</span>
                  </div>
                </div>
              </div>
            )}
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