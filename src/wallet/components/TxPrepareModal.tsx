import React, { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Send, AlertCircle, Copy, Check, FileCode, ShieldAlert, ArrowRight } from 'lucide-react';
import { BtcTxPreview, BscTxPreview, getErrorMessage } from '../types';

interface TxPrepareModalProps {
  onClose: () => void;
  defaultNetwork?: 'btc' | 'bsc';
}

export default function TxPrepareModal({ onClose, defaultNetwork = 'btc' }: TxPrepareModalProps) {
  const [network, setNetwork] = useState<'btc' | 'bsc'>(defaultNetwork);
  const [recipient, setRecipient] = useState('');
  const [amount, setAmount] = useState('');
  const [feeRate, setFeeRate] = useState('2');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  // Previews
  const [btcPreview, setBtcPreview] = useState<BtcTxPreview | null>(null);
  const [bscPreview, setBscPreview] = useState<BscTxPreview | null>(null);

  const handlePrepare = async (e: React.FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setError(null);
    setBtcPreview(null);
    setBscPreview(null);

    try {
      if (network === 'btc') {
        const cleanAmount = amount.trim();
        if (!/^\d+$/.test(cleanAmount)) {
          throw new Error('Please enter a valid whole satoshi amount integer (digits only)');
        }
        const satsBig = BigInt(cleanAmount);
        if (satsBig <= 0n) {
          throw new Error('Satoshi amount must be greater than zero');
        }
        if (satsBig > 21_000_000n * 100_000_000n) {
          throw new Error('Amount exceeds maximum Bitcoin supply (21M BTC)');
        }
        const satsNum = Number(satsBig);
        if (!Number.isSafeInteger(satsNum)) {
          throw new Error('Amount exceeds JavaScript safe integer limit');
        }

        const cleanRate = feeRate.trim() || '2';
        if (!/^\d+$/.test(cleanRate)) {
          throw new Error('Fee rate must be a valid whole number (sat/vB)');
        }
        const rateNum = Number(cleanRate);
        if (rateNum <= 0 || rateNum > 1000) {
          throw new Error('Fee rate must be between 1 and 1000 sat/vB');
        }

        const res = await invoke<BtcTxPreview>('prepare_btc_transaction', {
          recipient: recipient.trim(),
          amountSatoshis: satsNum,
          feeRateSatVb: rateNum,
        });
        setBtcPreview(res);
      } else {
        const cleanRecipient = recipient.trim();
        const cleanAmount = amount.trim();
        if (!cleanAmount) {
          throw new Error('Please enter a token amount');
        }
        const res = await invoke<BscTxPreview>('prepare_bsc_transaction', {
          recipient: cleanRecipient,
          amountTokens: cleanAmount,
        });
        setBscPreview(res);
      }
    } catch (err) {
      setError(getErrorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const copyPayload = (payload: string) => {
    void navigator.clipboard.writeText(payload);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="modal-backdrop">
      <div className="modal-card wide">
        <header className="modal-header">
          <Send size={22} className="text-sky-400" />
          <h3>Transaction Preparation (P2C-1 Testnet Preview)</h3>
        </header>

        <div className="preview-tabs">
          <button
            type="button"
            className={`tab-btn ${network === 'btc' ? 'active' : ''}`}
            onClick={() => {
              setNetwork('btc');
              setError(null);
              setBtcPreview(null);
              setBscPreview(null);
            }}
          >
            Bitcoin Signet
          </button>
          <button
            type="button"
            className={`tab-btn ${network === 'bsc' ? 'active' : ''}`}
            onClick={() => {
              setNetwork('bsc');
              setError(null);
              setBtcPreview(null);
              setBscPreview(null);
            }}
          >
            BSC Testnet (USDT)
          </button>
        </div>

        {error && (
          <div className="error text-xs flex items-center gap-2" role="alert">
            <AlertCircle size={14} className="shrink-0" />
            <span>{error}</span>
          </div>
        )}

        {!btcPreview && !bscPreview ? (
          <form onSubmit={handlePrepare} className="grid gap-3">
            <label className="text-xs text-slate-400">
              Recipient Address ({network === 'btc' ? 'Signet tb1q...' : 'BSC Testnet 0x...'})
              <input
                type="text"
                placeholder={network === 'btc' ? 'tb1q...' : '0x...'}
                value={recipient}
                onChange={(e) => setRecipient(e.target.value)}
                required
              />
            </label>

            <div className="grid grid-cols-2 gap-3">
              <label className="text-xs text-slate-400">
                Amount ({network === 'btc' ? 'Satoshis (min 546)' : 'USDT e.g. 1.5'})
                <input
                  type="text"
                  placeholder={network === 'btc' ? '10000' : '1.5'}
                  value={amount}
                  onChange={(e) => setAmount(e.target.value)}
                  required
                />
              </label>

              {network === 'btc' ? (
                <label className="text-xs text-slate-400">
                  Fee Rate (sat/vB)
                  <input
                    type="number"
                    min="1"
                    value={feeRate}
                    onChange={(e) => setFeeRate(e.target.value)}
                  />
                </label>
              ) : (
                <label className="text-xs text-slate-400">
                  Network Token
                  <input type="text" value="Binance-Peg Mock USDT (Chain 97)" disabled />
                </label>
              )}
            </div>

            <div className="modal-actions">
              <button type="button" className="subtle" onClick={onClose} disabled={busy}>
                Cancel
              </button>
              <button type="submit" disabled={busy}>
                {busy ? 'Preparing…' : 'Generate Preview'}
              </button>
            </div>
          </form>
        ) : (
          <div className="grid gap-3">
            {/* BTC Preview Display */}
            {btcPreview && (
              <div className="preview-card">
                <div className="flex justify-between items-center">
                  <span className="chip status-unlocked">Bản xem trước · Chưa ký</span>
                  <span className="text-[11px] text-slate-400 font-mono">{btcPreview.draftId}</span>
                </div>

                <div className="preview-row">
                  <span className="preview-label">Recipient:</span>
                  <span className="preview-value text-xs">{btcPreview.recipientAddress}</span>
                </div>
                <div className="preview-row">
                  <span className="preview-label">Amount:</span>
                  <span className="preview-value text-sky-400">{btcPreview.formattedAmountBtc} BTC ({btcPreview.amountSatoshis.toLocaleString()} sats)</span>
                </div>
                <div className="preview-row">
                  <span className="preview-label">Fee ({btcPreview.feeRateSatPerVb} sat/vB):</span>
                  <span className="preview-value text-amber-400">{btcPreview.formattedFeeBtc} BTC ({btcPreview.feeSatoshis.toLocaleString()} sats)</span>
                </div>
                <div className="preview-row">
                  <span className="preview-label">Change Address:</span>
                  <span className="preview-value text-xs">{btcPreview.changeAddress}</span>
                </div>
                <div className="preview-row">
                  <span className="preview-label">Inputs Selected:</span>
                  <span className="preview-value">{btcPreview.selectedUtxos.length} UTXO(s)</span>
                </div>

                <div>
                  <div className="flex justify-between items-center mb-1 text-[11px] text-slate-400">
                    <span>Unsigned PSBT Payload:</span>
                    <button
                      type="button"
                      className="mini-action-btn"
                      onClick={() => copyPayload(btcPreview.unsignedPsbtBase64)}
                    >
                      {copied ? <Check size={11} className="text-emerald-400" /> : <Copy size={11} />} Copy
                    </button>
                  </div>
                  <div className="preview-code-box">{btcPreview.unsignedPsbtBase64}</div>
                </div>
              </div>
            )}

            {/* BSC Preview Display */}
            {bscPreview && (
              <div className="preview-card">
                <div className="flex justify-between items-center">
                  <span className="chip status-unlocked">Bản xem trước · Chưa ký</span>
                  <span className="text-[11px] text-slate-400 font-mono">{bscPreview.draftId}</span>
                </div>

                <div className="preview-row">
                  <span className="preview-label">From:</span>
                  <span className="preview-value text-xs">{bscPreview.fromAddress}</span>
                </div>
                <div className="preview-row">
                  <span className="preview-label">Recipient:</span>
                  <span className="preview-value text-xs">{bscPreview.recipientAddress}</span>
                </div>
                <div className="preview-row">
                  <span className="preview-label">Token Transfer:</span>
                  <span className="preview-value text-emerald-400">{bscPreview.formattedAmount} {bscPreview.tokenSymbol}</span>
                </div>
                <div className="preview-row">
                  <span className="preview-label">Nonce / Gas Limit:</span>
                  <span className="preview-value">{bscPreview.nonce} / {bscPreview.gasLimit.toLocaleString()}</span>
                </div>
                <div className="preview-row">
                  <span className="preview-label">Estimated Gas Fee:</span>
                  <span className="preview-value text-amber-400">{bscPreview.estimatedFeeBnb} BNB ({bscPreview.gasPriceGwei} Gwei)</span>
                </div>

                <div>
                  <div className="flex justify-between items-center mb-1 text-[11px] text-slate-400">
                    <span>ERC-20 Calldata (transfer):</span>
                    <button
                      type="button"
                      className="mini-action-btn"
                      onClick={() => copyPayload(bscPreview.calldataHex)}
                    >
                      {copied ? <Check size={11} className="text-emerald-400" /> : <Copy size={11} />} Copy
                    </button>
                  </div>
                  <div className="preview-code-box">{bscPreview.calldataHex}</div>
                </div>
              </div>
            )}

            <div className="modal-actions">
              <button
                type="button"
                className="disabled-sign-btn"
                disabled
                title="P2C-1 Scope: Signing is not enabled until P2C-2 offline test vectors are approved"
              >
                Ký giao dịch — Chưa kích hoạt (P2C-1 Preview Only)
              </button>
              <button type="button" onClick={onClose}>
                Close
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
