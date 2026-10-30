import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ThemeProvider } from '../context/ThemeContext';
import { ThemeToggle } from '../components/ThemeToggle';
import { ModelSettingsModal } from '../components/ModelSettingsModal';

describe('Theme Management & Light/Dark Mode Experience', () => {
  beforeEach(() => {
    localStorage.clear();
    document.documentElement.className = '';
    document.documentElement.removeAttribute('data-theme');
    document.documentElement.style.colorScheme = '';
  });

  afterEach(() => {
    localStorage.clear();
    document.documentElement.className = '';
  });

  it('initializes with default light theme and updates document.documentElement', () => {
    render(
      <ThemeProvider>
        <ThemeToggle variant="icon" />
      </ThemeProvider>
    );

    expect(document.documentElement.classList.contains('dark')).toBe(false);
    expect(document.documentElement.getAttribute('data-theme')).toBe('light');
    expect(document.documentElement.style.colorScheme).toBe('light');
  });

  it('loads existing stored preference from localStorage', () => {
    localStorage.setItem('sthira_theme', 'dark');

    render(
      <ThemeProvider>
        <ThemeToggle variant="icon" />
      </ThemeProvider>
    );

    expect(document.documentElement.classList.contains('dark')).toBe(true);
    expect(document.documentElement.getAttribute('data-theme')).toBe('dark');
    expect(document.documentElement.style.colorScheme).toBe('dark');
  });

  it('icon variant toggles theme on click and updates localStorage and HTML root', () => {
    render(
      <ThemeProvider>
        <ThemeToggle variant="icon" />
      </ThemeProvider>
    );

    const toggleBtn = screen.getByTestId('theme-toggle-btn');
    expect(toggleBtn).toBeInTheDocument();
    expect(document.documentElement.classList.contains('dark')).toBe(false);

    // Toggle to Dark
    fireEvent.click(toggleBtn);
    expect(document.documentElement.classList.contains('dark')).toBe(true);
    expect(document.documentElement.getAttribute('data-theme')).toBe('dark');
    expect(localStorage.getItem('sthira_theme')).toBe('dark');

    // Toggle back to Light
    fireEvent.click(toggleBtn);
    expect(document.documentElement.classList.contains('dark')).toBe(false);
    expect(document.documentElement.getAttribute('data-theme')).toBe('light');
    expect(localStorage.getItem('sthira_theme')).toBe('light');
  });

  it('selector variant renders Light, Dark, System buttons and updates selection', () => {
    render(
      <ThemeProvider>
        <ThemeToggle variant="selector" />
      </ThemeProvider>
    );

    const lightBtn = screen.getByTestId('theme-option-light');
    const darkBtn = screen.getByTestId('theme-option-dark');
    const systemBtn = screen.getByTestId('theme-option-system');

    expect(lightBtn).toBeInTheDocument();
    expect(darkBtn).toBeInTheDocument();
    expect(systemBtn).toBeInTheDocument();

    // Click Dark
    fireEvent.click(darkBtn);
    expect(document.documentElement.classList.contains('dark')).toBe(true);
    expect(localStorage.getItem('sthira_theme')).toBe('dark');

    // Click Light
    fireEvent.click(lightBtn);
    expect(document.documentElement.classList.contains('dark')).toBe(false);
    expect(localStorage.getItem('sthira_theme')).toBe('light');

    // Click System
    fireEvent.click(systemBtn);
    expect(localStorage.getItem('sthira_theme')).toBe('system');
  });

  it('labeled variant toggles theme and updates label text', () => {
    render(
      <ThemeProvider>
        <ThemeToggle variant="labeled" />
      </ThemeProvider>
    );

    const labeledBtn = screen.getByTestId('theme-toggle-labeled');
    expect(labeledBtn).toHaveTextContent(/Dark Mode/i);

    // Click to switch to Dark Mode
    fireEvent.click(labeledBtn);
    expect(document.documentElement.classList.contains('dark')).toBe(true);
    expect(labeledBtn).toHaveTextContent(/Light Mode/i);

    // Click to switch back to Light Mode
    fireEvent.click(labeledBtn);
    expect(document.documentElement.classList.contains('dark')).toBe(false);
    expect(labeledBtn).toHaveTextContent(/Dark Mode/i);
  });

  it('Appearance tab in ModelSettingsModal allows switching theme', async () => {
    render(
      <ThemeProvider>
        <ModelSettingsModal isOpen={true} onClose={vi.fn()} onVaultMutated={vi.fn()} />
      </ThemeProvider>
    );

    // Switch to Appearance tab
    const appearanceTab = screen.getByRole('button', { name: /Appearance/i });
    expect(appearanceTab).toBeInTheDocument();
    fireEvent.click(appearanceTab);

    // Check that theme options are rendered inside Appearance tab
    expect(screen.getAllByText(/Warm Stone/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/Deep Obsidian/i).length).toBeGreaterThan(0);

    // Select Dark mode from selector
    const darkOption = screen.getByTestId('theme-option-dark');
    fireEvent.click(darkOption);

    expect(document.documentElement.classList.contains('dark')).toBe(true);
    expect(localStorage.getItem('sthira_theme')).toBe('dark');
  });
});
