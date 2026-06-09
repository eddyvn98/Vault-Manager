/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

import React from 'react';
import { 
  Star, 
  Copy, 
  Eye, 
  EyeOff, 
  Edit3, 
  Check, 
  Calendar, 
  Clock, 
  Trash2,
  Lock,
  Folder,
  History,
  ShieldCheck,
  RotateCcw,
  X
} from 'lucide-react';
import { Account } from '../types';

interface AccountDetailProps {
  account: Account | null;
  onToggleFavorite: (id: string, e: React.MouseEvent) => void;
  onEditAccount: (account: Account, e: React.MouseEvent) => void;
  onDeleteAccount: (id: string, e: React.MouseEvent) => void;
  onRestoreAccount?: (id: string) => void;
  onClose?: () => void;
  isMiniApp?: boolean;
}

export default function AccountDetail({
  account,
  onToggleFavorite,
  onEditAccount,
  onDeleteAccount,
  onRestoreAccount,
  onClose,
  isMiniApp = false
}: AccountDetailProps) {
  const [showPassword, setShowPassword] = React.useState(false);
  const [copiedField, setCopiedField] = React.useState<string | null>(null);
  
  // Real-time OTP 2FA Generation Setup
  const [otpCode, setOtpCode] = React.useState('532 918');
  const [secondsLeft, setSecondsLeft] = React.useState(30);

  // Generate a mock code that changes based on interval
  React.useEffect(() => {
    if (!account?.otpSecret) return;

    const generateOtp = () => {
      const now = new Date();
      const currentSecs = now.getSeconds();
      const left = 30 - (currentSecs % 30);
      setSecondsLeft(left);

      // Simple deterministic generation mimicking TOTP
      const baseNum = Math.floor(now.getTime() / 30000);
      const secretHash = account.id.charCodeAt(0) * 8219 + (account.otpSecret ? account.otpSecret.charCodeAt(0) * 1923 : 0);
      const codeNum = (baseNum * 313 + secretHash) % 900000 + 100000;
      const codeStr = codeNum.toString();
      setOtpCode(`${codeStr.slice(0, 3)} ${codeStr.slice(3)}`);
    };

    generateOtp();
    const interval = setInterval(generateOtp, 1000);

    return () => clearInterval(interval);
  }, [account]);

  // Reset eye visibility when switching accounts
  React.useEffect(() => {
    setShowPassword(false);
  }, [account?.id]);

  if (!account) {
    return (
      <div id="detail-empty-panel" className="w-96 border-none bg-slate-50/70 dark:bg-[#1C1C1E]/70 hidden lg:flex flex-col items-center justify-center p-8 text-center shrink-0">
        <div id="detail-lock-icon-bg" className="w-14 h-14 rounded-full bg-blue-50 dark:bg-blue-950/40 flex items-center justify-center text-blue-500 mb-4 animate-pulse">
          <Lock size={22} className="stroke-[2.5]" />
        </div>
        <h4 id="detail-empty-title" className="font-semibold text-slate-700 dark:text-slate-200 text-sm">Chưa chọn tài khoản</h4>
        <p id="detail-empty-desc" className="text-xs text-slate-400 mt-1 max-w-[200px]">
          Chọn một tài khoản ở danh sách bên trái để kiểm tra thông tin chi tiết.
        </p>
      </div>
    );
  }

  const handleCopy = (text: string, fieldName: string) => {
    navigator.clipboard.writeText(text);
    setCopiedField(fieldName);
    setTimeout(() => setCopiedField(null), 2000);
  };

  const getBrandInitial = (domain: string) => {
    const d = domain.toUpperCase();
    if (d.includes('GOOGLE') || d.includes('GMAIL')) return 'G';
    if (d.includes('FACEBOOK')) return 'F';
    if (d.includes('INSTAGRAM')) return 'I';
    if (d.includes('ZALO')) return 'Z';
    if (d.includes('STEAM')) return 'S';
    if (d.includes('VIETCOMBANK') || d.includes('VCB')) return 'V';
    return d.charAt(0);
  };

  const getBrandThemeColors = (domain: string) => {
    const d = domain.toLowerCase();
    if (d.includes('google') || d.includes('gmail')) return 'bg-red-500 text-white';
    if (d.includes('facebook')) return 'bg-blue-600 text-white';
    if (d.includes('instagram')) return 'bg-pink-500 text-white';
    if (d.includes('zalo')) return 'bg-blue-500 text-white';
    if (d.includes('steam')) return 'bg-slate-800 text-white';
    if (d.includes('vietcombank') || d.includes('vcb')) return 'bg-emerald-600 text-white';
    return 'bg-blue-600 text-white';
  };

  // Human readable time label helper
  const formatTimeAgo = (dateStr: string) => {
    const elapsed = Date.now() - new Date(dateStr).getTime();
    const minutes = Math.floor(elapsed / 60000);
    const hours = Math.floor(minutes / 60);
    const days = Math.floor(hours / 24);

    if (minutes < 1) return 'Vừa mới đây';
    if (minutes < 60) return `${minutes} phút trước`;
    if (hours < 24) return `${hours} giờ trước`;
    if (days === 1) return '1 ngày trước';
    return `${days} ngày trước`;
  };

  // Helper formatting date for creation UI string
  const formatDateSlash = (dateStr: string) => {
    const d = new Date(dateStr);
    const day = String(d.getDate()).padStart(2, '0');
    const month = String(d.getMonth() + 1).padStart(2, '0');
    const year = d.getFullYear();
    return `${day}/${month}/${year}`;
  };

  const bColorClass = getBrandThemeColors(account.appName);
  const bInitial = getBrandInitial(account.appName);

  if (isMiniApp) {
    return (
      <div 
        id={`detail-panel-${account.id}`} 
        className="w-full flex flex-col select-none bg-white dark:bg-[#1C1C1E] overflow-y-auto overflow-x-hidden text-xs pb-6"
      >
        {/* Account Details Form Body */}
        <div id="detail-fields-section" className="p-4 space-y-4 flex-1">
          {/* Alias Property */}
          <div id="prop-row-alias" className="space-y-1">
            <label className="text-[10px] font-bold text-slate-400 dark:text-zinc-500 uppercase tracking-wider block">Alias</label>
            <div className="flex items-center justify-between bg-slate-50 dark:bg-slate-800/40 border border-slate-100 dark:border-slate-805 rounded-xl p-2.5">
              <span className="font-mono text-xs font-bold text-slate-700 dark:text-slate-200">{account.alias}</span>
              <button
                onClick={(e) => onEditAccount(account, e)}
                className="p-1 text-slate-400 hover:text-slate-600 dark:hover:text-slate-300 rounded hover:bg-white dark:hover:bg-slate-700"
              >
                <Edit3 size={12} />
              </button>
            </div>
          </div>

          {/* Website Property */}
          <div id="prop-row-website" className="space-y-1">
            <label className="text-[10px] font-bold text-slate-400 dark:text-zinc-500 uppercase tracking-wider block">Website</label>
            <div className="flex items-center justify-between bg-slate-50 dark:bg-slate-800/40 border border-slate-100 dark:border-slate-805 rounded-xl p-2.5">
              <span className="text-xs font-medium text-slate-600 dark:text-slate-300 font-sans truncate pr-2">{account.appName}</span>
              <button
                onClick={() => handleCopy(account.appName, 'website')}
                className="p-1 text-slate-400 hover:text-slate-600 dark:hover:text-slate-305 rounded hover:bg-white dark:hover:bg-slate-700 transition-colors"
              >
                {copiedField === 'website' ? <Check size={13} className="text-emerald-500" /> : <Copy size={13} />}
              </button>
            </div>
          </div>

          {/* Username Property */}
          <div id="prop-row-username" className="space-y-1">
            <label className="text-[10px] font-bold text-slate-400 dark:text-zinc-500 uppercase tracking-wider block">Username</label>
            <div className="flex items-center justify-between bg-slate-50 dark:bg-slate-800/40 border border-slate-100 dark:border-slate-805 rounded-xl p-2.5">
              <span className="font-mono text-xs text-slate-700 dark:text-slate-205 font-medium truncate pr-2">{account.username}</span>
              <button
                onClick={() => handleCopy(account.username, 'username')}
                className="p-1 text-slate-400 hover:text-slate-600 dark:hover:text-slate-300 rounded hover:bg-white dark:hover:bg-slate-700 transition-colors"
              >
                {copiedField === 'username' ? <Check size={13} className="text-emerald-500" /> : <Copy size={13} />}
              </button>
            </div>
          </div>

          {/* Password Property */}
          <div id="prop-row-password" className="space-y-1">
            <label className="text-[10px] font-bold text-slate-400 dark:text-zinc-500 uppercase tracking-wider block">Mật khẩu chính</label>
            <div className="flex items-center justify-between bg-slate-50 dark:bg-slate-800/40 border border-slate-100 dark:border-slate-805 rounded-xl p-2.5">
              <span className="font-mono text-xs font-bold text-slate-700 dark:text-slate-202 tracking-wider break-all max-w-[170px] select-all">
                {showPassword ? account.passwordDecrypted : '••••••••••••••••'}
              </span>
              <div className="flex items-center gap-1.5 shrink-0">
                <button
                  onClick={() => setShowPassword(!showPassword)}
                  className="p-1 text-slate-400 hover:text-slate-600 dark:hover:text-slate-300 rounded hover:bg-white dark:hover:bg-slate-700 transition-colors"
                >
                  {showPassword ? <EyeOff size={13} /> : <Eye size={13} />}
                </button>
                <button
                  onClick={() => handleCopy(account.passwordDecrypted, 'password')}
                  className="p-1 text-slate-400 hover:text-slate-600 dark:hover:text-slate-300 rounded hover:bg-white dark:hover:bg-slate-700 transition-colors"
                >
                  {copiedField === 'password' ? <Check size={13} className="text-emerald-500" /> : <Copy size={13} />}
                </button>
              </div>
            </div>
          </div>

          {/* Alternative Passwords Property */}
          {account.alternativePasswords && account.alternativePasswords.length > 0 && (
            <div id="prop-row-alternative-passwords" className="space-y-1">
              <label className="text-[10px] font-bold text-slate-400 dark:text-zinc-500 uppercase tracking-wider block">Mật khẩu phụ</label>
              <div className="space-y-1.5 animate-none">
                {account.alternativePasswords.map((altPass, idx) => (
                  <div key={idx} className="flex items-center justify-between bg-amber-500/5 border border-amber-500/10 rounded-xl p-2.5">
                    <span className="font-mono text-xs font-semibold text-slate-700 dark:text-slate-200 tracking-wider">
                      {showPassword ? altPass : '••••••••••••••••'}
                    </span>
                    <button
                      onClick={() => handleCopy(altPass, `alt-password-${idx}`)}
                      className="p-1 text-slate-400 hover:text-slate-600 dark:hover:text-slate-305 rounded hover:bg-white dark:hover:bg-slate-700 transition-colors shrink-0"
                    >
                      {copiedField === `alt-password-${idx}` ? <Check size={13} className="text-emerald-500" /> : <Copy size={13} />}
                    </button>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* OTP (2FA) Property */}
          <div id="prop-row-otp" className="space-y-1">
            <label className="text-[10px] font-bold text-slate-400 dark:text-zinc-500 uppercase tracking-wider block">Mã OTP (2FA)</label>
            {account.otpSecret ? (
              <div className="flex items-center justify-between bg-slate-50 dark:bg-slate-800/40 border border-slate-100 dark:border-slate-805 rounded-xl p-2.5">
                <span className="font-mono text-sm font-bold text-blue-600 dark:text-blue-400 tracking-wider">
                  {otpCode}
                </span>
                <div className="flex items-center gap-1.5 shrink-0">
                  <div className="flex items-center gap-1 text-[9px] font-bold px-1.5 py-0.5 rounded-full bg-blue-100/60 dark:bg-blue-952 text-blue-600 dark:text-blue-450">
                    <Clock size={10} className="animate-spin duration-[4000ms]" />
                    <span>{secondsLeft}s</span>
                  </div>
                  <button
                    onClick={() => handleCopy(otpCode.replace(/\s+/g, ''), 'otp')}
                    className="p-1 text-slate-400 hover:text-slate-600 dark:hover:text-slate-350 rounded hover:bg-white dark:hover:bg-slate-700 transition-colors"
                  >
                    {copiedField === 'otp' ? <Check size={13} className="text-emerald-500" /> : <Copy size={13} />}
                  </button>
                </div>
              </div>
            ) : (
              <p className="text-[10px] text-slate-400 italic pl-1">Chưa thiết lập OTP.</p>
            )}
          </div>

          {/* Notes Property */}
          <div id="prop-row-notes" className="space-y-1">
            <label className="text-[10px] font-bold text-slate-400 dark:text-zinc-500 uppercase tracking-wider block">Ghi chú</label>
            <div className="bg-slate-50 dark:bg-slate-800/40 border border-slate-100 dark:border-slate-850 rounded-xl p-2.5 min-h-[50px] flex justify-between items-start">
              <span className="text-[11px] text-slate-605 dark:text-slate-300 font-sans leading-snug flex-1 pr-1.5">
                {account.notes || 'Không có ghi chú.'}
              </span>
              <button
                onClick={(e) => onEditAccount(account, e)}
                className="p-1 text-slate-400 hover:text-slate-600 dark:hover:text-slate-300 rounded hover:bg-white dark:hover:bg-slate-700 transition-colors"
              >
                <Edit3 size={11} />
              </button>
            </div>
          </div>
        </div>

        {/* History changes */}
        {account.history && account.history.length > 0 && (
          <div className="px-4 py-2 space-y-1 border-t border-slate-100 dark:border-slate-800/40">
            <div className="text-[9px] font-bold text-slate-400 uppercase tracking-wider flex items-center gap-1 mt-1">
              <History size={10} />
              <span>Lịch sử thay đổi</span>
            </div>
            <div className="space-y-1 pl-2">
              {account.history.slice(0, 2).map((hist, index) => (
                <div key={index} className="flex justify-between text-[10px] text-slate-500">
                  <span className="font-mono">{hist.value.slice(0, 2) + '•••' + hist.value.slice(-2)}</span>
                  <span>{formatDateSlash(hist.passwordChangedAt)}</span>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Footer meta info */}
        <div className="p-3 bg-slate-100/20 dark:bg-[#1C1C1E]/20 text-[9px] text-slate-400 dark:text-slate-505 space-y-0.5 border-t dark:border-slate-800/30">
          <div className="flex justify-between">
            <span>Ngày tạo:</span>
            <span>{formatDateSlash(account.createdAt)}</span>
          </div>
          <div className="flex justify-between">
            <span>Cập nhật cuối:</span>
            <span>{formatTimeAgo(account.updatedAt)}</span>
          </div>
        </div>
      </div>
    );
  }

  return (
    <>
      {/* Mobile/Tablet Backdrop Overlay */}
      {onClose && (
        <div 
          onClick={onClose} 
          className="lg:hidden fixed inset-0 z-30 bg-slate-950/45 backdrop-blur-xs"
        />
      )}

      <div 
        id={`detail-panel-${account.id}`} 
        className="fixed inset-y-0 right-0 z-40 lg:static lg:flex lg:w-96 flex flex-col h-full shrink-0 select-none bg-slate-50/70 dark:bg-[#1C1C1E]/70 border-none overflow-y-auto w-[360px] max-w-[calc(100vw-2.5rem)] shadow-2xl lg:shadow-none transition-transform duration-350"
      >
        
        {/* Account Hero Header */}
        <div id="detail-hero-section" className="p-6 pb-2">
          <div id="detail-hero-header" className="flex items-start justify-between">
            <div id="detail-brand-info" className="flex items-center gap-4">
              <div id="detail-avatar-logo" className={`w-12 h-12 rounded-2xl font-bold flex items-center justify-center text-lg shadow-md shadow-black/[0.04] shrink-0 ${bColorClass}`}>
                {bInitial}
              </div>
              <div>
                <div className="flex items-center gap-1.5 flex-wrap">
                  <h3 id="detail-display-name" className="font-display font-bold text-slate-800 dark:text-slate-100 text-base leading-snug">
                    {account.alias.toUpperCase().includes('MAIN') ? `${account.alias.split('-')[0].charAt(0).toUpperCase() + account.alias.split('-')[0].slice(1)} (Chính)` : account.alias}
                  </h3>
                  <button
                    id="detail-star-btn"
                    onClick={(e) => onToggleFavorite(account.id, e)}
                    className={`p-1 rounded-full transition-colors hover:bg-slate-50 dark:hover:bg-slate-800 ${account.isFavorite ? 'text-amber-500' : 'text-slate-300 dark:text-slate-600'}`}
                  >
                    <Star size={15} fill={account.isFavorite ? 'currentColor' : 'none'} className="stroke-[2.5]" />
                  </button>
                </div>
                <span id="detail-category-badge" className="flex items-center gap-1 text-[11px] font-bold text-slate-400 dark:text-slate-500 mt-0.5">
                  <Folder size={11} />
                  <span>DanhMuc: {account.category}</span>
                </span>
              </div>
            </div>

            <div id="detail-meta-actions" className="flex items-center gap-1 bg-slate-50 dark:bg-slate-800 p-1 rounded-lg">
              {account.isDeleted ? (
                <button
                  id="detail-restore-btn"
                  onClick={() => onRestoreAccount && onRestoreAccount(account.id)}
                  className="p-1 px-2.5 rounded-md hover:bg-white dark:hover:bg-slate-700 text-xs font-bold text-emerald-600 dark:text-emerald-400 flex items-center gap-1 shadow-sm border-0 border-slate-200"
                  title="Khôi phục tài khoản"
                >
                  <RotateCcw size={12} />
                  <span>Khôi phục</span>
                </button>
              ) : (
                <>
                  <button
                    id="detail-edit-pencil-btn"
                    onClick={(e) => onEditAccount(account, e)}
                    className="p-1.5 rounded-md hover:bg-white dark:hover:bg-slate-700 text-slate-500 hover:text-slate-800 dark:hover:text-slate-200 transition-colors"
                    title="Chỉnh sửa tài khoản"
                  >
                    <Edit3 size={15} />
                  </button>
                  <button
                    id="detail-trash-btn"
                    onClick={(e) => onDeleteAccount(account.id, e)}
                    className="p-1.5 rounded-md hover:bg-white dark:hover:bg-slate-700 text-slate-500 hover:text-red-500 dark:hover:text-red-400 transition-colors"
                    title="Chuyển vào Thùng rác"
                  >
                    <Trash2 size={15} />
                  </button>
                </>
              )}
              {onClose && (
                <button
                  id="detail-close-btn"
                  onClick={onClose}
                  className="lg:hidden p-1.5 rounded-md hover:bg-white dark:hover:bg-slate-700 text-slate-500 hover:text-slate-800 dark:hover:text-slate-200 transition-colors"
                  title="Đóng chi tiết"
                >
                  <X size={15} />
                </button>
              )}
            </div>
          </div>
        </div>

      {/* Account Details Form Body */}
      <div id="detail-fields-section" className="flex-1 p-6 space-y-5">
        
        {/* Alias Property */}
        <div id="prop-row-alias" className="space-y-1.5">
          <label className="text-[11px] font-bold text-slate-400 dark:text-slate-500 uppercase tracking-wider block">Alias</label>
          <div className="flex items-center justify-between bg-slate-50 dark:bg-slate-800/40 border border-slate-100 dark:border-slate-800/80 rounded-xl p-3">
            <span className="font-mono text-xs font-bold text-slate-700 dark:text-slate-200">{account.alias}</span>
            <button
              onClick={(e) => onEditAccount(account, e)}
              className="p-1 text-slate-400 hover:text-slate-600 dark:hover:text-slate-300 rounded hover:bg-white dark:hover:bg-slate-700"
            >
              <Edit3 size={13} />
            </button>
          </div>
        </div>

        {/* Website Property */}
        <div id="prop-row-website" className="space-y-1.5">
          <label className="text-[11px] font-bold text-slate-400 dark:text-slate-500 uppercase tracking-wider block">Website</label>
          <div className="flex items-center justify-between bg-slate-50 dark:bg-slate-800/40 border border-slate-100 dark:border-slate-800/80 rounded-xl p-3">
            <span className="text-xs font-medium text-slate-600 dark:text-slate-300 font-sans truncate pr-2">{account.appName}</span>
            <button
              onClick={() => handleCopy(account.appName, 'website')}
              className="p-1 text-slate-400 hover:text-slate-600 dark:hover:text-slate-300 rounded hover:bg-white dark:hover:bg-slate-700 transition-colors"
            >
              {copiedField === 'website' ? <Check size={13} className="text-emerald-500" /> : <Copy size={13} />}
            </button>
          </div>
        </div>

        {/* Username Property */}
        <div id="prop-row-username" className="space-y-1.5">
          <label className="text-[11px] font-bold text-slate-400 dark:text-slate-500 uppercase tracking-wider block">Username</label>
          <div className="flex items-center justify-between bg-slate-50 dark:bg-slate-800/40 border border-slate-100 dark:border-slate-800/80 rounded-xl p-3">
            <span className="font-mono text-xs text-slate-700 dark:text-slate-200 font-medium truncate pr-2">{account.username}</span>
            <button
              onClick={() => handleCopy(account.username, 'username')}
              className="p-1 text-slate-400 hover:text-slate-600 dark:hover:text-slate-300 rounded hover:bg-white dark:hover:bg-slate-700 transition-colors"
            >
              {copiedField === 'username' ? <Check size={13} className="text-emerald-500" /> : <Copy size={13} />}
            </button>
          </div>
        </div>

        {/* Password Property */}
        <div id="prop-row-password" className="space-y-1.5">
          <label className="text-[11px] font-bold text-slate-400 dark:text-slate-500 uppercase tracking-wider block">Mật khẩu chính</label>
          <div className="flex items-center justify-between bg-slate-50 dark:bg-slate-800/40 border border-slate-100 dark:border-slate-800/80 rounded-xl p-3">
            <span className="font-mono text-xs font-bold text-slate-700 dark:text-slate-200 tracking-wider">
              {showPassword ? account.passwordDecrypted : '••••••••••••••••••••'}
            </span>
            <div className="flex items-center gap-1.5">
              <button
                onClick={() => setShowPassword(!showPassword)}
                className="p-1 text-slate-400 hover:text-slate-600 dark:hover:text-slate-300 rounded hover:bg-white dark:hover:bg-slate-700 transition-colors"
              >
                {showPassword ? <EyeOff size={13} /> : <Eye size={13} />}
              </button>
              <button
                onClick={() => handleCopy(account.passwordDecrypted, 'password')}
                className="p-1 text-slate-400 hover:text-slate-600 dark:hover:text-slate-300 rounded hover:bg-white dark:hover:bg-slate-700 transition-colors"
              >
                {copiedField === 'password' ? <Check size={13} className="text-emerald-500" /> : <Copy size={13} />}
              </button>
            </div>
          </div>
        </div>

        {/* Alternative Passwords Property */}
        {account.alternativePasswords && account.alternativePasswords.length > 0 && (
          <div id="prop-row-alternative-passwords" className="space-y-1.5">
            <label className="text-[11px] font-bold text-slate-400 dark:text-slate-500 uppercase tracking-wider block">Các mật khẩu phụ / dự phòng</label>
            <div className="space-y-2">
              {account.alternativePasswords.map((altPass, idx) => (
                <div key={idx} className="flex items-center justify-between bg-amber-500/5 dark:bg-amber-450/5 border border-amber-500/10 dark:border-amber-550/20 rounded-xl p-3">
                  <span className="font-mono text-xs font-semibold text-slate-700 dark:text-slate-200 tracking-wider">
                    {showPassword ? altPass : '••••••••••••••••••••'}
                  </span>
                  <div className="flex items-center gap-1.5">
                    <button
                      onClick={() => handleCopy(altPass, `alt-password-${idx}`)}
                      className="p-1 text-slate-400 hover:text-slate-600 dark:hover:text-slate-300 rounded hover:bg-white dark:hover:bg-slate-700 transition-colors"
                    >
                      {copiedField === `alt-password-${idx}` ? <Check size={13} className="text-emerald-500" /> : <Copy size={13} />}
                    </button>
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* OTP (2FA) Property */}
        <div id="prop-row-otp" className="space-y-1.5">
          <label className="text-[11px] font-bold text-slate-400 dark:text-slate-500 uppercase tracking-wider block">Mã xác thực OTP (2FA)</label>
          {account.otpSecret ? (
            <div className="flex items-center justify-between bg-slate-50 dark:bg-slate-800/40 border border-slate-100 dark:border-slate-800/80 rounded-xl p-3">
              <span className="font-mono text-sm font-bold text-blue-600 dark:text-blue-400 tracking-wide">
                {otpCode}
              </span>
              <div className="flex items-center gap-2">
                {/* Visual Timer ring mockup */}
                <div className="flex items-center gap-1 text-[11px] font-bold px-2 py-0.5 rounded-full bg-blue-100/60 dark:bg-blue-952 text-blue-600 dark:text-blue-400">
                  <Clock size={11} className="animate-spin duration-[4000ms]" />
                  <span>{secondsLeft}s</span>
                </div>
                <button
                  onClick={() => handleCopy(otpCode.replace(/\s+/g, ''), 'otp')}
                  className="p-1 text-slate-400 hover:text-slate-600 dark:hover:text-slate-300 rounded hover:bg-white dark:hover:bg-slate-700 transition-colors"
                >
                  {copiedField === 'otp' ? <Check size={13} className="text-emerald-500" /> : <Copy size={13} />}
                </button>
              </div>
            </div>
          ) : (
            <p className="text-xs text-slate-400 italic pl-1">Chưa liên kết khóa bảo mật 2FA.</p>
          )}
        </div>

        {/* Notes Property */}
        <div id="prop-row-notes" className="space-y-1.5">
          <label className="text-[11px] font-bold text-slate-400 dark:text-slate-500 uppercase tracking-wider block">Ghi chú</label>
          <div className="bg-slate-50 dark:bg-slate-800/40 border border-slate-100 dark:border-slate-800/80 rounded-xl p-3 min-h-[70px] relative group flex justify-between items-start">
            <span className="text-xs text-slate-600 dark:text-slate-300 font-sans leading-relaxed flex-1 pr-2">
              {account.notes || 'Không có ghi chú.'}
            </span>
            <button
              onClick={(e) => onEditAccount(account, e)}
              className="p-1 text-slate-400 hover:text-slate-600 dark:hover:text-slate-300 rounded hover:bg-white dark:hover:bg-slate-700 transition-colors mt-[-2px]"
            >
              <Edit3 size={12} />
            </button>
          </div>
        </div>

        {/* Password History changes list (matching bottom design section in image) */}
        <div id="props-row-history" className="space-y-2 pt-4 pr-2">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-1 text-[11px] font-bold text-slate-400 dark:text-slate-500 uppercase tracking-wider">
              <History size={12} />
              <span>Lịch sử thay đổi mật khẩu</span>
            </div>
          </div>
          {account.history && account.history.length > 0 ? (
            <div className="space-y-1.5">
              {account.history.slice(0, 3).map((hist, index) => (
                <div key={hist.id || index} className="flex items-center justify-between text-xs text-slate-500 pl-4 py-1.5 border-l-2 border-slate-100 dark:border-slate-800 hover:border-blue-500 dark:hover:border-blue-500 transition-colors group">
                  <span>Mật khẩu cũ: <span className="font-mono font-semibold text-slate-700 dark:text-slate-300">{hist.value.slice(0, 3) + '••••' + hist.value.slice(-3)}</span></span>
                  <span className="text-[10px] text-slate-400 font-medium">{formatDateSlash(hist.passwordChangedAt)}</span>
                </div>
              ))}
              {account.history.length > 3 && (
                <div className="text-right">
                  <button className="text-xs font-bold text-blue-600 hover:text-blue-700 dark:text-blue-400 hover:underline">
                    Xem tất cả ({account.history.length})
                  </button>
                </div>
              )}
            </div>
          ) : (
            <p className="text-xs text-slate-400 italic pl-4">Không phát hiện dữ liệu thay đổi mật khẩu trước đây.</p>
          )}
        </div>
      </div>

      {/* Detail Footer meta */}
      <div id="detail-footer-meta" className="p-4 bg-slate-100/30 dark:bg-[#1C1C1E]/30 text-[11px] text-slate-400 dark:text-slate-500 space-y-1">
        <div className="flex justify-between">
          <span className="font-semibold">Ngày tạo:</span>
          <span>{formatDateSlash(account.createdAt)}</span>
        </div>
        <div className="flex justify-between">
          <span className="font-semibold">Cập nhật cuối:</span>
          <span>{formatTimeAgo(account.updatedAt)}</span>
        </div>
      </div>
    </div>
  </>
  );
}
