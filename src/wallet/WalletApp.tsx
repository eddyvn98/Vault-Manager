import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { ShieldCheck, Lock, KeyRound, Copy, AlertTriangle } from 'lucide-react';

type Status = { exists: boolean; unlocked: boolean; signingEnabled: boolean; network: string };
type Action = 'create' | 'restore' | 'unlock';
const offline: Status = { exists: false, unlocked: false, signingEnabled: false, network: 'offline-only' };
const message = (error: unknown) => typeof error === 'string' ? error : 'Operation failed';

export default function WalletApp() {
  const [status, setStatus] = useState<Status>(offline);
  const [loading, setLoading] = useState(true);
  const [action, setAction] = useState<Action>('create');
  const [password, setPassword] = useState('');
  const [recovery, setRecovery] = useState('');
  const [shownPhrase, setShownPhrase] = useState('');
  const [acknowledged, setAcknowledged] = useState(false);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);

  async function refresh() {
    try {
      const next = await invoke<Status>('wallet_status');
      setStatus(next);
      if (next.exists) setAction('unlock');
    } catch (err) {
      setError(message(err));
    } finally { setLoading(false); }
  }
  useEffect(() => { void refresh(); }, []);
  useEffect(() => {
    const lockOnBackground = () => {
      if (document.visibilityState === 'hidden') {
        void invoke('lock_wallet').then(refresh).catch(() => {});
      }
    };
    document.addEventListener('visibilitychange', lockOnBackground);
    return () => document.removeEventListener('visibilitychange', lockOnBackground);
  }, []);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError('');
    try {
      if (action === 'create') {
        const words = await invoke<string>('create_wallet', { password });
        setShownPhrase(words);
      } else if (action === 'restore') {
        await invoke('restore_wallet', { password, recovery });
        setRecovery('');
      } else {
        await invoke('unlock_wallet', { password });
      }
      setPassword('');
      await refresh();
    } catch (err) { setError(message(err)); }
    finally { setBusy(false); }
  }

  const lock = async () => {
    setBusy(true);
    try {
      await invoke('lock_wallet');
      setShownPhrase('');
      setRecovery('');
      setAcknowledged(false);
      await refresh();
    } catch (err) { setError(message(err)); }
    finally { setBusy(false); }
  };

  return <main className="wallet-shell">
    <section className="wallet-card">
      <header className="wallet-header">
        <div className="brand"><ShieldCheck size={26}/> <strong>Vault Manager</strong></div>
        <span className="chip">P0–P1 · OFFLINE</span>
      </header>
      <h1>Personal Wallet</h1>
      <p className="muted">Independent encrypted vault on this device. BTC and USDT (BEP20) signing and broadcasting are disabled until later phases.</p>
      {error && <p className="error" role="alert">{error}</p>}
      {loading ? <p>Opening secure vault…</p> : shownPhrase ? <>
        <div className="warning"><AlertTriangle size={19}/> Write down these 24 recovery words offline. Never take a screenshot or upload them to any service.</div>
        <div className="phrase" aria-label="Recovery phrase">{shownPhrase}</div>
        <button type="button" className="subtle" onClick={() => void navigator.clipboard.writeText(shownPhrase)}><Copy size={16}/> Copy (not recommended)</button>
        <label className="check"><input type="checkbox" checked={acknowledged} onChange={e => setAcknowledged(e.target.checked)}/> I have saved the recovery phrase offline.</label>
        <button disabled={!acknowledged} onClick={() => { setShownPhrase(''); setAcknowledged(false); }}>Hide recovery phrase permanently</button>
      </> : status.unlocked ? <>
        <div className="success"><ShieldCheck size={20}/> Vault unlocked only in native Rust memory.</div>
        <div className="assets"><div><b>Bitcoin · BTC</b><span>Balance / transactions: unavailable in P1</span></div><div><b>BNB Smart Chain · USDT</b><span>Balance / transactions: unavailable in P1</span></div></div>
        <button disabled={busy} onClick={() => void lock()}><Lock size={16}/> Lock vault</button>
      </> : <form onSubmit={e => void submit(e)}>
        <nav className="tabs">
          {!status.exists && <><button type="button" className={action === 'create' ? 'active' : ''} onClick={() => setAction('create')}>Create</button><button type="button" className={action === 'restore' ? 'active' : ''} onClick={() => setAction('restore')}>Restore</button></>}
          {status.exists && <span><KeyRound size={16}/> Unlock existing wallet</span>}
        </nav>
        {action === 'restore' && !status.exists && <label>Recovery phrase (24 words)<textarea value={recovery} onChange={e => setRecovery(e.target.value)} autoComplete="off" spellCheck={false} rows={4} required/></label>}
        <label>Local encryption password (12+ characters)<input type="password" minLength={12} maxLength={1024} autoComplete="new-password" value={password} onChange={e => setPassword(e.target.value)} required/></label>
        <button type="submit" disabled={busy || password.length < 12}>{busy ? 'Processing…' : action === 'create' ? 'Create encrypted wallet' : action === 'restore' ? 'Restore on this device' : 'Unlock vault'}</button>
        <p className="muted small">Your password and recovery phrase are not sent to a server. There is no transaction signing in this release.</p>
      </form>}
    </section>
  </main>;
}
