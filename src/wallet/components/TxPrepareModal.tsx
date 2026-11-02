import React, { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  Send,
  AlertCircle,
  Copy,
  Check,
  CheckCircle2,
  Loader2,
  ExternalLink,
  ShieldCheck,
  Radio,
} from 'lucide-react';
import {
  BtcTxPreview,
  BscTxPreview,
  BtcSignedPreview,
  BscSignedPreview,
  PreflightReport,
  TxLifecycleStatus,
  getErrorMessage,
} from '../types';

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

  // Signed Previews
  const [btcSigned, setBtcSigned] = useState<BtcSignedPreview | null>(null);
  const [bscSigned, setBscSigned] = useState<BscSignedPreview | null>(null);

  // Broadcast & Lifecycle
  const [broadcastTxHash, setBroadcastTxHash] = useState<string | null>(null);
  const [lifecycle, setLifecycle] = useState<TxLifecycleStatus | null>(null);
  const [preflightReport, setPreflightReport] = useState<PreflightReport | null>(null);
  const [isSigning, setIsSigning] = useState(false);
  const [isPreflighting, setIsPreflighting] = useState(false);
  const [isBroadcasting, setIsBroadcasting] = useState(false);

  const activeDraftId = btcPreview?.draftId || bscPreview?.draftId;
  const isPollingRef = useRef(false);

  // Poll lifecycle status once broadcasted (with overlap guard)
  useEffect(() => {
    let timer: number | null = null;

    if (broadcastTxHash && activeDraftId) {
      const poll = async () => {
        if (isPollingRef.current) return;
        isPollingRef.current = true;
        try {
          const status = await invoke<TxLifecycleStatus>('query_transaction_lifecycle', {
            draftId: activeDraftId,
          });
          setLifecycle(status);
        } catch (e) {
          console.error('Lifecycle poll error:', e);
        } finally {
          isPollingRef.current = false;
        }
      };

      void poll();
      timer = window.setInterval(poll, 3000);
    }

    return () => {
      if (timer) clearInterval(timer);
    };
  }, [broadcastTxHash, activeDraftId]);

  const handlePrepare = async (e: React.FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setError(null);
    setBtcPreview(null);
    setBscPreview(null);
    setBtcSigned(null);
    setBscSigned(null);
    setPreflightReport(null);
    setBroadcastTxHash(null);
    setLifecycle(null);

    try {
      if (network === 'btc') {
        const cleanAmount = amount.trim();
        if (!/^\d+$/.test(cleanAmount)) {
          throw new Error('Vui lòng nhập số satoshi nguyên hợp lệ (chỉ gồm các chữ số)');
        }
        const satsBig = BigInt(cleanAmount);
        if (satsBig <= 0n) {
          throw new Error('Số satoshi phải lớn hơn 0');
        }
        if (satsBig > 21_000_000n * 100_000_000n) {
          throw new Error('Số tiền vượt quá tổng cung tối đa của Bitcoin (21M BTC)');
        }
        const satsNum = Number(satsBig);
        if (!Number.isSafeInteger(satsNum)) {
          throw new Error('Số tiền vượt quá giới hạn số an toàn của JavaScript');
        }

        const cleanRate = feeRate.trim() || '2';
        if (!/^\d+$/.test(cleanRate)) {
          throw new Error('Mức phí (sat/vB) phải là số nguyên hợp lệ');
        }
        const rateNum = Number(cleanRate);
        if (rateNum <= 0 || rateNum > 1000) {
          throw new Error('Mức phí phải nằm trong khoảng từ 1 đến 1000 sat/vB');
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
          throw new Error('Vui lòng nhập số lượng USDT muốn chuyển');
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

  const handleConfirmAndSign = async () => {
    setIsSigning(true);
    setError(null);
    try {
      if (network === 'btc' && btcPreview) {
        const signed = await invoke<BtcSignedPreview>('confirm_and_sign_btc_draft', {
          draftId: btcPreview.draftId,
        });
        setBtcSigned(signed);
      } else if (network === 'bsc' && bscPreview) {
        const signed = await invoke<BscSignedPreview>('confirm_and_sign_bsc_draft', {
          draftId: bscPreview.draftId,
        });
        setBscSigned(signed);
      }
    } catch (err) {
      setError(getErrorMessage(err));
    } finally {
      setIsSigning(false);
    }
  };

  const handlePreflight = async () => {
    setIsPreflighting(true);
    setError(null);
    try {
      if (!activeDraftId) throw new Error('Không tìm thấy giao dịch đã ký');
      const report = await invoke<PreflightReport>('preflight_signed_transaction', {
        draftId: activeDraftId,
      });
      setPreflightReport(report);
    } catch (err) {
      setError(getErrorMessage(err));
    } finally {
      setIsPreflighting(false);
    }
  };

  const handleBroadcast = async () => {
    setIsBroadcasting(true);
    setError(null);
    try {
      if (!activeDraftId) throw new Error('Không tìm thấy giao dịch đã ký');
      const txHash = await invoke<string>('broadcast_signed_transaction', {
        draftId: activeDraftId,
        enableTestnetBroadcast: true,
      });
      setBroadcastTxHash(txHash);
    } catch (err) {
      setError(getErrorMessage(err));
    } finally {
      setIsBroadcasting(false);
    }
  };

  const copyPayload = (payload: string) => {
    void navigator.clipboard.writeText(payload);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const explorerUrl =
    broadcastTxHash &&
    (network === 'btc'
      ? `https://mempool.space/signet/tx/${broadcastTxHash}`
      : `https://testnet.bscscan.com/tx/${broadcastTxHash}`);

  return (
    <div className="modal-backdrop">
      <div className="modal-card wide">
        <header className="modal-header">
          <Send size={22} className="text-sky-400" />
          <h3>Tạo & Ký Giao Dịch Testnet Thực Tế</h3>
        </header>

        <div className="preview-tabs">
          <button
            type="button"
            className={`tab-btn ${network === 'btc' ? 'active' : ''}`}
            onClick={() => {
              if (busy || isSigning || isBroadcasting) return;
              setNetwork('btc');
              setError(null);
              setBtcPreview(null);
              setBscPreview(null);
              setBtcSigned(null);
              setBscSigned(null);
              setBroadcastTxHash(null);
              setLifecycle(null);
            }}
          >
            Bitcoin Signet
          </button>
          <button
            type="button"
            className={`tab-btn ${network === 'bsc' ? 'active' : ''}`}
            onClick={() => {
              if (busy || isSigning || isBroadcasting) return;
              setNetwork('bsc');
              setError(null);
              setBtcPreview(null);
              setBscPreview(null);
              setBtcSigned(null);
              setBscSigned(null);
              setBroadcastTxHash(null);
              setLifecycle(null);
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

        {/* STEP 1: INPUT FORM */}
        {!btcPreview && !bscPreview && (
          <form onSubmit={handlePrepare} className="grid gap-3">
            <label className="text-xs text-slate-400">
              Địa chỉ nhận ({network === 'btc' ? 'Signet tb1q...' : 'BSC Testnet 0x...'})
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
                Số lượng ({network === 'btc' ? 'Satoshis (min 546)' : 'USDT e.g. 0.1'})
                <input
                  type="text"
                  placeholder={network === 'btc' ? '2000' : '0.1'}
                  value={amount}
                  onChange={(e) => setAmount(e.target.value)}
                  required
                />
              </label>

              {network === 'btc' ? (
                <label className="text-xs text-slate-400">
                  Phí mạng (sat/vB)
                  <input
                    type="number"
                    min="1"
                    value={feeRate}
                    onChange={(e) => setFeeRate(e.target.value)}
                  />
                </label>
              ) : (
                <label className="text-xs text-slate-400">
                  Mạng & Token
                  <input type="text" value="Binance-Peg USDT (Chain ID 97)" disabled />
                </label>
              )}
            </div>

            <div className="modal-actions">
              <button type="button" className="subtle" onClick={onClose} disabled={busy}>
                Hủy
              </button>
              <button type="submit" disabled={busy}>
                {busy ? 'Đang chuẩn bị…' : 'Xem trước giao dịch'}
              </button>
            </div>
          </form>
        )}

        {/* STEP 2: PREPARED PREVIEW & SIGNING FLOW */}
        {(btcPreview || bscPreview) && (
          <div className="grid gap-3">
            {/* BTC Preview Display */}
            {btcPreview && (
              <div className="preview-card">
                <div className="flex justify-between items-center">
                  <span className={`chip ${btcSigned ? 'status-badge-confirmed' : 'status-unlocked'}`}>
                    {btcSigned ? 'Đã ký an toàn' : 'Bản xem trước · Chưa ký'}
                  </span>
                  <span className="text-[11px] text-slate-400 font-mono">{btcPreview.draftId}</span>
                </div>

                <div className="preview-row">
                  <span className="preview-label">Người nhận:</span>
                  <span className="preview-value text-xs">{btcPreview.recipientAddress}</span>
                </div>
                <div className="preview-row">
                  <span className="preview-label">Số lượng:</span>
                  <span className="preview-value text-sky-400">
                    {btcPreview.formattedAmountBtc} BTC ({btcPreview.amountSatoshis.toLocaleString()} sats)
                  </span>
                </div>
                <div className="preview-row">
                  <span className="preview-label">Phí ({btcPreview.feeRateSatPerVb} sat/vB):</span>
                  <span className="preview-value text-amber-400">
                    {btcPreview.formattedFeeBtc} BTC ({btcPreview.feeSatoshis.toLocaleString()} sats)
                  </span>
                </div>
                <div className="preview-row">
                  <span className="preview-label">Địa chỉ tiền thối:</span>
                  <span className="preview-value text-xs">{btcPreview.changeAddress}</span>
                </div>
                <div className="preview-row">
                  <span className="preview-label">UTXO đầu vào:</span>
                  <span className="preview-value">{btcPreview.selectedUtxos.length} UTXO</span>
                </div>

                {!btcSigned && (
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
                )}
              </div>
            )}

            {/* BSC Preview Display */}
            {bscPreview && (
              <div className="preview-card">
                <div className="flex justify-between items-center">
                  <span className={`chip ${bscSigned ? 'status-badge-confirmed' : 'status-unlocked'}`}>
                    {bscSigned ? 'Đã ký an toàn' : 'Bản xem trước · Chưa ký'}
                  </span>
                  <span className="text-[11px] text-slate-400 font-mono">{bscPreview.draftId}</span>
                </div>

                <div className="preview-row">
                  <span className="preview-label">Người gửi:</span>
                  <span className="preview-value text-xs">{bscPreview.fromAddress}</span>
                </div>
                <div className="preview-row">
                  <span className="preview-label">Người nhận:</span>
                  <span className="preview-value text-xs">{bscPreview.recipientAddress}</span>
                </div>
                <div className="preview-row">
                  <span className="preview-label">Token Transfer:</span>
                  <span className="preview-value text-emerald-400">
                    {bscPreview.formattedAmount} {bscPreview.tokenSymbol}
                  </span>
                </div>
                <div className="preview-row">
                  <span className="preview-label">Nonce / Gas Limit:</span>
                  <span className="preview-value">{bscPreview.nonce} / {bscPreview.gasLimit.toLocaleString()}</span>
                </div>
                <div className="preview-row">
                  <span className="preview-label">Ước tính phí gas:</span>
                  <span className="preview-value text-amber-400">
                    {bscPreview.estimatedFeeBnb} BNB ({bscPreview.gasPriceGwei} Gwei)
                  </span>
                </div>

                {!bscSigned && (
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
                )}
              </div>
            )}

            {/* SIGNED CONFIRMATION DETAILS */}
            {(btcSigned || bscSigned) && (
              <div className="preview-card border-emerald-500/30 bg-emerald-950/10">
                <div className="flex items-center gap-2 text-emerald-400 text-xs font-bold">
                  <ShieldCheck size={16} />
                  <span>Giao dịch đã được ký cục bộ thành công!</span>
                </div>

                {btcSigned && (
                  <>
                    <div className="preview-row">
                      <span className="preview-label">Mã giao dịch (TxID):</span>
                      <span className="preview-value text-xs text-sky-300">{btcSigned.txid}</span>
                    </div>
                    <div className="preview-row">
                      <span className="preview-label">Witness TxID (WTXID):</span>
                      <span className="preview-value text-xs text-slate-400">{btcSigned.wtxid}</span>
                    </div>
                  </>
                )}

                {bscSigned && (
                  <div className="preview-row">
                    <span className="preview-label">EVM Tx Hash (Keccak-256):</span>
                    <span className="preview-value text-xs text-emerald-300">{bscSigned.txHash}</span>
                  </div>
                )}
              </div>
            )}

            {/* PREFLIGHT DRY-RUN REPORT */}
            {preflightReport && (
              <div className="preview-card border-emerald-500/40 bg-emerald-950/20">
                <div className="flex items-center gap-2 text-emerald-400 text-xs font-bold mb-1">
                  <ShieldCheck size={16} />
                  <span>Kết quả Kiểm tra Dry-Run: Giao dịch hợp lệ 100%</span>
                </div>
                <div className="text-[11px] text-slate-300 mb-2">
                  Trạng thái bảo toàn: <span className="font-mono text-emerald-300 font-semibold">{preflightReport.status}</span> (Chưa phát sóng lên mạng, an toàn tuyệt đối).
                </div>
                <div className="space-y-1 bg-black/40 p-2.5 rounded border border-emerald-500/20 text-[11px]">
                  {preflightReport.checks.map((c, i) => (
                    <div key={i} className="flex items-start gap-1.5 text-slate-300">
                      <Check size={12} className="text-emerald-400 shrink-0 mt-0.5" />
                      <span>{c}</span>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* BROADCAST & LIFECYCLE TRACKER */}
            {broadcastTxHash && (
              <div className="preview-card border-sky-500/30 bg-sky-950/10">
                <div className="flex justify-between items-center">
                  <span className="text-xs font-bold text-sky-400 flex items-center gap-2">
                    <Radio size={14} className="animate-pulse" /> Trạng thái phát sóng On-chain
                  </span>
                  {lifecycle && (
                    <span
                      className={`status-badge-${
                        lifecycle.status === 'confirmed'
                          ? 'confirmed'
                          : lifecycle.status === 'failed'
                          ? 'failed'
                          : 'pending'
                      }`}
                    >
                      {lifecycle.status === 'confirmed' && (
                        <>
                          <CheckCircle2 size={12} /> Đã xác nhận on-chain (
                          {lifecycle.details.confirmations} block)
                        </>
                      )}
                      {lifecycle.status === 'pending' && (
                        <>
                          <Loader2 size={12} className="animate-spin" /> Đang chờ thợ đào đóng block
                        </>
                      )}
                      {lifecycle.status === 'broadcasting' && (
                        <>
                          <Loader2 size={12} className="animate-spin" /> Đang gửi tới node testnet...
                        </>
                      )}
                      {lifecycle.status === 'failed' && (
                        <>
                          <AlertCircle size={12} /> Thất bại: {lifecycle.details.errorMessage}
                        </>
                      )}
                      {lifecycle.status === 'dropped' && (
                        <>
                          <AlertCircle size={12} /> Bị loại khỏi mempool: {lifecycle.details.reason}
                        </>
                      )}
                      {lifecycle.status === 'reorgDetected' && (
                        <>
                          <AlertCircle size={12} /> Phát hiện Reorg chuỗi!
                        </>
                      )}
                    </span>
                  )}
                </div>

                <div className="preview-row">
                  <span className="preview-label">Hash phát sóng:</span>
                  <div className="flex items-center gap-2">
                    <span className="preview-value text-xs text-sky-300 font-mono truncate max-w-[260px]">
                      {broadcastTxHash}
                    </span>
                    <button
                      type="button"
                      className="mini-action-btn"
                      onClick={() => copyPayload(broadcastTxHash)}
                    >
                      {copied ? <Check size={11} className="text-emerald-400" /> : <Copy size={11} />}
                    </button>
                  </div>
                </div>

                {explorerUrl && (
                  <div className="mt-1">
                    <a
                      href={explorerUrl}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="text-xs text-sky-400 hover:text-sky-300 inline-flex items-center gap-1 font-semibold underline underline-offset-2"
                    >
                      Xem trên {network === 'btc' ? 'Mempool Signet Explorer' : 'BscScan Testnet'}
                      <ExternalLink size={12} />
                    </a>
                  </div>
                )}
              </div>
            )}

            {/* ACTION BUTTONS */}
            <div className="modal-actions">
              {!btcSigned && !bscSigned ? (
                <>
                  <button
                    type="button"
                    className="subtle"
                    onClick={() => {
                      setBtcPreview(null);
                      setBscPreview(null);
                    }}
                    disabled={isSigning}
                  >
                    Quay lại
                  </button>
                  <button
                    type="button"
                    className="sign-btn"
                    onClick={handleConfirmAndSign}
                    disabled={isSigning}
                  >
                    {isSigning ? (
                      <span className="flex items-center gap-2">
                        <Loader2 size={14} className="animate-spin" /> Đang ký cục bộ…
                      </span>
                    ) : (
                      'Ký giao dịch'
                    )}
                  </button>
                </>
              ) : !broadcastTxHash ? (
                <>
                  <button
                    type="button"
                    className="subtle"
                    onClick={onClose}
                    disabled={isBroadcasting || isPreflighting}
                  >
                    Đóng
                  </button>
                  <button
                    type="button"
                    className="subtle text-sky-400"
                    onClick={handlePreflight}
                    disabled={isBroadcasting || isPreflighting}
                    title="Xác thực raw transaction và chữ ký mà không gửi lên blockchain (bảo toàn trạng thái Signed)"
                  >
                    {isPreflighting ? (
                      <span className="flex items-center gap-2">
                        <Loader2 size={14} className="animate-spin" /> Đang kiểm tra…
                      </span>
                    ) : (
                      'Kiểm tra Dry-Run'
                    )}
                  </button>
                  <button
                    type="button"
                    className="broadcast-btn"
                    onClick={handleBroadcast}
                    disabled={isBroadcasting || isPreflighting}
                  >
                    {isBroadcasting ? (
                      <span className="flex items-center gap-2">
                        <Loader2 size={14} className="animate-spin" /> Đang phát sóng…
                      </span>
                    ) : (
                      'Phát sóng giao dịch Testnet'
                    )}
                  </button>
                </>
              ) : (
                <button type="button" onClick={onClose}>
                  Hoàn tất & Đóng
                </button>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
