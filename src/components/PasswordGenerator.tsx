/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

import React from 'react';
import { KeyRound, Copy, RotateCw, Check, ShieldAlert, Sparkles, AlertTriangle } from 'lucide-react';

export default function PasswordGenerator() {
  const [length, setLength] = React.useState(16);
  const [includeUppercase, setIncludeUppercase] = React.useState(true);
  const [includeLowercase, setIncludeLowercase] = React.useState(true);
  const [includeNumbers, setIncludeNumbers] = React.useState(true);
  const [includeSymbols, setIncludeSymbols] = React.useState(true);
  const [generatedPassword, setGeneratedPassword] = React.useState('');
  const [copied, setCopied] = React.useState(false);
  const [history, setHistory] = React.useState<string[]>([]);

  const generate = React.useCallback(() => {
    let chars = '';
    if (includeUppercase) chars += 'ABCDEFGHIJKLMNOPQRSTUVWXYZ';
    if (includeLowercase) chars += 'abcdefghijklmnopqrstuvwxyz';
    if (includeNumbers) chars += '0123456789';
    if (includeSymbols) chars += '!@#$%^&*()_+-=[]{}|;:,.<>?';

    if (!chars) {
      setGeneratedPassword('Vui lòng chọn ít nhất một tùy chọn!');
      return;
    }

    let password = '';
    for (let i = 0; i < length; i++) {
      const randomIndex = Math.floor(Math.random() * chars.length);
      password += chars[randomIndex];
    }

    setGeneratedPassword(password);
    setCopied(false);
    
    // Save to locally scoped history
    setHistory(prev => [password, ...prev.slice(0, 4)]);
  }, [length, includeUppercase, includeLowercase, includeNumbers, includeSymbols]);

  React.useEffect(() => {
    generate();
  }, [generate]);

  const handleCopy = () => {
    if (!generatedPassword || generatedPassword.startsWith('Vui lòng')) return;
    navigator.clipboard.writeText(generatedPassword);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  // Evaluate Password strength score
  const getStrengthInfo = () => {
    if (generatedPassword.startsWith('Vui lòng')) return { label: 'Không xác định', color: 'bg-slate-300', text: 'text-slate-500', width: 'w-0' };
    
    let score = 0;
    if (includeUppercase) score++;
    if (includeLowercase) score++;
    if (includeNumbers) score++;
    if (includeSymbols) score++;
    if (length >= 12) score++;
    if (length >= 16) score++;

    if (score <= 2) {
      return { label: 'Cực Yếu ⚠️', color: 'bg-red-500', text: 'text-red-500', width: 'w-1/4' };
    }
    if (score <= 4) {
      return { label: 'Trung Bình ⚡', color: 'bg-yellow-500', text: 'text-yellow-500', width: 'w-2/4' };
    }
    if (score === 5) {
      return { label: 'Mạnh 💪', color: 'bg-green-500', text: 'text-green-500', width: 'w-3/4' };
    }
    return { label: 'Tối Tân 🏆', color: 'bg-emerald-500', text: 'text-emerald-500', width: 'w-full' };
  };

  const strength = getStrengthInfo();

  return (
    <div id="password-generator-view" className="flex-1 overflow-y-auto bg-[#F5F5F7] dark:bg-[#1A1A1E] p-4 sm:p-6 space-y-6 select-none">
      <div>
        <h2 id="generator-headline" className="text-xl font-display font-bold text-slate-800 dark:text-slate-100">
          Trình tạo mật khẩu ngẫu nhiên tối tân
        </h2>
        <p id="generator-description" className="text-xs text-slate-400 mt-1">
          Tạo mật khẩu cực mạnh có độ mã hóa cao để bảo vệ tài khoản của bạn khỏi các cuộc tấn công Brute-Force.
        </p>
      </div>

      <div id="generator-grid" className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        
        {/* Left Side Controller form */}
        <div className="lg:col-span-2 bg-white dark:bg-[#1C1C1E] rounded-2xl p-6 shadow-[0_8px_30px_rgba(0,0,0,0.03)] space-y-6">
          
          {/* Output Display Area */}
          <div className="space-y-2">
            <label className="text-[11px] font-bold text-slate-400 dark:text-slate-500 uppercase tracking-wider">Mật khẩu được tạo</label>
            <div className="flex items-center justify-between bg-slate-50 dark:bg-[#161618] p-4 rounded-xl border-none">
              <span id="generated-password-label" className="font-mono text-sm sm:text-base font-bold text-slate-800 dark:text-slate-100 break-all select-all select-all-clickable pr-4">
                {generatedPassword}
              </span>

              <div className="flex items-center gap-2 shrink-0">
                <button
                  id="btn-regenerate-password"
                  onClick={generate}
                  className="p-2 text-slate-400 hover:text-slate-700 dark:hover:text-slate-200 hover:bg-white dark:hover:bg-slate-800 rounded-lg transition-colors border border-transparent hover:border-slate-200 dark:hover:border-slate-700"
                  title="Tạo lại mật khẩu mới"
                >
                  <RotateCw size={16} />
                </button>
                <button
                  id="btn-copy-generated-password"
                  onClick={handleCopy}
                  className="p-2 bg-blue-600 hover:bg-blue-700 text-white rounded-lg transition-colors shadow-md shadow-blue-500/10"
                  title="Sao chép mật khẩu"
                >
                  {copied ? <Check size={16} /> : <Copy size={16} />}
                </button>
              </div>
            </div>

            {/* Strength meter animation */}
            <div className="pt-2 space-y-1.5">
              <div className="flex justify-between text-xs font-semibold">
                <span className="text-slate-400">Độ mạnh mật khẩu:</span>
                <span className={strength.text}>{strength.label}</span>
              </div>
              <div className="w-full bg-slate-100 dark:bg-slate-800 h-2 rounded-full overflow-hidden">
                <div className={`h-full transition-all duration-300 ${strength.color} ${strength.width}`}></div>
              </div>
            </div>
          </div>

          <hr className="border-slate-50 dark:border-slate-800/40" />

          {/* Sliders and Switch Controls */}
          <div className="space-y-4">
            <div className="flex justify-between items-center text-sm font-semibold text-slate-700 dark:text-slate-300">
              <span>Độ dài mật khẩu:</span>
              <span className="font-mono text-blue-600 dark:text-blue-400 font-bold text-base">{length} ký tự</span>
            </div>
            
            <input
              type="range"
              min={8}
              max={64}
              value={length}
              onChange={(e) => setLength(parseInt(e.target.value))}
              className="w-full h-1.5 bg-slate-150 dark:bg-slate-800 rounded-lg appearance-none cursor-pointer accent-blue-600 range"
            />

            <div className="grid grid-cols-1 sm:grid-cols-2 gap-4 pt-4">
              <label className="flex items-center gap-3 p-3.5 bg-slate-50 dark:bg-[#2C2C2E]/50 rounded-xl cursor-pointer hover:bg-slate-100/70 dark:hover:bg-[#2C2C2E]/80 transition-colors select-none">
                <input
                  type="checkbox"
                  checked={includeUppercase}
                  onChange={(e) => setIncludeUppercase(e.target.checked)}
                  className="w-4.5 h-4.5 text-blue-600 border-slate-300 rounded focus:ring-blue-500 cursor-pointer"
                />
                <div className="text-xs">
                  <span className="font-semibold text-slate-700 dark:text-slate-200 block">Ký tự chữ HOA (A-Z)</span>
                  <span className="text-slate-400 font-medium">Bao gồm các ký tự In Hoa</span>
                </div>
              </label>

              <label className="flex items-center gap-3 p-3.5 bg-slate-50 dark:bg-[#2C2C2E]/50 rounded-xl cursor-pointer hover:bg-slate-100/70 dark:hover:bg-[#2C2C2E]/80 transition-colors select-none">
                <input
                  type="checkbox"
                  checked={includeLowercase}
                  onChange={(e) => setIncludeLowercase(e.target.checked)}
                  className="w-4.5 h-4.5 text-blue-600 border-slate-300 rounded focus:ring-blue-500 cursor-pointer"
                />
                <div className="text-xs">
                  <span className="font-semibold text-slate-700 dark:text-slate-200 block">Ký tự chữ thường (a-z)</span>
                  <span className="text-slate-400 font-medium">Bao gồm chữ cái thường tiếng Anh</span>
                </div>
              </label>

              <label className="flex items-center gap-3 p-3.5 bg-slate-50 dark:bg-[#2C2C2E]/50 rounded-xl cursor-pointer hover:bg-slate-100/70 dark:hover:bg-[#2C2C2E]/80 transition-colors select-none">
                <input
                  type="checkbox"
                  checked={includeNumbers}
                  onChange={(e) => setIncludeNumbers(e.target.checked)}
                  className="w-4.5 h-4.5 text-blue-600 border-slate-300 rounded focus:ring-blue-500 cursor-pointer"
                />
                <div className="text-xs">
                  <span className="font-semibold text-slate-700 dark:text-slate-200 block">Ký tự chữ số (0-9)</span>
                  <span className="text-slate-400 font-medium">Bao gồm số từ 0 đến 9</span>
                </div>
              </label>

              <label className="flex items-center gap-3 p-3.5 bg-slate-50 dark:bg-[#2C2C2E]/50 rounded-xl cursor-pointer hover:bg-slate-100/70 dark:hover:bg-[#2C2C2E]/80 transition-colors select-none">
                <input
                  type="checkbox"
                  checked={includeSymbols}
                  onChange={(e) => setIncludeSymbols(e.target.checked)}
                  className="w-4.5 h-4.5 text-blue-600 border-slate-300 rounded focus:ring-blue-500 cursor-pointer"
                />
                <div className="text-xs">
                  <span className="font-semibold text-slate-700 dark:text-slate-200 block">Ký tự đặc biệt (!@#...)</span>
                  <span className="text-slate-400 font-medium">Bao gồm ký hiệu đặc biệt bảo mật</span>
                </div>
              </label>
            </div>
          </div>
        </div>

        {/* Right Side: Generated History Logs for quick copying */}
        <div className="space-y-4">
          <div className="bg-white dark:bg-[#1C1C1E] rounded-2xl p-5 shadow-[0_8px_30px_rgba(0,0,0,0.03)] space-y-4">
            <h3 className="font-display font-bold text-sm text-slate-800 dark:text-slate-200 uppercase tracking-wider flex items-center gap-2">
              <Sparkles size={16} className="text-amber-500" />
              <span>Gợi ý mật khẩu vừa tạo</span>
            </h3>

            <div className="space-y-2 max-h-[300px] overflow-y-auto pr-1">
              {history.length === 0 ? (
                <p className="text-xs text-slate-400 italic">Chưa có lịch sử tạo mật khẩu.</p>
              ) : (
                history.map((pw, index) => (
                  <div key={index} className="flex items-center justify-between bg-slate-50 dark:bg-[#161618]/55 p-2.5 rounded-xl hover:bg-slate-100/70 dark:hover:bg-[#1C1C1E] transition-colors group">
                    <span className="font-mono text-xs text-slate-600 dark:text-slate-300 truncate max-w-[170px]">{pw}</span>
                    <button
                      onClick={() => {
                        navigator.clipboard.writeText(pw);
                        const orig = generatedPassword;
                        setGeneratedPassword(pw);
                        setCopied(true);
                        setTimeout(() => {
                          setCopied(false);
                        }, 1500);
                      }}
                      className="p-1 px-2 bg-white dark:bg-slate-800 rounded text-[10px] font-bold text-slate-500 hover:text-blue-600 border border-slate-200 dark:border-slate-700 shadow-sm transition-colors flex items-center gap-1 cursor-pointer"
                    >
                      <Copy size={10} />
                      <span>Sử dụng</span>
                    </button>
                  </div>
                ))
              )}
            </div>
          </div>
        </div>

      </div>
    </div>
  );
}
