import React from 'react';
import { Sun, Moon, Monitor } from 'lucide-react';
import { useTheme, type Theme } from '../context/ThemeContext';

interface ThemeToggleProps {
  variant?: 'icon' | 'labeled' | 'selector';
  className?: string;
}

export const ThemeToggle: React.FC<ThemeToggleProps> = ({ variant = 'icon', className = '' }) => {
  const { theme, resolvedTheme, setTheme, toggleTheme } = useTheme();

  if (variant === 'selector') {
    const options: { id: Theme; label: string; icon: React.FC<{ className?: string }> }[] = [
      { id: 'light', label: 'Light', icon: Sun },
      { id: 'dark', label: 'Dark', icon: Moon },
      { id: 'system', label: 'System', icon: Monitor },
    ];

    return (
      <div className={`inline-flex items-center p-1 rounded-xl bg-[#EFECE4] dark:bg-[#22241F] border border-[#E8E5DC] dark:border-[#2C2E27] gap-1 ${className}`}>
        {options.map((opt) => {
          const Icon = opt.icon;
          const isActive = theme === opt.id;
          return (
            <button
              key={opt.id}
              type="button"
              data-testid={`theme-option-${opt.id}`}
              onClick={() => setTheme(opt.id)}
              className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-all cursor-pointer ${
                isActive
                  ? 'bg-white dark:bg-[#161714] text-[#1E201C] dark:text-[#EDEAE2] shadow-2xs font-semibold'
                  : 'text-[#6E6D66] dark:text-[#9E9C93] hover:text-[#1E201C] dark:hover:text-[#EDEAE2]'
              }`}
            >
              <Icon className="w-3.5 h-3.5" />
              <span>{opt.label}</span>
            </button>
          );
        })}
      </div>
    );
  }

  if (variant === 'labeled') {
    return (
      <button
        type="button"
        data-testid="theme-toggle-labeled"
        onClick={toggleTheme}
        className={`flex items-center gap-2 px-2.5 py-1.5 rounded-lg text-xs text-[#3A3B36] dark:text-[#D4D1C7] hover:bg-[#E5E1D6] dark:hover:bg-[#262822] transition-colors cursor-pointer ${className}`}
        title={`Current theme: ${theme} (Click to toggle)`}
      >
        {resolvedTheme === 'dark' ? (
          <Sun className="w-3.5 h-3.5 text-amber-500 transition-transform hover:rotate-45" />
        ) : (
          <Moon className="w-3.5 h-3.5 text-[#6E6D66] transition-transform hover:-rotate-12" />
        )}
        <span>{resolvedTheme === 'dark' ? 'Light Mode' : 'Dark Mode'}</span>
      </button>
    );
  }

  // Default 'icon' button
  return (
    <button
      type="button"
      data-testid="theme-toggle-btn"
      onClick={toggleTheme}
      className={`p-1.5 rounded-lg text-[#6E6D66] dark:text-[#9E9C93] hover:text-[#1E201C] dark:hover:text-[#EDEAE2] hover:bg-[#E5E1D6] dark:hover:bg-[#262822] transition-all cursor-pointer flex items-center justify-center ${className}`}
      title={resolvedTheme === 'dark' ? 'Switch to Light Mode' : 'Switch to Dark Mode'}
      aria-label="Toggle theme"
    >
      {resolvedTheme === 'dark' ? (
        <Sun className="w-4 h-4 text-amber-400 hover:rotate-45 transition-transform" />
      ) : (
        <Moon className="w-4 h-4 text-[#6E6D66] hover:-rotate-12 transition-transform" />
      )}
    </button>
  );
};
