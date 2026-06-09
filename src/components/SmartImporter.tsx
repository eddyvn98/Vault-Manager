/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

import React from 'react';
import { 
  Plus, 
  Trash2, 
  Check, 
  Sparkles, 
  Clipboard, 
  FileUp, 
  Upload, 
  AlertCircle, 
  Info, 
  Edit3, 
  CheckSquare, 
  Square,
  RefreshCw,
  Cpu
} from 'lucide-react';
import { GoogleGenAI } from "@google/genai";

interface ParsedItem {
  id: string;
  alias: string;
  appName: string;
  username: string;
  passwordDecrypted: string;
  category: string;
  notes: string;
  selected: boolean;
}

interface SmartImporterProps {
  onImportSuccess: (accounts: Omit<any, 'id' | 'createdAt' | 'updatedAt' | 'history' | 'isFavorite' | 'isDeleted'>[]) => void;
  categories: string[];
}

// Trình bóc tách thủ công bằng thuật toán Rules/Regex Offline
export function parseFreeTextOffline(text: string): Omit<ParsedItem, 'selected'>[] {
  const lines = text.split('\n');
  const items: Omit<ParsedItem, 'selected'>[] = [];
  let lastValidItem: any = null;

  for (let i = 0; i < lines.length; i++) {
    const rawLine = lines[i];
    const trimmedLine = rawLine.trim();

    if (!trimmedLine) continue;

    const isIndented = /^\s+/.test(rawLine);
    const words = trimmedLine.split(/\s+/);

    if (isIndented && words.length === 1 && lastValidItem) {
      const word = words[0];
      const category = lastValidItem.category;
      const newItem = {
        id: `parsed-${Date.now()}-${i}-${Math.random()}`,
        alias: `${lastValidItem.alias} (Dự phòng)`,
        appName: lastValidItem.appName,
        username: lastValidItem.username || 'Chưa rõ',
        passwordDecrypted: word,
        category: category,
        notes: `Mật khẩu dự phòng phát hiện thụt lề dưới dịch vụ: ${lastValidItem.alias}`
      };
      items.push(newItem);
      continue;
    }

    let alias = '';
    let appName = '';
    let username = '';
    let passwordDecrypted = '';
    let category = 'Khác';
    let notes = 'Nhập thông minh từ ghi chép tự do.';

    if (words.length === 1) {
      const word = words[0];
      alias = `Ghi chú #${items.length + 1}`;
      appName = 'Ngoại tuyến';
      passwordDecrypted = word;
      
      if (/^\d+$/.test(word)) {
        alias = `Mã PIN / OTP #${items.length + 1}`;
        notes = 'Mã số bảo mật tự do (PIN/OTP).';
      }
    } else if (words.length === 2) {
      alias = words[0];
      appName = `${words[0].toLowerCase()}.com`;
      passwordDecrypted = words[1];
    } else if (words.length === 3) {
      alias = words[0];
      appName = `${words[0].toLowerCase()}.com`;
      username = words[1];
      passwordDecrypted = words[2];
    } else {
      const lastWord = words[words.length - 1];
      passwordDecrypted = lastWord;

      const secondToLastWord = words[words.length - 2];
      const isAltUsername = secondToLastWord.length <= 15 && !/[^a-zA-Z0-9_@]/.test(secondToLastWord);
      
      if (isAltUsername) {
        username = secondToLastWord;
        const appWords = words.slice(0, words.length - 2);
        alias = appWords.join(' ');
        appName = `${appWords.join('').toLowerCase()}.com`;
      } else {
        const appWords = words.slice(0, words.length - 1);
        alias = appWords.join(' ');
        appName = `${appWords.join('').toLowerCase()}.com`;
      }
    }

    // Tự động phân loại danh mục qua tên dịch vụ
    const lowerAlias = alias.toLowerCase();
    if (lowerAlias.includes('mail') || lowerAlias.includes('gmail') || lowerAlias.includes('outlook') || lowerAlias.includes('yahoo')) {
      category = 'Email';
    } else if (lowerAlias.includes('fb') || lowerAlias.includes('facebook') || lowerAlias.includes('tw') || lowerAlias.includes('twitter') || lowerAlias.includes('tik') || lowerAlias.includes('tiktok') || lowerAlias.includes('insta') || lowerAlias.includes('zalo') || lowerAlias.includes('telegram') || lowerAlias.includes('tele')) {
      category = 'Mạng xã hội';
    } else if (lowerAlias.includes('bank') || lowerAlias.includes('vcb') || lowerAlias.includes('acb') || lowerAlias.includes('mbb') || lowerAlias.includes('pay') || lowerAlias.includes('coin') || lowerAlias.includes('crypto')) {
      category = 'Ngân hàng';
    } else if (lowerAlias.includes('work') || lowerAlias.includes('job') || lowerAlias.includes('git') || lowerAlias.includes('slack') || lowerAlias.includes('jira') || lowerAlias.includes('notion') || lowerAlias.includes('elastic') || lowerAlias.includes('cic')) {
      category = 'Công việc';
    } else if (lowerAlias.includes('netflix') || lowerAlias.includes('spotify') || lowerAlias.includes('youtube') || lowerAlias.includes('jellyfin') || lowerAlias.includes('flex')) {
      category = 'Giải trí';
    } else if (lowerAlias.includes('game') || lowerAlias.includes('steam') || lowerAlias.includes('epic') || lowerAlias.includes('garena') || lowerAlias.includes('riot')) {
      category = 'Game';
    }

    const newItem = {
      id: `parsed-${Date.now()}-${i}-${Math.random()}`,
      alias,
      appName,
      username,
      passwordDecrypted,
      category,
      notes
    };

    items.push(newItem);
    lastValidItem = newItem;
  }

  return items;
}

