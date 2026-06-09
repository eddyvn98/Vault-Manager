/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

import React from 'react';
import { ShieldCheck, ShieldAlert, AlertTriangle, Key, Search, ChevronRight, Check } from 'lucide-react';
import { Account } from '../types';

interface SecurityCheckupProps {
  accounts: Account[];
  onSelectAccount: (account: Account) => void;
  setCurrentTab: (tab: string) => void;
}

export default function SecurityCheckup({
  accounts,
  onSelectAccount,
  setCurrentTab
}: SecurityCheckupProps) {
  
  const activeAccounts = accounts.filter(a => !a.isDeleted);

  // Security analysis states
  const analysis = React.useMemo(() => {
    let weak: Account[] = [];
    let reused: Account[] = [];
    let strong: Account[] = [];
    
    // Group passwords to check duplicates
    const passGroups: { [key: string]: Account[] } = {};

    activeAccounts.forEach(acc => {
      const pwd = acc.passwordDecrypted;
      
      // 1. Weak password check
      const hasUpper = /[A-Z]/.test(pwd);
      const hasLower = /[a-z]/.test(pwd);
      const hasDigit = /[0-9]/.test(pwd);
      const hasSpecial = /[^A-Za-z0-9]/.test(pwd);
      
      const isTooShort = pwd.length < 10;
      const complexityCount = [hasUpper, hasLower, hasDigit, hasSpecial].filter(Boolean).length;
      
      if (isTooShort || complexityCount < 3) {
        weak.push(acc);
      } else {
        strong.push(acc);
      }

      // Group for reuse check
      if (!passGroups[pwd]) {
        passGroups[pwd] = [];
      }
      passGroups[pwd].push(acc);
    });

    // Extract duplicate list
    Object.keys(passGroups).forEach(pwd => {
      if (passGroups[pwd].length > 1) {
        reused = [...reused, ...passGroups[pwd]];
      }
    });

    return { weak, reused, strong };
  }, [activeAccounts]);

  const total = activeAccounts.length || 1;
  const weakPercent = Math.round((analysis.weak.length / total) * 100);
  const reusedPercent = Math.round((analysis.reused.length / total) * 100);
  const strongPercent = Math.round((analysis.strong.length / total) * 100);

  const handleFixClick = (account: Account) => {
    onSelectAccount(account);
    setCurrentTab('accounts');
  };

  return (
    <div id="security-checkup-view" className="flex-1 overflow-y-auto bg-[#F5F5F7] dark:bg-[#1A1A1E] p-4 sm:p-6 space-y-6 select-none">
      <div>
        <h2 id="security-headline" className="text-xl font-display font-bold text-slate-800 dark:text-slate-100">
          Kiểm tra bảo mật mật khẩu
        </h2>
        <p id="security-description" className="text-xs text-slate-400 mt-1">
          Hệ thống quét bảo mật giúp phân tích độ phức tạp của mật khẩu và cảnh báo nguy cơ rò rỉ dữ liệu khi sử dụng chung một mật khẩu.
        </p>
      </div>

      {/* Bento Stats row */}
      <div id="security-bento-grid" className="grid grid-cols-1 md:grid-cols-3 gap-6">
        
        {/* Safe Badge */}
        <div className="bg-white dark:bg-[#1C1C1E] rounded-2xl p-5 shadow-[0_8px_30px_rgba(0,0,0,0.03)] flex items-center justify-between">
          <div className="space-y-1.5">
            <span className="text-xs font-semibold text-slate-400">Mật khẩu mạnh mẽ</span>
            <h3 className="text-3xl font-bold text-emerald-600 dark:text-emerald-500">{analysis.strong.length} <span className="text-sm text-slate-400 font-semibold">Tài khoản</span></h3>
            <div className="flex items-center gap-1.5 text-[11px] text-emerald-600 font-bold bg-emerald-50 dark:bg-emerald-950/20 px-2 py-0.5 rounded-full w-max">
              <span>{strongPercent}% Đạt chuẩn</span>
            </div>
          </div>
          <div className="w-12 h-12 bg-emerald-50 dark:bg-emerald-950/40 rounded-xl flex items-center justify-center text-emerald-600">
            <ShieldCheck size={24} />
          </div>
        </div>

        {/* Weak warning Badge */}
        <div className="bg-white dark:bg-[#1C1C1E] rounded-2xl p-5 shadow-[0_8px_30px_rgba(0,0,0,0.03)] flex items-center justify-between">
          <div className="space-y-1.5">
            <span className="text-xs font-semibold text-slate-400">Mật khẩu quá yếu</span>
            <h3 className="text-3xl font-bold text-red-600 dark:text-red-500">{analysis.weak.length} <span className="text-sm text-slate-400 font-semibold">Tài khoản</span></h3>
            <div className="flex items-center gap-1.5 text-[11px] text-red-600 font-bold bg-red-50 dark:bg-red-950/20 px-2 py-0.5 rounded-full w-max">
              <span>{weakPercent}% Cần cải thiện</span>
            </div>
          </div>
          <div className="w-12 h-12 bg-red-50 dark:bg-red-950/40 rounded-xl flex items-center justify-center text-red-600 animate-pulse">
            <ShieldAlert size={24} />
          </div>
        </div>

        {/* Duplicate warning Badge */}
        <div className="bg-white dark:bg-[#1C1C1E] rounded-2xl p-5 shadow-[0_8px_30px_rgba(0,0,0,0.03)] flex items-center justify-between">
          <div className="space-y-1.5">
            <span className="text-xs font-semibold text-slate-400">Trùng lặp mật khẩu</span>
            <h3 className="text-3xl font-bold text-amber-500">{analysis.reused.length} <span className="text-sm text-slate-400 font-semibold">Tài khoản</span></h3>
            <div className="flex items-center gap-1.5 text-[11px] text-amber-600 font-bold bg-amber-50 dark:bg-amber-950/20 px-2 py-0.5 rounded-full w-max">
              <span>{reusedPercent}% Sử dụng chung</span>
            </div>
          </div>
          <div className="w-12 h-12 bg-amber-50 dark:bg-amber-950/40 rounded-xl flex items-center justify-center text-amber-500">
            <AlertTriangle size={24} />
          </div>
        </div>

      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        
        {/* Weak accounts list */}
        <div className="bg-white dark:bg-[#1C1C1E] rounded-2xl p-5 shadow-[0_8px_30px_rgba(0,0,0,0.03)] space-y-4">
          <h3 className="font-display font-bold text-sm text-slate-800 dark:text-slate-200 uppercase tracking-wider flex items-center gap-2">
            <ShieldAlert size={16} className="text-red-500" />
            <span>Mật khẩu yếu hoặc quá ngắn ({analysis.weak.length})</span>
          </h3>

          <div className="max-h-[300px] overflow-y-auto space-y-2.5 pr-2">
            {analysis.weak.length === 0 ? (
              <div className="p-8 text-center text-slate-400 text-xs">
                🎉 Tuyệt vời! Tất cả mật khẩu của bạn đều đạt độ an toàn chuẩn phức tạp.
              </div>
            ) : (
              analysis.weak.map(acc => (
                <div key={acc.id} className="flex items-center justify-between bg-slate-50 dark:bg-[#2C2C2E]/40 p-3 rounded-xl hover:bg-slate-100/70 transition-colors">
                  <div>
                    <span className="font-semibold text-xs text-slate-800 dark:text-slate-250 block">{acc.alias}</span>
                    <span className="text-[10px] text-slate-400 font-mono">{acc.username}</span>
                  </div>
                  <button
                    onClick={() => handleFixClick(acc)}
                    className="flex items-center gap-1 py-1 px-2.5 rounded bg-blue-50 dark:bg-blue-950/60 text-[10px] font-bold text-blue-600 dark:text-blue-400 transition-colors cursor-pointer border border-transparent hover:border-blue-200"
                  >
                    <span>Sửa mật khẩu</span>
                    <ChevronRight size={10} />
                  </button>
                </div>
              ))
            )}
          </div>
        </div>

        {/* Duplicate accounts list */}
        <div className="bg-white dark:bg-[#1C1C1E] rounded-2xl p-5 shadow-[0_8px_30px_rgba(0,0,0,0.03)] space-y-4">
          <h3 className="font-display font-bold text-sm text-slate-800 dark:text-slate-200 uppercase tracking-wider flex items-center gap-2">
            <AlertTriangle size={16} className="text-amber-500" />
            <span>Sử dụng chung mật khẩu ({analysis.reused.length})</span>
          </h3>

          <div className="max-h-[300px] overflow-y-auto space-y-2.5 pr-2">
            {analysis.reused.length === 0 ? (
              <div className="p-8 text-center text-slate-400 text-xs">
                🎉 Tuyệt vời! Không phát hiện trường hợp dùng trùng lặp mật khẩu nào.
              </div>
            ) : (
              analysis.reused.map(acc => (
                <div key={acc.id} className="flex items-center justify-between bg-slate-50 dark:bg-[#2C2C2E]/40 p-3 rounded-xl hover:bg-slate-100/70 transition-colors">
                  <div>
                    <span className="font-semibold text-xs text-slate-800 dark:text-slate-250 block">{acc.alias}</span>
                    <span className="text-[10px] text-slate-400 font-mono">{acc.username}</span>
                  </div>
                  <button
                    onClick={() => handleFixClick(acc)}
                    className="flex items-center gap-1 py-1 px-2.5 rounded bg-blue-50 dark:bg-blue-950/60 text-[10px] font-bold text-blue-600 dark:text-blue-400 transition-colors cursor-pointer border border-transparent hover:border-blue-200"
                  >
                    <span>Sửa mật khẩu</span>
                    <ChevronRight size={10} />
                  </button>
                </div>
              ))
            )}
          </div>
        </div>

      </div>
    </div>
  );
}
