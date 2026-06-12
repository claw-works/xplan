import { useState, useEffect, createContext, useContext, useCallback } from 'react';
import { X } from 'lucide-react';

interface ToastItem {
  id: number;
  message: string;
  type: 'error' | 'success';
}

interface ToastContextType {
  showError: (message: string) => void;
  showSuccess: (message: string) => void;
}

const ToastContext = createContext<ToastContextType>({
  showError: () => {},
  showSuccess: () => {},
});

export function useToast() {
  return useContext(ToastContext);
}

let nextId = 0;

export function ToastProvider({ children }: { children: React.ReactNode }) {
  const [toasts, setToasts] = useState<ToastItem[]>([]);

  const addToast = useCallback((message: string, type: 'error' | 'success') => {
    const id = nextId++;
    setToasts(prev => [...prev, { id, message, type }]);
  }, []);

  const removeToast = useCallback((id: number) => {
    setToasts(prev => prev.filter(t => t.id !== id));
  }, []);

  const showError = useCallback((message: string) => addToast(message, 'error'), [addToast]);
  const showSuccess = useCallback((message: string) => addToast(message, 'success'), [addToast]);

  return (
    <ToastContext.Provider value={{ showError, showSuccess }}>
      {children}
      <div className="fixed top-4 right-4 z-50 flex flex-col gap-2 max-w-sm">
        {toasts.map(toast => (
          <ToastItem key={toast.id} toast={toast} onDismiss={() => removeToast(toast.id)} />
        ))}
      </div>
    </ToastContext.Provider>
  );
}

function ToastItem({ toast, onDismiss }: { toast: ToastItem; onDismiss: () => void }) {
  useEffect(() => {
    const timer = setTimeout(onDismiss, 5000);
    return () => clearTimeout(timer);
  }, [onDismiss]);

  const bg = toast.type === 'error'
    ? 'bg-red-950/90 border-red-800 text-red-200'
    : 'bg-green-950/90 border-green-800 text-green-200';

  return (
    <div className={`px-4 py-3 rounded-lg border backdrop-blur-sm shadow-lg flex items-start gap-3 animate-in slide-in-from-right ${bg}`}>
      <p className="text-sm flex-1">{toast.message}</p>
      <button onClick={onDismiss} className="text-current opacity-60 hover:opacity-100 shrink-0">
        <X size={14} />
      </button>
    </div>
  );
}
