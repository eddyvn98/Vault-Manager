import React, { useEffect, useState, useRef, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  ShieldCheck,
  Lock,
  KeyRound,
  Copy,
  AlertTriangle,
  RotateCcw,
  CheckCircle2,
  Trash2,
  Clock,
} from 'lucide-react';

type Status = {
  exists: boolean;
  unlocked: boolean;
  signingEnabled: boolean;
  network: string;
  failedAttempts: number;
  lockoutSeconds: number;
};

type Action = 'create' | 'restore' | 'unlock';

const offline: Status = {
  exists: false,
  unlocked: false,
  signingEnabled: false,
  network: 'offline-only',
  failedAttempts: 0,
  lockoutSeconds: 0,
};

const message = (error: unknown) =>
  typeof error === 'string' ? error : error instanceof Error ? error.message : 'Operation failed';

export default function WalletApp() {
  const [status, setStatus] = useState<Status>(offline);
  const [loading, setLoading] = useState(true);
  const [action, setAction] = useState<Action>('create');
  const [password, setPassword] = useState('');
  const [recovery, setRecovery] = useState('');
  const [shownPhrase, setShownPhrase] = useState('');
  const [error, setError] = useState('');
  const [successMsg, setSuccessMsg] = useState('');
  const [busy, setBusy] = useState(false);
  const [cooldown, setCooldown] = useState(0);

  // Backup verification quiz state
  const [quizIndices, setQuizIndices] = useState<number[]>([]);
  const [quizInputs, setQuizInputs] = useState<Record<number, string>>({});
  const [quizPassed, setQuizPassed] = useState(false);

  const inactivityTimerRef = useRef<number | null>(null);

  const phraseWords = useMemo(() => {
    if (!shownPhrase) return [];
    return shownPhrase.trim().split(/\s+/);
  }, [shownPhrase]);

  // Generate 3 random word indices for backup verification
  useEffect(() => {
    if (phraseWords.length === 24) {
      const idxs: number[] = [];
      while (idxs.length < 3) {
        const r = Math.floor(Math.random() * 24);
        if (!idxs.includes(r)) idxs.push(r);
      }
      idxs.sort((a, b) => a - b);
      setQuizIndices(idxs);
      setQuizInputs({});
      setQuizPassed(false);
    }
  }, [phraseWords]);

  async function refresh() {
    try {
      const next = await invoke<Status>('wallet_status');
      setStatus(next);
      setCooldown(next.lockoutSeconds);
      if (next.exists && !next.unlocked) {
        setAction('unlock');
      } else if (!next.exists) {
        setAction('create');
      }
    } catch (err) {
      setError(message(err));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void refresh();
  }, []);

  // Cooldown countdown timer
  useEffect(() => {
    if (cooldown <= 0) return;
    const interval = window.setInterval(() => {
      setCooldown((prev) => {
        if (prev <= 1) {
          clearInterval(interval);
          return 0;
        }
        return prev - 1;
      });
    }, 1000);
    return () => clearInterval(interval);
  }, [cooldown]);

  // Lock on window blur / background visibility change
  useEffect(() => {
    const lockOnBackground = () => {
      if (document.visibilityState === 'hidden' && status.unlocked) {
        void invoke('lock_wallet').then(refresh).catch(() => {});
      }
    };
    document.addEventListener('visibilitychange', lockOnBackground);
    return () => document.removeEventListener('visibilitychange', lockOnBackground);
  }, [status.unlocked]);

  // Inactivity auto-lock (5 minutes)
  const resetInactivityTimer = () => {
    if (inactivityTimerRef.current) {
      window.clearTimeout(inactivityTimerRef.current);
    }
    if (status.unlocked) {
      inactivityTimerRef.current = window.setTimeout(() => {
        void invoke('lock_wallet')
          .then(() => {
            void refresh();
            setError('Wallet locked automatically due to inactivity.');
          })
          .catch(() => {});
      }, 5 * 60 * 1000);
    }
  };

  useEffect(() => {
    if (!status.unlocked) return;
    resetInactivityTimer();
    const events = ['mousedown', 'mousemove', 'keydown', 'scroll', 'touchstart'];
    events.forEach((ev) => window.addEventListener(ev, resetInactivityTimer, { passive: true }));
    return () => {
      if (inactivityTimerRef.current) window.clearTimeout(inactivityTimerRef.current);
      events.forEach((ev) => window.removeEventListener(ev, resetInactivityTimer));
    };
  }, [status.unlocked]);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError('');
    setSuccessMsg('');
    try {
      if (action === 'create') {
        const words = await invoke<string>('create_wallet', { password });
        setShownPhrase(words);
      } else if (action === 'restore') {
        await invoke('restore_wallet', { password, recovery });
        setRecovery('');
        setSuccessMsg('Wallet restored successfully on this device. Please unlock to continue.');
      } else {
        await invoke('unlock_wallet', { password });
      }
      setPassword('');
      await refresh();
    } catch (err) {
      setError(message(err));
      await refresh();
    } finally {
      setBusy(false);
    }
  }

  const lock = async () => {
    setBusy(true);
    setError('');
    try {
      await invoke('lock_wallet');
      setShownPhrase('');
      setRecovery('');
      await refresh();
    } catch (err) {
      setError(message(err));
    } finally {
      setBusy(false);
    }
  };

  const handleReset = async () => {
    const confirmed = window.confirm(
      'WARNING: This will permanently delete the local encrypted wallet file from this device. ' +
        'Ensure you have your 24-word recovery phrase saved offline if you wish to restore it later. Proceed?'
    );
    if (!confirmed) return;

    setBusy(true);
    setError('');
    try {
      await invoke('reset_wallet');
      setShownPhrase('');
      setRecovery('');
      setPassword('');
      setSuccessMsg('Wallet has been reset. You can now create or restore a wallet.');
      await refresh();
    } catch (err) {
      setError(message(err));
    } finally {
      setBusy(false);
    }
  };

  const verifyQuiz = () => {
    let allValid = true;
    for (const idx of quizIndices) {
      const expected = phraseWords[idx]?.toLowerCase().trim();
      const actual = (quizInputs[idx] || '').toLowerCase().trim();
      if (!actual || actual !== expected) {
        allValid = false;
        break;
      }
    }
    if (allValid) {
      setQuizPassed(true);
      setError('');
    } else {
      setError('One or more verification words do not match. Please verify against your written backup.');
    }
  };

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
          Independent local cryptographic vault. Private keys are derived solely inside native memory
          and never leave this device.
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

        {cooldown > 0 && (
          <div className="warning">
            <Clock size={18} />{' '}
            <span>
              Brute-force protection active. Please wait <strong>{cooldown}s</strong> before retry.
            </span>
          </div>
        )}

        {loading ? (
          <p className="muted">Initializing local secure vault…</p>
        ) : shownPhrase ? (
          <div className="creation-flow">
            <div className="warning">
              <AlertTriangle size={20} />
              <div>
                <strong>Backup Required:</strong> Write down these 24 words in exact order on paper. Never store
                in digital notes, cloud storage, or screenshots.
              </div>
            </div>

            <div className="phrase-grid">
              {phraseWords.map((word, i) => (
                <div key={i} className="word-badge">
                  <span className="word-num">{i + 1}</span>
                  <span className="word-text">{word}</span>
                </div>
              ))}
            </div>

            <div className="quiz-section">
              <h3>Verify Your Backup</h3>
              <p className="muted small">
                Confirm words #{quizIndices.map((i) => i + 1).join(', #')} from your written record:
              </p>
              <div className="quiz-inputs">
                {quizIndices.map((idx) => (
                  <label key={idx} className="quiz-label">
                    <span>Word #{idx + 1}</span>
                    <input
                      type="text"
                      autoComplete="off"
                      spellCheck={false}
                      value={quizInputs[idx] || ''}
                      onChange={(e) =>
                        setQuizInputs((prev) => ({ ...prev, [idx]: e.target.value }))
                      }
                      placeholder={`Enter word #${idx + 1}`}
                      disabled={quizPassed}
                    />
                  </label>
                ))}
              </div>

              {!quizPassed ? (
                <button type="button" className="subtle" onClick={verifyQuiz}>
                  Verify Backup Words
                </button>
              ) : (
                <div className="success">
                  <CheckCircle2 size={18} /> <span>Backup successfully verified!</span>
                </div>
              )}
            </div>

            <div className="phrase-actions">
              <button
                type="button"
                className="subtle"
                onClick={() => void navigator.clipboard.writeText(shownPhrase)}
              >
                <Copy size={16} /> Copy to clipboard (insecure)
              </button>
              <button
                disabled={!quizPassed}
                onClick={() => {
                  setShownPhrase('');
                  setQuizPassed(false);
                  setSuccessMsg('Wallet secured. You can now unlock your vault.');
                }}
              >
                Hide recovery phrase & complete setup
              </button>
            </div>
          </div>
        ) : status.unlocked ? (
          <div className="unlocked-dashboard">
            <div className="success">
              <ShieldCheck size={20} />{' '}
              <span>Vault is unlocked in native memory (Argon2id + XChaCha20-Poly1305).</span>
            </div>

            <div className="assets">
              <div className="asset-card">
                <div className="asset-title">
                  <span className="asset-symbol">BTC</span>
                  <strong>Bitcoin (SegWit Native / P2WPKH)</strong>
                </div>
                <span className="asset-sub">Key derivation & signing engine scheduled for P2</span>
              </div>
              <div className="asset-card">
                <div className="asset-title">
                  <span className="asset-symbol">USDT</span>
                  <strong>Tether USD (BNB Smart Chain BEP-20)</strong>
                </div>
                <span className="asset-sub">BIP44 m/44'/60' EVM & BEP20 signing scheduled for P2</span>
              </div>
            </div>

            <div className="dashboard-actions">
              <button disabled={busy} onClick={() => void lock()}>
                <Lock size={16} /> Lock Vault
              </button>
              <button type="button" className="danger-btn" onClick={() => void handleReset()}>
                <Trash2 size={16} /> Reset Vault
              </button>
            </div>
          </div>
        ) : (
          <form onSubmit={(e) => void submit(e)}>
            <nav className="tabs">
              {!status.exists ? (
                <>
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
                </>
              ) : (
                <div className="unlock-header">
                  <span>
                    <KeyRound size={16} /> Vault encrypted on this device
                  </span>
                  {status.failedAttempts > 0 && (
                    <span className="failed-badge">
                      {status.failedAttempts} failed attempt{status.failedAttempts > 1 ? 's' : ''}
                    </span>
                  )}
                </div>
              )}
            </nav>

            {action === 'restore' && !status.exists && (
              <label>
                24-Word Recovery Phrase
                <textarea
                  value={recovery}
                  onChange={(e) => setRecovery(e.target.value)}
                  autoComplete="off"
                  spellCheck={false}
                  rows={4}
                  placeholder="word1 word2 word3 ... word24"
                  required
                />
              </label>
            )}

            <label>
              Master Password (12+ characters)
              <input
                type="password"
                minLength={12}
                maxLength={1024}
                autoComplete="current-password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder="Enter strong local password"
                disabled={cooldown > 0}
                required
              />
            </label>

            <button
              type="submit"
              disabled={busy || password.length < 12 || cooldown > 0}
            >
              {busy ? (
                'Processing Argon2id…'
              ) : cooldown > 0 ? (
                `Cooldown active (${cooldown}s)`
              ) : action === 'create' ? (
                'Create Encrypted Wallet'
              ) : action === 'restore' ? (
                'Restore on this Device'
              ) : (
                'Unlock Vault'
              )}
            </button>

            {status.exists && (
              <div className="reset-row">
                <button
                  type="button"
                  className="link-btn"
                  onClick={() => void handleReset()}
                  disabled={busy}
                >
                  <RotateCcw size={14} /> Reset or re-import wallet on this device
                </button>
              </div>
            )}

            <p className="muted small">
              Vault data is stored with unique Argon2id salt and XChaCha20-Poly1305 nonce. No network
              transmissions or telemetry.
            </p>
          </form>
        )}
      </section>
    </main>
  );
}