export default function SmartImporter({ onImportSuccess, categories }: SmartImporterProps) {
  const [activeImportMode, setActiveImportMode] = React.useState<'freetext' | 'file'>('freetext');
  const [freeTextValue, setFreeTextValue] = React.useState('');
  const [geminiKeyInput, setGeminiKeyInput] = React.useState(() => {
    return localStorage.getItem('user_gemini_api_key') || '';
  });
  
  // Trạng thái xử lý bóc tách
  const [isProcessing, setIsProcessing] = React.useState(false);
  const [errorStr, setErrorStr] = React.useState<string | null>(null);
  const [successCount, setSuccessCount] = React.useState<number | null>(null);
  
  // Mảng kết quả sau khi parse để preview
  const [previewItems, setPreviewItems] = React.useState<ParsedItem[]>([]);

  // Lưu API Key vào LocalStorage khi sửa đổi
  React.useEffect(() => {
    localStorage.setItem('user_gemini_api_key', geminiKeyInput);
  }, [geminiKeyInput]);

  // Paste mẫu văn bản của người dùng để trải nghiệm tức thì
  const handleLoadSampleData = () => {
    const sample = `Tw 5HgQB4D66U6Z
Tik dypn34u2tuec
  uu29L8X!?N]#2Q
Flex jellyfin Eddy ge35,LqS>=WZL*x
Cic WATN)h&Y4Uk>mQs
15742
Cake 9x.eb6Q6ttW 5XT
apple edy98 pVsRH4mf`;
    setFreeTextValue(sample);
    setErrorStr(null);
  };

  // Hàm Parser Offline (Không tốn API Key, 100% riêng tư)
  const handleOfflineParse = () => {
    if (!freeTextValue.trim()) {
      setErrorStr('Vui lòng nhập hoặc dán văn bản chứa thông tin mật khẩu của bạn.');
      return;
    }
    
    setIsProcessing(true);
    setErrorStr(null);
    
    setTimeout(() => {
      try {
        const results = parseFreeTextOffline(freeTextValue);
        const mappedResults: ParsedItem[] = results.map(item => ({
          ...item,
          selected: true
        }));
        
        setPreviewItems(mappedResults);
        if (mappedResults.length === 0) {
          setErrorStr('Không thể bóc tách bất kỳ dòng nào. Hãy chắc chắn văn bản không trống.');
        }
      } catch (err) {
        console.error(err);
        setErrorStr('Đã xảy ra lỗi khi phân tích văn bản. Vui lòng kiểm tra lại cấu trúc.');
      } finally {
        setIsProcessing(false);
      }
    }, 400);
  };

  // Hàm Parser thông minh qua Gemini AI
  const handleGeminiParse = async () => {
    if (!freeTextValue.trim()) {
      setErrorStr('Vui lòng nhập hoặc dán văn bản chứa thông tin mật khẩu của bạn.');
      return;
    }

    // Kiểm tra API Key: Ưu tiên dùng VITE_GEMINI_API_KEY nếu có, không thì đòi userKey nhập vào
    const apiKey = geminiKeyInput || (import.meta as any).env?.VITE_GEMINI_API_KEY || '';
    if (!apiKey) {
      setErrorStr('Vui lòng điền Gemini API Key của bạn để sử dụng tính năng phân tích AI thông minh, hoặc chọn "Phân tích Offline" hoàn toàn miễn phí!');
      return;
    }

    setIsProcessing(true);
    setErrorStr(null);

    try {
      const ai = new GoogleGenAI({
        apiKey: apiKey,
        httpOptions: {
          headers: {
            'User-Agent': 'aistudio-build',
          }
        }
      });

      const systemPrompt = `Phân tích đoạn văn bản chứa thông tin mật khẩu tự do dưới đây và trích xuất thành mảng JSON chứa các tài khoản mật khẩu.
Mỗi dòng đại diện cho một thông tin đăng nhập. Một số dòng có thể thụt lề (bắt đầu bằng khoảng trắng) tức là thông tin mật khẩu dự phòng hoặc tài khoản con của dòng ngay trước nó.

Văn bản cần phân tích:
${freeTextValue}

Hãy phân tích và trả về một mảng JSON các đối tượng. Mỗi đối tượng phải có cấu trúc chính xác như sau:
{
  "alias": "Tên viết tắt hoặc tên dịch vụ thân thiện, ví dụ 'Twitter', 'TikTok', 'Vietnam Airlines'",
  "appName": "Tên miền website hoặc tên ứng dụng, ví dụ 'twitter.com', 'tiktok.com'",
  "username": "Tên đăng nhập hoặc Email nếu có, không có thì viết rỗng ''",
  "passwordDecrypted": "Mật khẩu bóc tách được",
  "category": "Chọn một trong các danh mục sau: 'Email', 'Mạng xã hội', 'Ngân hàng', 'Công việc', 'Giải trí', 'Game', 'Khác'",
  "notes": "Ghi chú ngắn gọn về tài khoản này (ví dụ: 'Mật khẩu phụ phát hiện dưới Twitter' hoặc 'Nhập thông minh từ văn bản ghi chép tự do')."
}

Yêu cầu trả về CHỈ chuỗi mảng JSON thô hợp lệ bắt đầu bằng [ và kết thúc bằng ]. Không bao gồm các thẻ markdown \`\`\`json ở trước hay sau, không chứa các dòng giải thích nào khác.`;

      const response = await ai.models.generateContent({
        model: 'gemini-3.5-flash',
        contents: systemPrompt
      });

      let rawText = response.text || '';
      rawText = rawText.trim();
      
      // Clean up markdown code blocks if the model wrapped it with them
      if (rawText.startsWith('```json')) {
        rawText = rawText.substring(7);
      } else if (rawText.startsWith('```')) {
        rawText = rawText.substring(3);
      }
      
      if (rawText.endsWith('```')) {
        rawText = rawText.substring(0, rawText.length - 3);
      }
      rawText = rawText.trim();

      const parsedJson = JSON.parse(rawText);
      if (Array.isArray(parsedJson)) {
        const formattedItems: ParsedItem[] = parsedJson.map((item, idx) => ({
          id: `gemini-parsed-${Date.now()}-${idx}-${Math.random()}`,
          alias: item.alias || 'Chưa rõ',
          appName: item.appName || 'unnamed.com',
          username: item.username || '',
          passwordDecrypted: item.passwordDecrypted || '',
          category: categories.includes(item.category) ? item.category : 'Khác',
          notes: item.notes || 'Nhập bằng phân tích AI Gemini.',
          selected: true
        }));
        setPreviewItems(formattedItems);
      } else {
        throw new Error('Định dạng kết quả trả về từ AI không phải là mảng.');
      }

    } catch (err: any) {
      console.error(err);
      setErrorStr(`Lỗi phân tích AI: ${err.message || 'Lỗi không xác định'}. Bạn có thể sử dụng Trình Phân tích Trực tiếp (Offline) hoạt động ngay lập tức!`);
    } finally {
      setIsProcessing(false);
    }
  };

  // Xử lý kéo thả tệp tin CSV / JSON thật
  const handleFileDrop = (e: React.DragEvent<HTMLDivElement>) => {
    e.preventDefault();
    if (e.dataTransfer.files && e.dataTransfer.files[0]) {
      processSelectedFile(e.dataTransfer.files[0]);
    }
  };

  const handleFileSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
    if (e.target.files && e.target.files[0]) {
      processSelectedFile(e.target.files[0]);
    }
  };

  const processSelectedFile = (file: File) => {
    setErrorStr(null);
    setIsProcessing(true);

    const reader = new FileReader();
    reader.onload = (event) => {
      try {
        const content = event.target?.result as string;
        if (!content) throw new Error('Không đọc được nội dung tệp tin.');

        if (file.name.endsWith('.json')) {
          // Xử lý tệp JSON
          const data = JSON.parse(content);
          let rawAccounts: any[] = [];
          if (Array.isArray(data)) {
            rawAccounts = data;
          } else if (data.accounts && Array.isArray(data.accounts)) {
            rawAccounts = data.accounts;
          } else {
            throw new Error('Tệp JSON không chứa mảng tài khoản hợp lệ.');
          }

          const parsed: ParsedItem[] = rawAccounts.map((acc: any, idx: number) => ({
            id: `json-file-${Date.now()}-${idx}`,
            alias: acc.alias || acc.name || acc.title || 'unnamed',
            appName: acc.appName || acc.url || 'unknown.com',
            username: acc.username || acc.login || '',
            passwordDecrypted: acc.passwordDecrypted || acc.password || '',
            category: categories.includes(acc.category) ? acc.category : 'Khác',
            notes: acc.notes || 'Nhập từ tệp JSON.',
            selected: true
          }));
          setPreviewItems(parsed);

        } else if (file.name.endsWith('.csv')) {
          // Xử lý tệp CSV (Chrome / NordPass / Dashlane standard)
          const lines = content.split('\n');
          if (lines.length < 2) throw new Error('Tệp CSV trống hoặc không đúng định dạng.');

          // Đọc header để ánh xạ thông minh
          const header = lines[0].toLowerCase().split(',');
          const nameIdx = header.findIndex(h => h.includes('name') || h.includes('title') || h.includes('alias'));
          const urlIdx = header.findIndex(h => h.includes('url') || h.includes('website') || h.includes('app'));
          const userIdx = header.findIndex(h => h.includes('username') || h.includes('login') || h.includes('user') || h.includes('email'));
          const passIdx = header.findIndex(h => h.includes('password') || h.includes('pass'));
          const noteIdx = header.findIndex(h => h.includes('note') || h.includes('remark'));

          const parsed: ParsedItem[] = [];
          for (let i = 1; i < lines.length; i++) {
            const line = lines[i].trim();
            if (!line) continue;

            const cols = parseCSVLine(line);
            if (cols.length === 0) continue;

            const alias = nameIdx !== -1 && cols[nameIdx] ? cols[nameIdx] : 'Chrome Account';
            const appName = urlIdx !== -1 && cols[urlIdx] ? cols[urlIdx] : 'chrome.com';
            const username = userIdx !== -1 && cols[userIdx] ? cols[userIdx] : '';
            const passwordDecrypted = passIdx !== -1 && cols[passIdx] ? cols[passIdx] : '';
            const notes = noteIdx !== -1 && cols[noteIdx] ? cols[noteIdx] : 'Nhập từ tệp CSV.';

            parsed.push({
              id: `csv-file-${Date.now()}-${i}`,
              alias,
              appName,
              username,
              passwordDecrypted,
              category: 'Khác',
              notes,
              selected: true
            });
          }
          setPreviewItems(parsed);
        } else {
          throw new Error('Định dạng tệp không được hỗ trợ. Vui lòng chọn .csv hoặc .json');
        }
      } catch (err: any) {
        console.error(err);
        setErrorStr(`Lỗi đọc tệp tin: ${err.message || 'Lỗi cấu trúc hoặc định dạng'}`);
      } finally {
        setIsProcessing(false);
      }
    };

    reader.onerror = () => {
      setErrorStr('Lỗi không thể đọc tệp tin.');
      setIsProcessing(false);
    };

    reader.readAsText(file);
  };

  // Helper bóc tách dòng CSV nâng cao (xử lý dấu ngoặc kép bọc cột)
  const parseCSVLine = (line: string): string[] => {
    const result: string[] = [];
    let cur = '';
    let inQuotes = false;
    for (let i = 0; i < line.length; i++) {
      const c = line[i];
      if (c === '"') {
        inQuotes = !inQuotes;
      } else if (c === ',' && !inQuotes) {
        result.push(cur.trim());
        cur = '';
      } else {
        cur += c;
      }
    }
    result.push(cur.trim());
    return result;
  };

  // Inline Editing: thay đổi giá trị của cột trong bảng preview
  const handleUpdatePreviewField = (id: string, field: keyof ParsedItem, value: any) => {
    setPreviewItems(prev => prev.map(item => {
      if (item.id === id) {
        return { ...item, [field]: value };
      }
      return item;
    }));
  };

  // Toggle chọn dòng để import
  const handleToggleSelectItem = (id: string) => {
    setPreviewItems(prev => prev.map(item => {
      if (item.id === id) {
        return { ...item, selected: !item.selected };
      }
      return item;
    }));
  };

  // Toggle chọn tất cả
  const isAllSelected = previewItems.length > 0 && previewItems.every(i => i.selected);
  const handleToggleSelectAll = () => {
    setPreviewItems(prev => prev.map(item => ({
      ...item,
      selected: !isAllSelected
    })));
  };

  // Xóa bớt dòng trong Preview
  const handleDeletePreviewItem = (id: string) => {
    setPreviewItems(prev => prev.filter(item => item.id !== id));
  };

  // Thêm một dòng trống mới để điền thủ công
  const handleAddEmptyPreviewRow = () => {
    const newItem: ParsedItem = {
      id: `manually-added-${Date.now()}-${Math.random()}`,
      alias: 'Tài khoản mới',
      appName: 'facebook.com',
      username: '',
      passwordDecrypted: '',
      category: 'Khác',
      notes: 'Thêm thủ công trong bảng Preview.',
      selected: true
    };
    setPreviewItems(prev => [...prev, newItem]);
  };

  // Tiến hành Nhập Toàn Bộ Tài Khoản Đã Chọn vào Database thật
  const handleConfirmImport = () => {
    const itemsToImport = previewItems.filter(i => i.selected);
    if (itemsToImport.length === 0) {
      setErrorStr('Vui lòng chọn ít nhất một tài khoản hợp lệ để nhập vào hệ thống.');
      return;
    }

    const dataToPass = itemsToImport.map(item => ({
      alias: item.alias.trim() || 'unnamed-alias',
      appName: item.appName.trim() || 'unknown_app',
      username: item.username.trim() || '',
      passwordDecrypted: item.passwordDecrypted.trim(),
      category: item.category,
      notes: item.notes.trim()
    }));

    onImportSuccess(dataToPass);
    setSuccessCount(itemsToImport.length);
    setPreviewItems([]);
    setFreeTextValue('');
  };

  return (
    <div id="smart-importer-container" className="flex-1 overflow-y-auto bg-[#F5F5F7] dark:bg-[#1A1A1E] p-4 sm:p-8 flex flex-col items-center">
      <div className="w-full max-w-4xl space-y-6">
        
        {/* Header Block */}
        <div className="text-center sm:text-left space-y-1">
          <h2 className="font-display font-bold text-xl sm:text-2xl text-slate-800 dark:text-slate-100 flex items-center justify-center sm:justify-start gap-2.5">
            <FileUp className="text-blue-500 w-6 h-6 stroke-[2.5]" />
            Cổng Nhập Mật Khẩu Thông Minh
          </h2>
          <p className="text-xs sm:text-sm text-slate-500 dark:text-zinc-400">
            Dễ dàng di chuyển dữ liệu mật khẩu của bạn từ các bản ghi chép tự do, file Excel hoặc file xuất CSV từ Google Chrome/NordPass.
          </p>
        </div>

        {/* IOS-style Segments control tab selection */}
        <div className="grid grid-cols-2 gap-1 bg-slate-200/50 dark:bg-zinc-800/50 p-0.5 rounded-xl text-xs font-bold w-full max-w-md mx-auto sm:mx-0">
          <button
            onClick={() => {
              setActiveImportMode('freetext');
              setErrorStr(null);
            }}
            className={`py-2 rounded-lg text-center cursor-pointer transition-all flex items-center justify-center gap-1.5 ${
              activeImportMode === 'freetext' 
                ? 'bg-white dark:bg-zinc-700 text-blue-600 dark:text-sky-400 shadow-sm' 
                : 'text-slate-505 dark:text-zinc-400 hover:text-slate-800 dark:hover:text-zinc-200'
            }`}
          >
            <Clipboard size={14} />
            Dán văn bản tự do (${previewItems.length > 0 ? "Bản nháp" : "AI/Offline"})
          </button>
          <button
            onClick={() => {
              setActiveImportMode('file');
              setErrorStr(null);
            }}
            className={`py-2 rounded-lg text-center cursor-pointer transition-all flex items-center justify-center gap-1.5 ${
              activeImportMode === 'file' 
                ? 'bg-white dark:bg-zinc-700 text-blue-600 dark:text-sky-400 shadow-sm' 
                : 'text-slate-505 dark:text-zinc-400 hover:text-slate-800 dark:hover:text-zinc-200'
            }`}
          >
            <Upload size={14} />
            Nhập file CSV / JSON
          </button>
        </div>

        {/* Warning or Error feedback row */}
        {errorStr && (
          <div className="bg-red-50 dark:bg-red-950/20 border border-red-200/30 dark:border-red-900/30 text-red-600 dark:text-red-400 p-3.5 rounded-2xl flex items-start gap-3 text-xs font-medium animate-in fade-in slide-in-from-top-1 duration-150">
            <AlertCircle size={15} className="shrink-0 mt-0.5" />
            <span>{errorStr}</span>
          </div>
        )}

        {/* Normal Flow: free text entry */}
        {activeImportMode === 'freetext' && (
          <div className="bg-white dark:bg-[#1C1C1E] rounded-3xl p-5 border border-slate-200/30 dark:border-zinc-800/40 space-y-4 shadow-sm">
            <div className="flex flex-col sm:flex-row justify-between items-start sm:items-center gap-3">
              <div className="space-y-0.5">
                <span className="text-xs font-bold text-slate-800 dark:text-zinc-300 block">
                  Cửa sổ Nhập Ghi Chú Văn Bản
                </span>
                <p className="text-[10px] text-slate-400 dark:text-zinc-500">
                  Hãy sao chép các dòng mật khẩu bạn hay lưu trữ ở ghi chú, tin nhắn rồi dán trực tiếp vào ô bên dưới.
                </p>
              </div>
              <button
                type="button"
                onClick={handleLoadSampleData}
                className="text-[10px] font-bold text-blue-600 dark:text-sky-400 border border-blue-500/30 dark:border-sky-500/30 px-3 py-1.5 rounded-xl hover:bg-blue-50 dark:hover:bg-blue-950/50 transition-colors cursor-pointer shrink-0"
              >
                📝 Tải dữ liệu mẫu của Tôi
              </button>
            </div>

            {/* Input Big free-text field */}
            <div className="relative">
              <textarea
                value={freeTextValue}
                onChange={(e) => {
                  setFreeTextValue(e.target.value);
                  setErrorStr(null);
                }}
                rows={8}
                placeholder={`Dán văn bản thủ công của bạn vào đây, ví dụ:\nTw 5HgQB4D66U6Z\nTik dypn34u2tuec\n  uu29L8X!?N]#2Q\nFlex jellyfin Eddy ge35,LqS>=WZL*x\nCic WATN)h&Y4Uk>mQs\n15742\napple edy98 pVsRH4mf`}
                className="w-full bg-[#F5F5F7]/50 dark:bg-[#151518]/60 border border-slate-205 dark:border-zinc-850 p-4 rounded-2xl font-mono text-[11px] sm:text-xs text-slate-800 dark:text-zinc-200 placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-blue-500/30 dark:focus:ring-blue-500/20 focus:bg-white dark:focus:bg-[#1C1C1E] transition-all"
              />
              <span className="absolute bottom-3 right-3 text-[10px] text-slate-400 dark:text-zinc-500 font-medium font-sans">
                {freeTextValue.length} ký tự | {freeTextValue.split('\n').filter(Boolean).length} dòng
              </span>
            </div>

            {/* Advanced Gemini key input togglable card */}
            <div className="bg-[#F5F5F7] dark:bg-[#151518]/60 p-4 rounded-2xl border border-slate-200/50 dark:border-zinc-800/40 space-y-3 font-sans">
              <div className="flex items-start gap-2.5">
                <Cpu className="text-blue-500 w-4 h-4 shrink-0 mt-0.5" />
                <div className="space-y-0.5">
                  <span className="text-[11px] font-bold text-slate-700 dark:text-zinc-300 block">
                    Tính năng Phân Loại & Tối Ưu bằng AI
                  </span>
                  <p className="text-[10px] text-slate-400 leading-normal">
                    Nếu bạn có sẵn Gemini API Key từ Google AI Studio, hãy dán vào ô bên dưới. AI sẽ giúp bóc tách các dòng siêu lộn xộn, tự nhận diện danh mục và ghi chú tốt hơn gấp nhiều lần!
                  </p>
                </div>
              </div>

              <div className="flex gap-2">
                <input
                  type="password"
                  value={geminiKeyInput}
                  onChange={(e) => setGeminiKeyInput(e.target.value)}
                  placeholder="Dán Gemini API Key của bạn (nếu có)..."
                  className="flex-1 bg-white dark:bg-[#1C1C1E] border border-slate-200 dark:border-zinc-800 px-3 py-1.5 rounded-xl text-[11px] font-mono focus:outline-none focus:ring-1 focus:ring-blue-500 text-slate-800 dark:text-zinc-100 placeholder-slate-400/80"
                />
                {geminiKeyInput && (
                  <button
                    onClick={() => setGeminiKeyInput('')}
                    className="px-2.5 py-1.5 border border-red-200/50 dark:border-red-900/40 text-red-650 rounded-xl text-[10px] font-bold bg-red-50/50 dark:bg-red-950/20"
                    title="Xóa Key"
                  >
                    Clear
                  </button>
                )}
              </div>
            </div>

            {/* Action parsing triggers */}
            <div className="flex items-center justify-end gap-2.5 pt-1.5">
              <button
                type="button"
                onClick={handleOfflineParse}
                disabled={isProcessing}
                className="px-5 py-2.5 bg-slate-100 hover:bg-slate-200/80 active:scale-95 dark:bg-zinc-800 dark:hover:bg-zinc-700 font-bold text-slate-700 dark:text-zinc-200 rounded-xl text-xs transition-all cursor-pointer disabled:opacity-50 inline-flex items-center gap-1.5"
              >
                {isProcessing ? <RefreshCw className="animate-spin w-3.5 h-3.5" /> : '⚡️'}
                Phân tích Offline (Siêu tốc & Bảo mật)
              </button>
              
              <button
                type="button"
                onClick={handleGeminiParse}
                disabled={isProcessing}
                className="px-5 py-2.5 bg-gradient-to-r from-blue-600 to-indigo-600 hover:opacity-90 active:scale-95 dark:from-blue-650 dark:to-indigo-650 font-bold text-white rounded-xl text-xs shadow-sm shadow-blue-500/10 transition-all cursor-pointer disabled:opacity-50 inline-flex items-center gap-1.5"
              >
                {isProcessing ? <RefreshCw className="animate-spin w-3.5 h-3.5" /> : <Sparkles className="w-3.5 h-3.5 shrink-0" />}
                Trích xuất bằng AI Gemini
              </button>
            </div>
          </div>
        )}

        {/* File Drag & Drop Flow */}
        {activeImportMode === 'file' && (
          <div className="bg-white dark:bg-[#1C1C1E] rounded-3xl p-5 border border-slate-200/30 dark:border-zinc-800/40 space-y-4 shadow-sm font-sans">
            <span className="text-xs font-bold text-slate-800 dark:text-zinc-300 block">
              Nhập từ tệp tin cơ sở dữ liệu xuất bản
            </span>

            {/* Drag & drop box */}
            <div
              onDragOver={(e) => e.preventDefault()}
              onDrop={handleFileDrop}
              className="border-2 border-dashed border-slate-250 dark:border-zinc-700 hover:border-blue-500 dark:hover:border-sky-500 rounded-2xl p-8 py-10 text-center bg-[#F5F5F7]/40 dark:bg-[#2C2C2E]/20 hover:bg-white dark:hover:bg-[#1C1C1E] transition-all cursor-pointer relative"
            >
              <input
                type="file"
                accept=".csv, .json"
                onChange={handleFileSelect}
                className="absolute inset-0 w-full h-full opacity-0 cursor-pointer"
              />
              <span className="text-3xl block mb-2 leading-none">📂</span>
              <span className="text-xs font-bold text-slate-700 dark:text-slate-200 block">
                Kéo thả tệp tin hoặc click để duyệt
              </span>
              <span className="text-[10px] text-slate-400 block mt-1 leading-normal max-w-xs mx-auto">
                Hỗ trợ tệp tin chuẩn <strong>.csv</strong> xuất từ Chrome/NordPass, hoặc tệp .json xuất từ Vault Password Manager. Dung lượng tối đa 5MB.
              </span>
            </div>

            {/* Hint alert */}
            <div className="flex gap-2 bg-amber-50/60 dark:bg-amber-950/10 p-3.5 rounded-xl text-[10px] leading-relaxed text-amber-600 dark:text-amber-400 font-medium">
              <Info size={13} className="shrink-0 mt-0.5" />
              <span>
                <strong>Lưu ý bảo mật tệp tin:</strong> Tệp xuất bản thường lưu mật khẩu dạng văn bản trần không mã hóa (Plaintext). Vui lòng lưu trữ cẩn thận và xóa tệp tin này khỏi máy sau khi đã nhập thành công vào Vault an toàn của bạn.
              </span>
            </div>
          </div>
        )}

        {/* Smart Preview & Preview Table Block: Show results parsed */}
        {previewItems.length > 0 && (
          <div className="space-y-3.5 font-sans animate-in fade-in duration-350">
            
            {/* Title review area with control buttons */}
            <div className="flex flex-col sm:flex-row justify-between items-start sm:items-center gap-3">
              <div className="space-y-0.5 text-left">
                <h3 className="font-bold text-sm text-slate-800 dark:text-zinc-200 flex items-center gap-2">
                  🛡️ Bảng Kiểm Duyệt Kết Quả Bóc Tách
                  <span className="bg-blue-100 dark:bg-blue-950 text-[#007AFF] text-[10px] px-2 py-0.5 rounded-full font-bold">
                    Phát hiện {previewItems.length} tài khoản
                  </span>
                </h3>
                <p className="text-[10px] text-slate-400 dark:text-zinc-500">
                  Hãy rà soát kỹ lại, chỉnh sửa trực tiếp thông tin vào các ô bên dưới và tích chọn các tài khoản bạn muốn bảo lưu rồi bấm Nhập.
                </p>
              </div>

              <button
                onClick={handleAddEmptyPreviewRow}
                className="text-[10px] font-bold text-[#007AFF] dark:text-[#0A84FF] border border-blue-500/20 px-3 py-1.5 rounded-xl bg-blue-50/45 dark:bg-blue-950/20 cursor-pointer shrink-0 inline-flex items-center gap-1 hover:opacity-90"
              >
                <Plus size={12} className="stroke-[2.5]" />
                Thêm hàng thủ công
              </button>
            </div>

            {/* Smart Scroll Table container */}
            <div className="bg-white dark:bg-[#1C1C1E] border border-slate-200/50 dark:border-zinc-800 rounded-2xl overflow-hidden shadow-sm">
              <div className="overflow-x-auto max-w-full">
                <table className="w-full text-left border-collapse min-w-[700px]">
                  <thead>
                    <tr className="border-b border-[#E5E5EA]/50 dark:border-zinc-800 text-[10px] font-bold text-slate-400 dark:text-zinc-500 bg-slate-50/50 dark:bg-[#1C1C1E]/30 select-none uppercase tracking-wider">
                      <th className="py-2.5 px-3 w-10 text-center">
                        <button
                          onClick={handleToggleSelectAll}
                          className="text-slate-400 hover:text-blue-500 transition-colors"
                          title="Chọn tất cả"
                        >
                          {isAllSelected ? (
                            <CheckSquare size={15} className="text-blue-500 mx-auto" />
                          ) : (
                            <Square size={15} className="mx-auto" />
                          )}
                        </button>
                      </th>
                      <th className="py-2.5 px-3 max-w-[140px]">Tên Alias</th>
                      <th className="py-2.5 px-3 max-w-[140px]">Web / Ứng dụng</th>
                      <th className="py-2.5 px-3 max-w-[145px]">Username</th>
                      <th className="py-2.5 px-3 max-w-[145px]">Password (Mật khẩu)</th>
                      <th className="py-2.5 px-3 w-[120px]">Phân loại</th>
                      <th className="py-2.5 px-3 text-center w-12">Xóa</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-[#E5E5EA]/20 dark:divide-zinc-850">
                    {previewItems.map((item) => {
                      return (
                        <tr
                          key={item.id}
                          className={`hover:bg-[#F2F2F7]/30 dark:hover:bg-[#1C1C1E]/20 transition-colors ${
                            item.selected ? 'bg-blue-50/5 dark:bg-blue-950/5' : 'opacity-60'
                          }`}
                        >
                          {/* Item Select checkbox */}
                          <td className="py-2 px-3 text-center">
                            <button
                              onClick={() => handleToggleSelectItem(item.id)}
                              className="text-slate-400 hover:text-blue-500 transition-colors"
                            >
                              {item.selected ? (
                                <CheckSquare size={15} className="text-blue-500 mx-auto" />
                              ) : (
                                <Square size={15} className="mx-auto" />
                              )}
                            </button>
                          </td>

                          {/* Alias column inline input */}
                          <td className="py-2 px-3 font-medium">
                            <input
                              type="text"
                              value={item.alias}
                              onChange={(e) => handleUpdatePreviewField(item.id, 'alias', e.target.value)}
                              className="w-full bg-[#F5F5F7]/30 border-b border-transparent hover:border-slate-300 dark:hover:border-zinc-700/60 focus:border-blue-500 dark:border-transparent py-1 focus:outline-none text-[11px] font-sans font-bold text-slate-800 dark:text-zinc-100"
                              placeholder="Ví dụ: Facebook"
                            />
                          </td>

                          {/* Website app column inline input */}
                          <td className="py-2 px-3 font-medium">
                            <input
                              type="text"
                              value={item.appName}
                              onChange={(e) => handleUpdatePreviewField(item.id, 'appName', e.target.value)}
                              className="w-full bg-[#F5F5F7]/30 border-b border-transparent hover:border-slate-300 dark:hover:border-zinc-700/60 focus:border-blue-500 dark:border-transparent py-1 focus:outline-none text-[11px] font-mono text-slate-500 dark:text-zinc-400"
                              placeholder="Ví dụ: facebook.com"
                            />
                          </td>

                          {/* Username inline input */}
                          <td className="py-2 px-3 font-medium">
                            <input
                              type="text"
                              value={item.username}
                              onChange={(e) => handleUpdatePreviewField(item.id, 'username', e.target.value)}
                              className="w-full bg-[#F5F5F7]/30 border-b border-transparent hover:border-slate-300 dark:hover:border-zinc-700/60 focus:border-blue-500 dark:border-transparent py-1 focus:outline-none text-[11px] font-mono text-slate-655 dark:text-zinc-300"
                              placeholder="Chưa rõ / Nhập user..."
                            />
                          </td>

                          {/* Password inline input (plaintext for review easiness) */}
                          <td className="py-2 px-3 font-medium">
                            <input
                              type="text"
                              value={item.passwordDecrypted}
                              onChange={(e) => handleUpdatePreviewField(item.id, 'passwordDecrypted', e.target.value)}
                              className="w-full bg-[#F5F5F7]/30 border-b border-transparent hover:border-slate-300 dark:hover:border-zinc-700/60 focus:border-blue-500 dark:border-transparent py-1 focus:outline-none text-[11px] font-mono text-emerald-600 dark:text-emerald-400 font-bold"
                              placeholder="Nhập mật khẩu..."
                            />
                          </td>

                          {/* Category select block */}
                          <td className="py-2 px-3 font-medium">
                            <select
                              value={item.category}
                              onChange={(e) => handleUpdatePreviewField(item.id, 'category', e.target.value)}
                              className="w-full bg-[#F5F5F7]/40 dark:bg-[#1C1C1E] text-[10px] p-1 px-1.5 rounded-lg border border-slate-200 dark:border-zinc-800 text-slate-600 dark:text-zinc-300 font-bold cursor-pointer focus:outline-none"
                            >
                              {categories.map(cat => (
                                <option key={cat} value={cat}>{cat}</option>
                              ))}
                            </select>
                          </td>

                          {/* Delete Row button */}
                          <td className="py-2 px-3 text-center">
                            <button
                              onClick={() => handleDeletePreviewItem(item.id)}
                              className="p-1 text-slate-400 hover:text-red-500 rounded-md hover:bg-red-50 dark:hover:bg-red-950/20 transition-all cursor-pointer"
                              title="Loại bỏ dòng"
                            >
                              <Trash2 size={13} />
                            </button>
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
            </div>

            {/* Finish and Complete Import Row */}
            <div className="flex flex-col sm:flex-row justify-between items-start sm:items-center bg-blue-50/40 dark:bg-[#1C1C1E] border border-[#007AFF]/10 p-4 rounded-2xl gap-3">
              <span className="text-[11px] leading-relaxed text-slate-500 dark:text-zinc-400">
                <strong>Xong!</strong> Toàn bộ dữ liệu bóc tách được kiểm soát hoàn toàn bởi bạn. Bấm nút nạp ở góc phải để mã hóa AES-256 an toàn và đưa vĩnh viễn vào lưu trữ cục bộ offline của Vault Manager.
              </span>
              
              <button
                onClick={handleConfirmImport}
                className="px-6 py-3 bg-[#007AFF] hover:bg-[#0063E1] dark:bg-[#0A84FF] dark:hover:bg-[#0072E3] text-white rounded-xl text-xs font-bold shrink-0 transition-all active:scale-[0.97] shadow-sm shadow-blue-500/10 cursor-pointer inline-flex items-center gap-1.5"
              >
                <Check size={14} className="stroke-[2.5]" />
                Xác nhận Nhập {previewItems.filter(i => i.selected).length} tài khoản
              </button>
            </div>
          </div>
        )}

        {/* Success complete notification popup widget */}
        {successCount !== null && (
          <div className="bg-emerald-50 dark:bg-emerald-950/15 border border-emerald-200/30 dark:border-emerald-900/30 p-5 rounded-3xl text-left space-y-2 font-sans animate-in zoom-in-95 duration-200">
            <div className="flex items-center gap-2.5">
              <span className="text-xl">🎉</span>
              <h4 className="font-bold text-sm text-emerald-800 dark:text-emerald-450">
                Lưu trữ Thành Công!
              </h4>
            </div>
            <p className="text-xs text-slate-505 dark:text-zinc-400 leading-relaxed">
              Bạn đã nhập thành công <strong>{successCount}</strong> tài khoản mật khẩu vào Database an toàn cục bộ. Toàn bộ thông tin đăng nhập mới đã được bọc khóa mật mã AES-256 an toàn và có thể đồng bộ siêu tốc sang Giả lập Telegram Bot và Sync Agent nền ngay lập tức!
            </p>
            <div className="pt-1.5">
              <button
                type="button"
                onClick={() => setSuccessCount(null)}
                className="px-4 py-1.5 bg-emerald-600 hover:bg-emerald-700 text-white rounded-xl text-[10px] font-bold shadow-xs cursor-pointer"
              >
                Tuyệt vời, Đóng thông báo!
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
