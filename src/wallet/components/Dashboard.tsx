import React, { useEffect, useState, useRef, useCallback } from 'react';
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
  Info,
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

  // Balance states & race condition protection
  const btcReqSeq = useRef(0);
  const [btcBalance, setBtcBalance] = useState<BtcBalance | null>(null);
  const [loadingBtcBalance, setLoadingBtcBalance] = useState(false);
  const [btcBalanceError, setBtcBalanceError] = useState<string | null>(null);

  const bscReqSeq = useRef(0);
  const [bscBalance, setBscBalance] = useState<BscBalance | null>(null);
  const [loadingBscBalance, setLoadingBscBalance] = useState(false);
  const [bscBalanceError, setBscBalanceError] = useState<string | null>(null);

  // Authenticated Reset Modal
  const [showResetModal, setShowResetModal] = useState(false);
  const [resetPassword, setResetPassword] = useState('');
  const [resetBusy, setResetBusy] = useState(false);
  const [resetError, setResetError] = useState('');

  // Fetch BTC Balance with sequence-id guard against race conditions
  const fetchBtcBalance = useCallback(
    async (addr: string, isSignet: boolean) => {
      const currentReqId = ++btcReqSeq.current;
      setLoadingBtcBalance(true);
      setBtcBalanceError(null);

      try {
        const bal = await invoke<BtcBalance>('query_btc_balance', {
          address: addr,
          signet: isSignet,
        });
        if (currentReqId === btcReqSeq.current) {
          setBtcBalance(bal);
        }
      } catch (err) {
        if (currentReqId === btcReqSeq.current) {
          setBtcBalanceError(getErrorMessage(err));
        }
      } finally {
        if (currentReqId === btcReqSeq.current) {
          setLoadingBtcBalance(false);
        }
      }
    },
    []
  );

  // Fetch BSC Balance with sequence-id guard against race conditions
  const fetchBscBalance = useCallback(
    async (addr: string, isTestnet: boolean) => {
      const currentReqId = ++bscReqSeq.current;
      setLoadingBscBalance(true);
      setBscBalanceError(null);

      try {
        const bal = await invoke<BscBalance>('query_bsc_balance', {
          address: addr,
          testnet: isTestnet,
        });
        if (currentReqId === bscReqSeq.current) {
          setBscBalance(bal);
        }
      } catch (err) {
        if (currentReqId === bscReqSeq.current) {
          setBscBalanceError(getErrorMessage(err));
        }
      } finally {
        if (currentReqId === bscReqSeq.current) {
          setLoadingBscBalance(false);
        }
      }
    },
    []
  );

  // Initial load of addresses and balances
  useEffect(() => {
    async function initDashboard() {
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
    void initDashboard();
  }, [onError, fetchBtcBalance, fetchBscBalance]);

  const copyToClipboard = (text: string, key: string) => {
    void navigator.clipboard.writeText(text);
    setCopiedKey(key);
    setTimeout(() => setCopiedKey(null), 2000);
  };

  const handleBtcNetworkChange = (network: 'mainnet' | 'signet') => {
    if (network === btcNetwork) return;
    setBtcNetwork(network);
    setBtcBalance(null);
    setBtcBalanceError(null);
    if (!addresses) return;
    const addr = network === 'mainnet' ? addresses.btcMainnet : addresses.btcSignet;
    void fetchBtcBalance(addr, network === 'signet');
  };

  const handleBscNetworkChange = (network: 'mainnet' | 'testnet') => {
    if (network === bscNetwork) return;
    setBscNetwork(network);
    setBscBalance(null);
    setBscBalanceError(null);
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
                title="Refresh on-chain balance"
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
              <>
                <div className="balance-values">
                  <div className="balance-primary">
                    {loadingBtcBalance && !btcBalance ? 'Loading…' : btcBalance ? btcBalance.formattedBtc : '—'}
                    <span className="balance-currency">BTC</span>
                  </div>
                  <div className="balance-secondary">
                    {btcBalance ? `${btcBalance.totalSatoshis.toLocaleString()} sats` : ''}
                    {btcBalance && btcBalance.unconfirmedSatoshis !== 0 && (
                      <span className="balance-pending ml-2">
                        {btcBalance.unconfirmedSatoshis > 0 ? '+' : ''}
                        {btcBalance.unconfirmedSatoshis.toLocaleString()} mempool
                      </span>
                    )}
                  </div>
                </div>

                <div className="text-[11px] text-slate-500 flex items-center gap-1 mt-1">
                  <Info size={12} />
                  <span>Showing balance for first receive address (0/0). UTXO aggregation in P2C.</span>
                </div>
              </>
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
                Mainnet (56)
              </button>
              <button
                type="button"
                className={`toggle-btn ${bscNetwork === 'testnet' ? 'active' : ''}`}
                onClick={() => handleBscNetworkChange('testnet')}
              >
                Testnet (97)
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
            <span className="contract-title">Active Contract:</span>
            <div className="contract-info">
              <span>{bscBalance ? bscBalance.usdt.name : (bscNetwork === 'mainnet' ? 'Binance-Peg BSC-USD' : 'BSC Testnet Mock USDT')}</span>
              <code className="contract-code">
                {bscBalance?.usdt.contractAddress ?? (bscNetwork === 'mainnet' ? addresses?.usdtBep20Contract : '0x337610d27c682E347C9cD60BD4b3b107C9d34dDd')}
              </code>
            </div>
          </div>

          {/* BSC Live Balance Section */}
          <div className="balance-box">
            <div className="balance-header">
              <span className="balance-title">
                <Coins size={14} /> Balances ({bscBalance ? bscBalance.networkName : (bscNetwork === 'mainnet' ? 'BSC Mainnet' : 'BSC Testnet')})
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
                  <span className="token-label">
                    {bscBalance ? bscBalance.usdt.symbol : 'USDT'} ({bscBalance ? bscBalance.usdt.name : 'Binance-Peg BSC-USD'})
                  </span>
                  <div className="token-amount">
                    {loadingBscBalance && !bscBalance ? 'Loading…' : bscBalance ? bscBalance.usdt.formatted : '—'}
                    <span className="balance-currency">USDT</span>
                  </div>
                  {bscBalance?.usdt.note && (
                    <span className="text-[10px] text-amber-400">{bscBalance.usdt.note}</span>
                  )}
                </div>

                <div className="token-subcard">
                  <span className="token-label">Native Gas (BNB)</span>
                  <div className="token-amount bnb-color">
                    {loadingBscBalance && !bscBalance ? 'Loading…' : bscBalance ? bscBalance.formattedBnb : '—'}
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
