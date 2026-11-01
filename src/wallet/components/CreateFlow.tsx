import React, { useState, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { AlertTriangle, CheckCircle2, ArrowLeft, ShieldCheck } from 'lucide-react';
import { getErrorMessage } from '../types';

interface CreateFlowProps {
  onSuccess: () => void;
  onError: (msg: string) => void;
}

type Stage = 'enter_password' | 'show_phrase' | 'quiz_backup';

export default function CreateFlow({ onSuccess, onError }: CreateFlowProps) {
  const [stage, setStage] = useState<Stage>('enter_password');
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [phrase, setPhrase] = useState('');
  const [busy, setBusy] = useState(false);

  // Quiz state
  const [quizIndices, setQuizIndices] = useState<number[]>([]);
  const [quizInputs, setQuizInputs] = useState<Record<number, string>>({});
  const [quizPassed, setQuizPassed] = useState(false);
  const [quizError, setQuizError] = useState('');

  const words = useMemo(() => {
    return phrase ? phrase.trim().split(/\s+/) : [];
  }, [phrase]);

  const handleStartCreation = async (e: React.FormEvent) => {
    e.preventDefault();
    if (password.length < 12) {
      onError('Password must be at least 12 characters');
      return;
    }
    if (password !== confirmPassword) {
      onError('Passwords do not match');
      return;
    }

    setBusy(true);
    onError('');
    try {
      const generated = await invoke<string>('start_wallet_creation');
      setPhrase(generated);

      // Pick 3 unique random words to verify
      const idxs: number[] = [];
      while (idxs.length < 3) {
        const r = Math.floor(Math.random() * 24);
        if (!idxs.includes(r)) idxs.push(r);
      }
      idxs.sort((a, b) => a - b);
      setQuizIndices(idxs);
      setQuizInputs({});
      setQuizPassed(false);
      setQuizError('');

      setStage('show_phrase');
    } catch (err) {
      onError(getErrorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const handleVerifyQuiz = () => {
    let allValid = true;
    for (const idx of quizIndices) {
      const expected = words[idx]?.toLowerCase().trim();
      const actual = (quizInputs[idx] || '').toLowerCase().trim();
      if (!actual || actual !== expected) {
        allValid = false;
        break;
      }
    }

    if (allValid) {
      setQuizPassed(true);
      setQuizError('');
    } else {
      setQuizError('Verification failed. One or more words do not match your written notes.');
    }
  };

  const handleFinalize = async () => {
    setBusy(true);
    onError('');
    try {
      await invoke('finalize_wallet_creation', {
        password,
        recovery: phrase,
      });
      // Zero out memory
      setPassword('');
      setConfirmPassword('');
      setPhrase('');
      onSuccess();
    } catch (err) {
      onError(getErrorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  if (stage === 'enter_password') {
    return (
      <form onSubmit={handleStartCreation} className="create-password-form">
        <label>
          Master Encryption Password (12+ characters)
          <input
            type="password"
            minLength={12}
            maxLength={1024}
            autoComplete="new-password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            placeholder="At least 12 characters"
            required
          />
        </label>
        <label>
          Confirm Master Password
          <input
            type="password"
            minLength={12}
            maxLength={1024}
            autoComplete="new-password"
            value={confirmPassword}
            onChange={(e) => setConfirmPassword(e.target.value)}
            placeholder="Re-enter password"
            required
          />
        </label>
        <button type="submit" disabled={busy || password.length < 12}>
          {busy ? 'Generating BIP39 Entropy…' : 'Generate Secure 24-Word Phrase'}
        </button>
        <p className="muted small">
          The vault file will NOT be created on disk until you successfully verify your paper backup in the next steps.
        </p>
      </form>
    );
  }

  if (stage === 'show_phrase') {
    return (
      <div className="creation-flow">
        <div className="warning">
          <AlertTriangle size={20} />
          <div>
            <strong>Physical Paper Backup Required:</strong> Write down each word in the numbered order below on physical paper. Never save digitally or take screenshots.
          </div>
        </div>

        <div className="phrase-grid">
          {words.map((word, i) => (
            <div key={i} className="word-badge">
              <span className="word-num">{i + 1}</span>
              <span className="word-text">{word}</span>
            </div>
          ))}
        </div>

        <div className="phrase-actions">
          <button type="button" onClick={() => setStage('quiz_backup')}>
            I have written down all 24 words → Proceed to Verification Quiz
          </button>
          <button
            type="button"
            className="subtle"
            onClick={() => {
              setStage('enter_password');
              setPhrase('');
            }}
          >
            <ArrowLeft size={16} /> Cancel & re-choose password
          </button>
        </div>
      </div>
    );
  }

  // stage === 'quiz_backup' (24 words are HIDDEN to verify actual paper notes)
  return (
    <div className="creation-flow">
      <div className="quiz-section">
        <h3>Verify Offline Backup</h3>
        <p className="muted small">
          To ensure you have recorded the phrase on paper, enter words #{quizIndices.map((i) => i + 1).join(', #')} without viewing the screen:
        </p>

        {quizError && (
          <div className="error" role="alert">
            <AlertTriangle size={18} /> <span>{quizError}</span>
          </div>
        )}

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
          <button type="button" className="subtle" onClick={handleVerifyQuiz}>
            Check Words
          </button>
        ) : (
          <div className="success">
            <CheckCircle2 size={18} /> <span>Backup confirmed! Ready to encrypt and store vault.</span>
          </div>
        )}
      </div>

      <div className="phrase-actions">
        <button
          disabled={!quizPassed || busy}
          onClick={() => void handleFinalize()}
        >
          <ShieldCheck size={16} /> {busy ? 'Encrypting & Persisting Vault…' : 'Encrypt & Complete Setup'}
        </button>

        <button
          type="button"
          className="subtle"
          onClick={() => {
            setStage('show_phrase');
            setQuizError('');
          }}
          disabled={busy}
        >
          <ArrowLeft size={16} /> Back to view recovery phrase again
        </button>
      </div>
    </div>
  );
}
