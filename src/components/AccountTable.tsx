/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

import React from 'react';
import { 
  Star, 
  Eye, 
  EyeOff, 
  Copy, 
  Edit3, 
  Trash2, 
  Check, 
  MoreVertical, 
  Filter, 
  Plus, 
  ChevronLeft, 
  ChevronRight,
  ShieldCheck,
  Globe,
  Search
} from 'lucide-react';
import { Account } from '../types';

interface AccountTableProps {
  accounts: Account[];
  selectedCategory: string | null;
  searchQuery: string;
  setSearchQuery: (query: string) => void;
  selectedAccountId: string | null;
  onSelectAccount: (account: Account) => void;
  onToggleFavorite: (id: string, e: React.MouseEvent) => void;
  onEditAccount: (account: Account, e: React.MouseEvent) => void;
  onDeleteAccount: (id: string, e: React.MouseEvent) => void;
  onAddAccount: () => void;
  activeTab: string; // 'accounts' | 'favorites' | 'trash'
  isMiniApp?: boolean;
}

export default function AccountTable({
  accounts,
  selectedCategory,
  searchQuery,
  setSearchQuery,
  selectedAccountId,
  onSelectAccount,
  onToggleFavorite,
  onEditAccount,
  onDeleteAccount,
  onAddAccount,
  activeTab,
  isMiniApp = false
}: AccountTableProps) {
  const [currentPage, setCurrentPage] = React.useState(1);
  const [copiedId, setCopiedId] = React.useState<string | null>(null);
  const [itemsPerPage, setItemsPerPage] = React.useState(15);

  // Local mini-app overrides for tab/category switching
  const [localTab, setLocalTab] = React.useState<'accounts' | 'favorites' | 'trash'>('accounts');
  const [localCategory, setLocalCategory] = React.useState<string | null>(null);

  const activeTabEffective = isMiniApp ? localTab : activeTab;
  const selectedCategoryEffective = isMiniApp ? localCategory : selectedCategory;

  // Filter states
  const [filterStrength, setFilterStrength] = React.useState<'all' | 'weak' | 'strong'>('all');
  const [filter2FA, setFilter2FA] = React.useState<'all' | 'has2fa' | 'no2fa'>('all');
  const [sortBy, setSortBy] = React.useState<'newest' | 'oldest' | 'alphabetical'>('newest');
  const [showFiltersDropdown, setShowFiltersDropdown] = React.useState(false);

  // Ref to detect click outside dropdown
  const dropdownRef = React.useRef<HTMLDivElement>(null);

  React.useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
        setShowFiltersDropdown(false);
      }
    }
    document.addEventListener("mousedown", handleClickOutside);
    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, []);

  // Header Title mapping
  let title = "Tất cả tài khoản";
  if (activeTabEffective === 'favorites') title = "Tài khoản ưa thích";
  else if (activeTabEffective === 'trash') title = "Thùng rác mật khẩu";
  else if (selectedCategoryEffective) title = selectedCategoryEffective;

  // Filter accounts based on tab, category, search
  const filteredAccounts = React.useMemo(() => {
    const rawFiltered = accounts.filter((account) => {
      // 1. Check trash status
      if (activeTabEffective === 'trash') {
        if (!account.isDeleted) return false;
      } else {
        if (account.isDeleted) return false;
      }

      // 2. Check favorites
      if (activeTabEffective === 'favorites') {
        if (!account.isFavorite) return false;
      }

      // 3. Check selected category
      if (selectedCategoryEffective && activeTabEffective === 'accounts') {
        if (account.category !== selectedCategoryEffective) return false;
      }

      // 4. Check search query
      if (searchQuery.trim() !== '') {
        const query = searchQuery.toLowerCase();
        const matchesQuery = (
          account.alias.toLowerCase().includes(query) ||
          account.appName.toLowerCase().includes(query) ||
          account.username.toLowerCase().includes(query) ||
          (account.notes && account.notes.toLowerCase().includes(query))
        );
        if (!matchesQuery) return false;
      }

      // 5. Check strength
      if (filterStrength !== 'all') {
        const pwd = account.passwordDecrypted;
        const hasUpper = /[A-Z]/.test(pwd);
        const hasLower = /[a-z]/.test(pwd);
        const hasDigit = /[0-9]/.test(pwd);
        const hasSpecial = /[^A-Za-z0-9]/.test(pwd);
        const isTooShort = pwd.length < 10;
        const complexityCount = [hasUpper, hasLower, hasDigit, hasSpecial].filter(Boolean).length;
        const isWeak = isTooShort || complexityCount < 3;
        
        if (filterStrength === 'weak' && !isWeak) return false;
        if (filterStrength === 'strong' && isWeak) return false;
      }

      // 6. Check 2FA
      if (filter2FA !== 'all') {
        const hasOTP = !!account.otpSecret && account.otpSecret.trim() !== '';
        if (filter2FA === 'has2fa' && !hasOTP) return false;
        if (filter2FA === 'no2fa' && hasOTP) return false;
      }

      return true;
    });

    // Sorting
    return [...rawFiltered].sort((a, b) => {
      if (sortBy === 'newest') {
        return new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime();
      }
      if (sortBy === 'oldest') {
        return new Date(a.updatedAt).getTime() - new Date(b.updatedAt).getTime();
      }
      if (sortBy === 'alphabetical') {
        return a.alias.localeCompare(b.alias);
      }
      return 0;
    });
  }, [accounts, activeTabEffective, selectedCategoryEffective, searchQuery, filterStrength, filter2FA, sortBy]);

  // Handle pagination
  const totalPages = Math.max(1, Math.ceil(filteredAccounts.length / itemsPerPage));
  
  React.useEffect(() => {
    setCurrentPage(1);
  }, [selectedCategoryEffective, searchQuery, activeTabEffective, filterStrength, filter2FA]);

  const displayedAccounts = React.useMemo(() => {
    const startIndex = (currentPage - 1) * itemsPerPage;
    return filteredAccounts.slice(startIndex, startIndex + itemsPerPage);
  }, [filteredAccounts, currentPage]);

  const handleCopyPassword = (account: Account, e: React.MouseEvent) => {
    e.stopPropagation();
    navigator.clipboard.writeText(account.passwordDecrypted);
    setCopiedId(account.id);
    setTimeout(() => setCopiedId(null), 2000);
  };

  // Helper to determine brand logo color configuration
  const getBrandConfig = (domain: string) => {
    const d = domain.toLowerCase();
    if (d.includes('google') || d.includes('gmail')) {
      return { letter: 'G', bg: 'bg-red-500 text-white', color: 'text-red-500' };
    }
    if (d.includes('facebook')) {
      return { letter: 'F', bg: 'bg-blue-600 text-white', color: 'text-blue-600' };
    }
    if (d.includes('instagram')) {
      return { letter: 'I', bg: 'bg-pink-500 text-white', color: 'text-pink-500' };
    }
    if (d.includes('zalo')) {
      return { letter: 'Z', bg: 'bg-blue-500 text-white', color: 'text-blue-500' };
    }
    if (d.includes('steam')) {
      return { letter: 'S', bg: 'bg-slate-800 text-white', color: 'text-slate-800' };
    }
    if (d.includes('vietcombank') || d.includes('vcb')) {
      return { letter: 'V', bg: 'bg-emerald-600 text-white', color: 'text-emerald-500' };
    }
    if (d.includes('github')) {
      return { letter: 'G', bg: 'bg-slate-900 text-white', color: 'text-slate-900' };
    }
    if (d.includes('chatgpt') || d.includes('openai')) {
      return { letter: 'O', bg: 'bg-teal-500 text-white', color: 'text-teal-500' };
    }
    if (d.includes('netflix')) {
      return { letter: 'N', bg: 'bg-red-600 text-white', color: 'text-red-600' };
    }
    if (d.includes('spotify')) {
      return { letter: 'S', bg: 'bg-green-500 text-white', color: 'text-green-500' };
    }
    return { letter: domain.charAt(0).toUpperCase(), bg: 'bg-blue-100 text-blue-600 dark:bg-blue-900 dark:text-blue-300', color: 'text-blue-500' };
  };

  // Helper to construct readable status duration string
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

  if (isMiniApp) {
    return (
      <div id="table-view-container" className="flex-1 flex flex-col min-w-0 bg-[#F5F5F7] dark:bg-[#151518] p-3 overflow-hidden select-none h-full max-h-full">
        {/* Search, Filter, Add Action Row */}
        <div className="flex items-center gap-1.5 mb-2.5 shrink-0">
          <div className="relative flex-1 group">
            <span className="absolute inset-y-0 left-0 flex items-center pl-2.5 text-slate-400">
              <Search size={14} />
            </span>
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Tìm tài khoản..."
              className="w-full bg-[#E5E5EA]/70 dark:bg-[#2C2C2E]/60 text-[11px] text-slate-850 dark:text-zinc-100 placeholder-slate-400/80 pl-8 pr-3 py-2 rounded-xl focus:bg-white dark:focus:bg-[#1E1E1E] focus:outline-none focus:ring-1 focus:ring-[#1c7bb6] transition-all font-sans"
            />
          </div>

          <div className="relative shrink-0" ref={dropdownRef}>
            <button 
              onClick={() => setShowFiltersDropdown(!showFiltersDropdown)}
              className={`p-2 rounded-xl border transition-all cursor-pointer flex items-center justify-center ${
                showFiltersDropdown || filterStrength !== 'all' || filter2FA !== 'all' || sortBy !== 'newest'
                  ? 'text-[#1c7bb6] bg-[#1c7bb6]/10 border-[#1c7bb6]/30'
                  : 'text-slate-600 dark:text-zinc-355 bg-white dark:bg-[#2C2C2E] border-slate-200 dark:border-zinc-800'
              }`}
              title="Bộ lọc"
            >
              <Filter size={14} />
            </button>

            {showFiltersDropdown && (
              <div className="absolute top-9 right-0 w-64 bg-white dark:bg-[#2C2C2E] rounded-2xl shadow-[0_8px_30px_rgba(0,0,0,0.15)] border border-[#E5E5EA] dark:border-zinc-800 p-3.5 z-50 text-left space-y-3 font-sans animate-in fade-in slide-in-from-top-1 duration-120">
                <div className="flex items-center justify-between border-b border-[#E5E5EA]/40 dark:border-zinc-800/40 pb-1.5">
                  <span className="text-[11px] font-bold text-slate-800 dark:text-zinc-200">Lọc danh sách</span>
                  <button 
                    onClick={() => {
                      setFilterStrength('all');
                      setFilter2FA('all');
                      setSortBy('newest');
                    }}
                    className="text-[10px] font-semibold text-[#1c7bb6] hover:underline cursor-pointer"
                  >
                    Reset
                  </button>
                </div>

                <div className="space-y-1">
                  <label className="text-[9px] font-bold text-slate-400 dark:text-zinc-500 uppercase tracking-widest">Độ mạnh Pass</label>
                  <div className="grid grid-cols-3 gap-1 bg-[#F5F5F7] dark:bg-[#1E1E1E] p-0.5 rounded-lg text-[10px] font-medium">
                    {[
                      { key: 'all', label: 'Tất cả' },
                      { key: 'weak', label: 'Yếu' },
                      { key: 'strong', label: 'Mạnh' }
                    ].map(opt => (
                      <button
                        key={opt.key}
                        onClick={() => setFilterStrength(opt.key as any)}
                        className={`py-0.5 rounded text-center cursor-pointer transition-all ${
                          filterStrength === opt.key 
                            ? 'bg-white dark:bg-[#2C2C2E] text-[#1c7bb6] font-semibold shadow-xs' 
                            : 'text-slate-500 dark:text-zinc-450'
                        }`}
                      >
                        {opt.label}
                      </button>
                    ))}
                  </div>
                </div>

                <div className="space-y-1">
                  <label className="text-[9px] font-bold text-slate-400 dark:text-zinc-500 uppercase tracking-widest">Bảo mật 2FA</label>
                  <div className="grid grid-cols-3 gap-1 bg-[#F5F5F7] dark:bg-[#1E1E1E] p-0.5 rounded-lg text-[10px] font-medium">
                    {[
                      { key: 'all', label: 'Tất cả' },
                      { key: 'has2fa', label: 'Có 2FA' },
                      { key: 'no2fa', label: 'K.Có' }
                    ].map(opt => (
                      <button
                        key={opt.key}
                        onClick={() => setFilter2FA(opt.key as any)}
                        className={`py-0.5 rounded text-center cursor-pointer transition-all ${
                          filter2FA === opt.key 
                            ? 'bg-white dark:bg-[#2C2C2E] text-[#1c7bb6] font-semibold shadow-xs' 
                            : 'text-slate-505 dark:text-zinc-450'
                        }`}
                      >
                        {opt.label}
                      </button>
                    ))}
                  </div>
                </div>

                <div className="space-y-1">
                  <label className="text-[9px] font-bold text-slate-400 dark:text-zinc-500 uppercase tracking-widest">Sắp xếp</label>
                  <select
                    value={sortBy}
                    onChange={(e) => setSortBy(e.target.value as any)}
                    className="w-full bg-[#F5F5F7] dark:bg-[#1E1E1E] text-[10px] px-2 py-1 rounded-lg border border-[#E5E5EA] dark:border-zinc-800 focus:outline-none text-slate-700 dark:text-zinc-300 font-medium cursor-pointer"
                  >
                    <option value="newest">⌛ Mới cập nhật</option>
                    <option value="oldest">🕰️ Cũ nhất trước</option>
                    <option value="alphabetical">🔤 Tên (A-Z)</option>
                  </select>
                </div>
              </div>
            )}
          </div>

          {activeTabEffective !== 'trash' && (
            <button
              onClick={onAddAccount}
              className="p-2 text-white bg-[#1c7bb6] hover:bg-[#156190] rounded-xl flex items-center justify-center cursor-pointer transition-all active:scale-[0.95] shrink-0"
              title="Thêm tài khoản"
            >
              <Plus size={14} className="stroke-[2.5]" />
            </button>
          )}
        </div>

        {/* iOS-Style Segmented Tab Switcher */}
        <div className="grid grid-cols-3 gap-1 bg-[#E5E5EA]/70 dark:bg-[#2C2C2E]/60 p-0.5 rounded-xl text-[10px] font-bold mb-2.5 shrink-0">
          {[
            { key: 'accounts', label: 'Cơ sở dữ liệu' },
            { key: 'favorites', label: 'Yêu thích ⭐' },
            { key: 'trash', label: 'Thùng rác 🗑️' }
          ].map(t => (
            <button
              key={t.key}
              onClick={() => {
                setLocalTab(t.key as any);
                setLocalCategory(null); // Reset category selection for a fresh view
              }}
              className={`py-1.5 rounded-lg text-center cursor-pointer transition-all ${
                localTab === t.key 
                  ? 'bg-white dark:bg-[#1C1C1E] text-[#1c7bb6] dark:text-sky-400 font-bold shadow-xs' 
                  : 'text-slate-550 dark:text-zinc-400'
              }`}
            >
              {t.label}
            </button>
          ))}
        </div>

        {/* Swipe-friendly Horizontal Category Filters */}
        {localTab !== 'trash' && (
          <div className="flex gap-1 overflow-x-auto pb-2 scrollbar-none shrink-0 text-[10px] font-bold -mx-1 px-1 mb-1">
            {[
              { key: null, label: '📂 Tất cả' },
              { key: 'Email', label: '📧 Email' },
              { key: 'Social', label: '👥 Social' },
              { key: 'Finance', label: '💵 Finance' },
              { key: 'Work', label: '💼 Work' },
              { key: 'Education', label: '🎓 Edu' }
            ].map(cat => {
              const isActive = localCategory === cat.key;
              return (
                <button
                  key={cat.label}
                  onClick={() => setLocalCategory(cat.key)}
                  className={`px-3 py-1 rounded-full whitespace-nowrap transition-colors cursor-pointer border ${
                    isActive 
                      ? 'bg-[#1c7bb6] text-white border-transparent shadow-xs font-bold' 
                      : 'bg-white dark:bg-[#2A2A2C] text-slate-650 dark:text-zinc-300 border-slate-205 dark:border-zinc-800'
                  }`}
                >
                  {cat.label}
                </button>
              );
            })}
          </div>
        )}

        {/* Mobile-optimized accounts list */}
        <div className="flex-1 min-h-0 overflow-y-auto space-y-2">
          {displayedAccounts.length === 0 ? (
            <div className="h-48 flex flex-col items-center justify-center p-4 text-center">
              <Globe size={24} className="text-slate-400 mb-2" />
              <span className="font-semibold text-slate-700 dark:text-slate-200 text-xs">Không tìm thấy tài khoản</span>
              <p className="text-[10px] text-slate-400 max-w-[200px] mt-0.5">Không khớp bộ lọc hoặc từ khóa tìm kiếm.</p>
            </div>
          ) : (
            <div className="bg-white dark:bg-[#1C1C1E] border border-[#E5E5EA]/40 dark:border-zinc-800/40 rounded-2xl divide-y divide-[#E5E5EA]/10 dark:divide-zinc-800/40 overflow-hidden shadow-xs">
              {displayedAccounts.map((account) => {
                const bLogo = getBrandConfig(account.appName);
                const isSelected = selectedAccountId === account.id;
                
                return (
                  <div
                    key={account.id}
                    onClick={() => onSelectAccount(account)}
                    className={`p-3 flex items-center justify-between gap-2.5 hover:bg-[#F2F2F7]/40 dark:hover:bg-[#1C1C1E]/30 transition-colors cursor-pointer ${
                      isSelected ? 'bg-[#1c7bb6]/5 dark:bg-[#1c7bb6]/10' : ''
                    }`}
                  >
                    {/* Brand avatar */}
                    <div className={`w-8 h-8 rounded-xl font-bold flex items-center justify-center text-xs shrink-0 shadow-xs ${bLogo.bg}`}>
                      {bLogo.letter}
                    </div>

                    {/* Meta information */}
                    <div className="min-w-0 flex-1 leading-tight">
                      <div className="flex items-center gap-1 flex-wrap">
                        <span className="font-bold text-xs text-slate-800 dark:text-zinc-100 truncate max-w-[120px]">
                          {account.alias}
                        </span>
                        {account.isFavorite && (
                          <Star size={10} className="text-amber-500 shrink-0" fill="currentColor" />
                        )}
                        {account.otpSecret && (
                          <span className="bg-emerald-100/60 dark:bg-emerald-950/30 text-emerald-700 dark:text-emerald-450 text-[8px] font-bold px-1.5 py-0.1 rounded-full border border-emerald-205/30 shrink-0">
                            2FA
                          </span>
                        )}
                      </div>
                      <span className="text-[10px] text-slate-400 dark:text-zinc-500 font-mono block truncate mt-0.5">
                        {account.username}
                      </span>
                      <span className="text-[9px] text-slate-300 dark:text-zinc-600 block truncate">
                        {account.appName}
                      </span>
                    </div>

                    {/* Action buttons (Copy, Star) */}
                    <div className="flex items-center gap-1 shrink-0" onClick={(e) => e.stopPropagation()}>
                      <button
                        onClick={(e) => onToggleFavorite(account.id, e)}
                        className={`p-1.5 rounded-lg transition-colors bg-slate-50 dark:bg-[#2C2C2E] cursor-pointer ${
                          account.isFavorite ? 'text-amber-500' : 'text-slate-400 dark:text-zinc-500'
                        }`}
                      >
                        <Star size={11} fill={account.isFavorite ? "currentColor" : "none"} />
                      </button>
                      <button
                        onClick={(e) => handleCopyPassword(account, e)}
                        className="p-1.5 rounded-lg bg-[#1c7bb6]/10 dark:bg-[#1c7bb6]/20 text-[#1c7bb6] hover:text-[#156190] dark:text-sky-450 transition-colors cursor-pointer flex items-center justify-center"
                        title="Sao chép mật khẩu"
                      >
                        {copiedId === account.id ? (
                          <Check size={11} className="text-[#30D158]" />
                        ) : (
                          <Copy size={11} />
                        )}
                      </button>
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>

        {/* Lightweight Mobile Pagination */}
        {filteredAccounts.length > 0 && (
          <div className="border-t border-[#E5E5EA]/40 dark:border-zinc-800/40 pt-2 px-1 flex items-center justify-between mt-2 select-none shrink-0 text-[10px] text-slate-400 dark:text-zinc-500">
            <span>
              {currentPage}/{totalPages} trang ({filteredAccounts.length} mục)
            </span>
            <div className="flex items-center gap-1">
              <button
                onClick={() => setCurrentPage(p => Math.max(1, p - 1))}
                disabled={currentPage === 1}
                className="p-1 rounded-md border border-[#E5E5EA] dark:border-[#3A3A3C] hover:bg-slate-100 dark:hover:bg-[#2C2C2E] text-slate-500 disabled:opacity-30 transition-colors cursor-pointer"
              >
                <ChevronLeft size={11} />
              </button>
              <button
                onClick={() => setCurrentPage(p => Math.min(totalPages, p + 1))}
                disabled={currentPage === totalPages}
                className="p-1 rounded-md border border-[#E5E5EA] dark:border-[#3A3A3C] hover:bg-slate-100 dark:hover:bg-[#2C2C2E] text-slate-500 disabled:opacity-30 transition-colors cursor-pointer"
              >
                <ChevronRight size={11} />
              </button>
            </div>
          </div>
        )}
      </div>
    );
  }

  return (
    <div id="table-view-container" className="flex-1 flex flex-col min-w-0 bg-[#F5F5F7] dark:bg-[#1A1A1E] p-3 sm:p-4 overflow-y-auto select-none">
      
      {/* Title block with action buttons */}
      <div id="table-header-block" className="flex items-center justify-between gap-1.5 sm:gap-3 mb-3 sm:mb-4 shrink-0">
        <div id="search-input-box" className="relative group flex-grow sm:max-w-xs md:max-w-sm shrink-0">
          <span className="absolute inset-y-0 left-0 flex items-center pl-2.5 sm:pl-3 text-slate-400">
            <Search size={14} className="transition-colors group-focus-within:text-[#007AFF] sm:w-4 sm:h-4" />
          </span>
          <input
            id="global-search-input"
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Tìm kiếm tài khoản..."
            className="w-full bg-[#E5E5EA]/70 dark:bg-[#2C2C2E] text-[11px] sm:text-xs text-slate-850 dark:text-zinc-100 placeholder-slate-400/80 pl-8 sm:pl-9 pr-3 sm:pr-4 py-1.5 sm:py-2.5 rounded-xl border border-slate-200/50 dark:border-zinc-800 focus:bg-white dark:focus:bg-[#1E1E1E] focus:outline-none focus:ring-2 focus:ring-[#007AFF]/35 dark:focus:ring-[#0A84FF]/35 transition-all font-sans"
          />
        </div>

        <div id="table-header-actions" className="flex items-center gap-1 sm:gap-2 shrink-0 relative">
          <div className="relative" ref={dropdownRef}>
            <button 
              id="btn-show-filters" 
              onClick={() => setShowFiltersDropdown(!showFiltersDropdown)}
              className={`flex items-center gap-1 sm:gap-1.5 px-2.5 sm:px-3 py-1.5 sm:py-2 text-xs sm:text-sm font-semibold rounded-xl border transition-all cursor-pointer ${
                showFiltersDropdown || filterStrength !== 'all' || filter2FA !== 'all' || sortBy !== 'newest'
                  ? 'text-[#007AFF] dark:text-[#0A84FF] bg-[#007AFF]/10 dark:bg-[#0A84FF]/10 border-[#007AFF]/50 dark:border-[#0A84FF]/50'
                  : 'text-slate-600 dark:text-zinc-300 bg-white dark:bg-[#2C2C2E] border-slate-250 dark:border-zinc-800 hover:bg-slate-50 dark:hover:bg-zinc-800/80'
              }`}
            >
              <Filter size={13} className="sm:w-3.5 sm:h-3.5" />
              <span className="hidden sm:inline">Bộ lọc</span>
              {(filterStrength !== 'all' || filter2FA !== 'all' || sortBy !== 'newest') && (
                <span className="w-1.5 h-1.5 rounded-full bg-[#007AFF] dark:bg-[#0A84FF] animate-pulse"></span>
              )}
            </button>

            {showFiltersDropdown && (
              <div id="filters-dropdown-panel" className="absolute top-10 sm:top-11 right-0 w-64 sm:w-72 bg-white dark:bg-[#2C2C2E] rounded-2xl shadow-[0_12px_40px_rgba(0,0,0,0.12)] border border-[#E5E5EA] dark:border-zinc-800 p-3 sm:p-4 z-50 text-left space-y-3 sm:space-y-4 font-sans animate-in fade-in slide-in-from-top-2 duration-150">
                {/* Header of dropdown */}
                <div className="flex items-center justify-between border-b border-[#E5E5EA]/40 dark:border-zinc-800/40 pb-2">
                  <span className="text-xs font-bold text-slate-800 dark:text-zinc-200">Cấu hình bộ lọc</span>
                  <button 
                    onClick={() => {
                      setFilterStrength('all');
                      setFilter2FA('all');
                      setSortBy('newest');
                    }}
                    className="text-[10px] font-semibold text-[#007AFF] dark:text-[#0A84FF] hover:underline cursor-pointer"
                  >
                    Xóa tất cả
                  </button>
                </div>

                {/* Filter Strength */}
                <div className="space-y-1.5">
                  <label className="text-[10px] font-bold text-slate-400 dark:text-zinc-500 uppercase tracking-wider">Độ mạnh mật khẩu</label>
                  <div className="grid grid-cols-3 gap-1 bg-[#F5F5F7] dark:bg-[#1E1E1E] p-0.5 rounded-lg text-[11px] font-medium">
                    {[
                      { key: 'all', label: 'Tất cả' },
                      { key: 'weak', label: 'Yếu' },
                      { key: 'strong', label: 'Mạnh' }
                    ].map(opt => (
                      <button
                        key={opt.key}
                        onClick={() => setFilterStrength(opt.key as any)}
                        className={`py-1 rounded text-center cursor-pointer transition-all ${
                          filterStrength === opt.key 
                            ? 'bg-white dark:bg-[#2C2C2E] text-[#007AFF] dark:text-[#0A84FF] font-semibold shadow-xs' 
                            : 'text-slate-505 dark:text-zinc-400 hover:text-slate-800 dark:hover:text-zinc-200'
                        }`}
                      >
                        {opt.label}
                      </button>
                    ))}
                  </div>
                </div>

                {/* Filter 2FA */}
                <div className="space-y-1.5">
                  <label className="text-[10px] font-bold text-slate-400 dark:text-zinc-500 uppercase tracking-wider">Bảo mật 2FA</label>
                  <div className="grid grid-cols-3 gap-1 bg-[#F5F5F7] dark:bg-[#1E1E1E] p-0.5 rounded-lg text-[11px] font-medium">
                    {[
                      { key: 'all', label: 'Tất cả' },
                      { key: 'has2fa', label: 'Có 2FA' },
                      { key: 'no2fa', label: 'Không 2FA' }
                    ].map(opt => (
                      <button
                        key={opt.key}
                        onClick={() => setFilter2FA(opt.key as any)}
                        className={`py-1 rounded text-center cursor-pointer transition-all ${
                          filter2FA === opt.key 
                            ? 'bg-white dark:bg-[#2C2C2E] text-[#007AFF] dark:text-[#0A84FF] font-semibold shadow-xs' 
                            : 'text-slate-505 dark:text-zinc-400 hover:text-slate-800 dark:hover:text-zinc-200'
                        }`}
                      >
                        {opt.label}
                      </button>
                    ))}
                  </div>
                </div>

                {/* Sort By Option */}
                <div className="space-y-1.5">
                  <label className="text-[10px] font-bold text-slate-400 dark:text-zinc-500 uppercase tracking-wider">Sắp xếp theo</label>
                  <select
                    value={sortBy}
                    onChange={(e) => setSortBy(e.target.value as any)}
                    className="w-full bg-[#F5F5F7] dark:bg-[#1E1E1E] text-xs px-3 py-1.5 rounded-lg border border-[#E5E5EA] dark:border-zinc-800 focus:outline-none text-slate-700 dark:text-zinc-300 font-medium cursor-pointer"
                  >
                    <option value="newest">⌛ Mới cập nhật trước</option>
                    <option value="oldest">🕰️ Cũ nhất trước</option>
                    <option value="alphabetical">🔤 Tên ứng dụng (A-Z)</option>
                  </select>
                </div>

                {/* Footer apply button */}
                <div className="pt-2 border-t border-[#E5E5EA]/40 dark:border-zinc-800/40 flex justify-end">
                  <button
                    onClick={() => setShowFiltersDropdown(false)}
                    className="px-3 py-1 bg-[#007AFF] dark:bg-[#0A84FF] text-white text-[11px] font-bold rounded-lg hover:opacity-90 active:scale-95 transition-all cursor-pointer"
                  >
                    Xong
                  </button>
                </div>
              </div>
            )}
          </div>
          
          {activeTab !== 'trash' && (
            <button
              id="btn-add-account-trigger"
              onClick={onAddAccount}
              className="flex items-center gap-1 sm:gap-1.5 px-2.5 sm:px-4.5 py-1.5 sm:py-2 text-xs sm:text-sm font-semibold text-white bg-[#007AFF] hover:bg-[#0063E1] dark:bg-[#0A84FF] dark:hover:bg-[#0072E3] rounded-xl cursor-pointer transition-all active:scale-[0.98] shadow-sm shrink-0"
            >
              <Plus size={14} className="stroke-[2.5] sm:w-4 sm:h-4" />
              <span className="hidden sm:inline">Thêm tài khoản</span>
            </button>
          )}
        </div>
      </div>

      {/* Main Table Grid Card */}
      <div id="table-results-card" className="flex-1 bg-white dark:bg-[#1C1C1E] rounded-2xl overflow-hidden flex flex-col shadow-[0_4px_24px_rgba(0,0,0,0.02)] border border-[#E5E5EA]/40 dark:border-[#2C2C2E]/60">
        
        {/* Responsive Table Area */}
        <div id="table-scroller" className="flex-1 overflow-x-auto">
          {displayedAccounts.length === 0 ? (
            <div id="empty-state-container" className="h-full flex flex-col items-center justify-center p-8 text-center">
              <div id="empty-state-icon-bg" className="w-16 h-16 rounded-2xl bg-slate-50 dark:bg-slate-800/50 flex items-center justify-center text-slate-400 mb-4 border border-slate-100 dark:border-slate-800">
                <Globe size={28} />
              </div>
              <h3 id="empty-state-title" className="font-semibold text-slate-700 dark:text-slate-200 text-base">Không tìm thấy tài khoản</h3>
              <p id="empty-state-desc" className="text-xs text-slate-400 max-w-sm mt-1">
                Không tìm thấy dữ liệu mật khẩu nào khớp với lọc tìm kiếm hoặc danh mục đã chọn.
              </p>
              {activeTab !== 'trash' && (
                <button
                  onClick={onAddAccount}
                  className="mt-4 px-4 py-2 bg-blue-50 hover:bg-blue-100 dark:bg-blue-950/40 dark:hover:bg-blue-950 text-blue-600 dark:text-blue-400 font-semibold rounded-xl text-xs transition-colors"
                >
                  Tạo tài khoản đầu tiên
                </button>
              )}
            </div>
          ) : (
            <table id="accounts-database-table" className="w-full text-left border-collapse">
              <thead>
                <tr className="border-b border-[#E5E5EA]/40 dark:border-[#2C2C2E] text-[11px] font-bold text-slate-400 dark:text-zinc-500 uppercase tracking-wider bg-slate-50/50 dark:bg-[#1E1E1E]/50 select-none">
                  <th className="py-2.5 px-3 w-10 text-center sm:px-5">Yêu thích</th>
                  <th className="py-2.5 px-3">Alias</th>
                  <th className="py-2.5 px-4 hidden md:table-cell">Website / App</th>
                  <th className="py-2.5 px-4 hidden sm:table-cell">Username</th>
                  <th className="py-2.5 px-4 hidden lg:table-cell">Cập nhật</th>
                  <th className="py-2.5 px-3 sm:px-5 text-right">Thao tác</th>
                </tr>
              </thead>
              <tbody id="accounts-database-tbody" className="divide-y divide-[#E5E5EA]/20 dark:divide-zinc-800/20">
                {displayedAccounts.map((account) => {
                  const bLogo = getBrandConfig(account.appName);
                  const isSelected = selectedAccountId === account.id;
                  
                  return (
                    <tr
                      id={`account-tr-${account.id}`}
                      key={account.id}
                      onClick={() => onSelectAccount(account)}
                      className={`group hover:bg-[#F2F2F7]/50 dark:hover:bg-[#1C1C1E]/30 cursor-pointer transition-colors ${
                        isSelected ? 'bg-[#007AFF]/5 dark:bg-[#0A84FF]/10' : ''
                      }`}
                    >
                      {/* Favorite Button */}
                      <td className="py-2 px-3 sm:px-5 text-center">
                        <button
                          id={`star-btn-${account.id}`}
                          onClick={(e) => onToggleFavorite(account.id, e)}
                          className={`p-1 rounded-md hover:bg-[#E5E5EA]/60 dark:hover:bg-zinc-805 transition-colors cursor-pointer ${
                            account.isFavorite ? 'text-amber-500' : 'text-slate-300 dark:text-zinc-600 group-hover:text-slate-400'
                          }`}
                          title={account.isFavorite ? "Loại bỏ khỏi ưa thích" : "Đánh dấu ưa thích"}
                        >
                          <Star size={14} fill={account.isFavorite ? "currentColor" : "none"} />
                        </button>
                      </td>

                      {/* Alias Domain Column */}
                      <td className="py-2 px-3">
                        <div className="flex items-center gap-2.5">
                          <div id={`brand-avatar-${account.id}`} className={`w-7.5 h-7.5 rounded-lg font-bold flex items-center justify-center text-[11px] shadow-xs shrink-0 ${bLogo.bg}`}>
                            {bLogo.letter}
                          </div>
                          <div className="min-w-0 flex-1">
                            <div className="flex items-center gap-1.5">
                              <span id={`alias-label-${account.id}`} className="font-semibold text-xs text-slate-800 dark:text-zinc-100 block group-hover:text-[#007AFF] dark:group-hover:text-[#0A84FF] transition-colors truncate leading-tight">
                                {account.alias}
                              </span>
                              {account.alternativePasswords && account.alternativePasswords.length > 0 && (
                                <span className="bg-amber-100/80 dark:bg-amber-950/40 text-amber-800 dark:text-amber-400 text-[8px] font-bold px-1.5 py-0.2 rounded-full border border-amber-205 dark:border-amber-900/30 whitespace-nowrap" title={`Có ${account.alternativePasswords.length + 1} mật khẩu khả dụng`}>
                                  {account.alternativePasswords.length + 1} Pass
                                </span>
                              )}
                            </div>
                            
                            {/* Mobile-only secondary block */}
                            <div className="flex flex-col text-[10px] text-slate-400 dark:text-zinc-500 mt-0.5 md:hidden gap-0.5">
                              <span className="truncate"><span className="font-medium text-slate-500">Web:</span> {account.appName}</span>
                              <span className="truncate sm:hidden"><span className="font-medium text-slate-500">User:</span> {account.username}</span>
                            </div>
                          </div>
                        </div>
                      </td>

                      {/* Website/App Column */}
                      <td className="py-2 px-4 hidden md:table-cell">
                        <span id={`appname-label-${account.id}`} className="text-xs font-medium text-slate-500 dark:text-zinc-400">
                          {account.appName}
                        </span>
                      </td>

                      {/* Username Column */}
                      <td className="py-2 px-4 hidden sm:table-cell max-w-[150px]">
                        <span id={`username-label-${account.id}`} className="text-xs text-slate-600 dark:text-zinc-350 font-mono font-medium truncate block">
                          {account.username}
                        </span>
                      </td>

                      {/* Update Time Column */}
                      <td className="py-2 px-4 hidden lg:table-cell">
                        <span id={`update-label-${account.id}`} className="text-[11px] font-medium text-slate-400 dark:text-zinc-500">
                          {formatTimeAgo(account.updatedAt)}
                        </span>
                      </td>

                      {/* Active Actions Column */}
                      <td className="py-2 px-3 sm:px-5 text-right" onClick={(e) => e.stopPropagation()}>
                        <div id={`actions-wrapper-${account.id}`} className="flex items-center justify-end gap-0.5">
                          
                          {/* Quick Copy Password inline */}
                          <button
                            id={`copy-btn-${account.id}`}
                            onClick={(e) => handleCopyPassword(account, e)}
                            className="p-1 rounded-md hover:bg-[#E5E5EA]/65 dark:hover:bg-[#2C2C2E] text-slate-400 hover:text-slate-800 dark:text-zinc-400 dark:hover:text-zinc-200 transition-colors cursor-pointer"
                            title="Sao chép mật khẩu"
                          >
                            {copiedId === account.id ? (
                              <Check size={13} className="text-[#30D158]" />
                            ) : (
                              <Copy size={13} />
                            )}
                          </button>

                          {/* Quick Edit inline */}
                          {activeTab !== 'trash' && (
                            <button
                              id={`edit-btn-${account.id}`}
                              onClick={(e) => onEditAccount(account, e)}
                              className="p-1 rounded-md hover:bg-[#E5E5EA]/65 dark:hover:bg-[#2C2C2E] text-slate-400 hover:text-slate-800 dark:text-zinc-400 dark:hover:text-zinc-200 transition-colors cursor-pointer"
                              title="Chỉnh sửa tài khoản"
                            >
                              <Edit3 size={13} />
                            </button>
                          )}

                          {/* Quick Delete inline */}
                          <button
                            id={`delete-btn-${account.id}`}
                            onClick={(e) => onDeleteAccount(account.id, e)}
                            className="p-1 rounded-md hover:bg-[#E5E5EA]/65 dark:hover:bg-[#2C2C2E] text-slate-400 hover:text-[#FF453A] dark:text-zinc-400 dark:hover:text-[#FF453A] transition-colors cursor-pointer"
                            title={activeTab === 'trash' ? "Xóa vĩnh viễn" : "Chuyển vào thùng rác"}
                          >
                            <Trash2 size={13} />
                          </button>
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          )}
        </div>

        {/* Footer info pagination row (Matching Image footer style) */}
        {filteredAccounts.length > 0 && (
          <div id="table-footer-pagination" className="border-t border-[#E5E5EA] dark:border-[#2C2C2E] px-3 sm:px-4 py-2 sm:py-3 flex flex-row items-center justify-between gap-3 select-none shrink-0 bg-slate-50/10 dark:bg-[#1C1C1E]/30">
            <div className="flex items-center gap-3">
              <span id="pagination-status-label" className="text-[10px] sm:text-xs font-semibold text-slate-400 dark:text-zinc-500">
                <span className="hidden xs:inline">Hiển thị </span>{((currentPage - 1) * itemsPerPage) + 1} - {Math.min(currentPage * itemsPerPage, filteredAccounts.length)} / {filteredAccounts.length}
              </span>
              <div className="hidden sm:flex items-center gap-1 bg-[#E5E5EA]/45 dark:bg-[#2C2C2E]/60 p-0.5 rounded-lg border border-[#E5E5EA]/70 dark:border-[#3A3A3C]/50 text-[10px] font-bold">
                {[10, 15, 25, 50, 100].map((size) => (
                  <button
                    key={size}
                    onClick={() => {
                      setItemsPerPage(size);
                      setCurrentPage(1);
                    }}
                    className={`px-2 py-0.5 rounded transition-all cursor-pointer leading-none ${
                      itemsPerPage === size 
                        ? 'bg-white dark:bg-[#1E1E1E] text-[#007AFF] dark:text-[#0A84FF] shadow-xs' 
                        : 'text-slate-505 dark:text-zinc-400 hover:text-slate-800 dark:hover:text-zinc-200'
                    }`}
                  >
                    {size}
                  </button>
                ))}
              </div>
            </div>

            <div id="pagination-buttons" className="flex items-center gap-1 px-0.5 font-sans justify-end shrink-0">
              <button
                id="pagination-btn-prev"
                onClick={() => setCurrentPage(p => Math.max(1, p - 1))}
                disabled={currentPage === 1}
                className="p-1 rounded-md border border-[#E5E5EA] dark:border-[#3A3A3C] hover:bg-slate-100 dark:hover:bg-[#2C2C2E] text-slate-500 hover:text-slate-800 dark:text-zinc-400 dark:hover:text-zinc-200 disabled:opacity-30 disabled:hover:bg-transparent transition-colors cursor-pointer"
              >
                <ChevronLeft size={12} className="sm:w-3.5 sm:h-3.5" />
              </button>

              <div id="desktop-numeric-pagination" className="hidden sm:flex items-center gap-1">
                {Array.from({ length: totalPages }, (_, i) => i + 1).map((page) => {
                  const isNearCurrent = Math.abs(page - currentPage) <= 1;
                  const isEdge = page === 1 || page === totalPages;
                  if (!isNearCurrent && !isEdge) {
                    if (page === 2 && currentPage > 3) {
                      return <span key={page} className="text-slate-300 dark:text-zinc-700 px-1 text-xs">...</span>;
                    }
                    if (page === totalPages - 1 && currentPage < totalPages - 2) {
                      return <span key={page} className="text-slate-300 dark:text-zinc-700 px-1 text-xs">...</span>;
                    }
                    return null;
                  }
                  return (
                    <button
                      id={`pagination-page-btn-${page}`}
                      key={page}
                      onClick={() => setCurrentPage(page)}
                      className={`w-5.5 h-5.5 sm:w-6 sm:h-6 rounded-md text-[11px] sm:text-xs font-bold transition-colors cursor-pointer ${
                        page === currentPage
                          ? 'bg-[#007AFF] dark:bg-[#0A84FF] text-white shadow-xs'
                          : 'border border-[#E5E5EA] dark:border-[#3A3A3C] hover:bg-slate-150 dark:hover:bg-[#2C2C2E] text-slate-600 dark:text-zinc-400 hover:text-slate-800 dark:hover:text-zinc-200'
                      }`}
                    >
                      {page}
                    </button>
                  );
                })}
              </div>

              {/* On narrow screen sizes, show a simple status label between arrow buttons instead of large numbers block */}
              <span className="sm:hidden text-[11px] font-bold text-slate-500 min-w-[32px] text-center shrink-0">
                {currentPage} / {totalPages}
              </span>

              <button
                id="pagination-btn-next"
                onClick={() => setCurrentPage(p => Math.min(totalPages, p + 1))}
                disabled={currentPage === totalPages}
                className="p-1 rounded-md border border-[#E5E5EA] dark:border-[#3A3A3C] hover:bg-slate-100 dark:hover:bg-[#2C2C2E] text-slate-500 hover:text-slate-800 dark:text-zinc-400 dark:hover:text-zinc-200 disabled:opacity-30 disabled:hover:bg-transparent transition-colors cursor-pointer"
              >
                <ChevronRight size={12} className="sm:w-3.5 sm:h-3.5" />
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
