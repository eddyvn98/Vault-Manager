import React, { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getErrorMessage } from '../types';

interface RestoreFlowProps {
  onSuccess: () => void;
  onError: (msg: string) => void;
}

export default function RestoreFlow({ onSuccess, onError }: RestoreFlowProps) {
  const [recovery, setRecovery] = useState('');
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [busy, setBusy] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
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
      await invoke('restore_wallet', { password, recovery });
      setPassword('');
      setConfirmPassword('');
      setRecovery('');
      onSuccess();
    } catch (err) {
      onError(getErrorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <form onSubmit={handleSubmit}>
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

      <label>
        New Local Master Password (12+ characters)
        <input
          type="password"
          minLength={12}
          maxLength={1024}
          autoComplete="new-password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          placeholder="Choose new local master password"
          required
        />
      </label>

      <label>
        Confirm Local Master Password
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
        {busy ? 'Deriving Argon2id & Encrypting…' : 'Restore & Encrypt on this Device'}
      </button>

      <p className="muted small">
        The recovery phrase is validated locally using the BIP39 checksum and encrypted directly with Argon2id + XChaCha20-Poly1305.
      </p>
    </form>
  );
}
