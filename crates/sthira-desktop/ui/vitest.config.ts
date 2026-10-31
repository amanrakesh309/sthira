import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: './src/test/setup.ts',
    // 15 suites each spin up their own jsdom environment in parallel; individual tests
    // that render the full `App` routinely exceed the 5s default under that load and
    // were intermittently timing out. This headroom makes the suite deterministic.
    testTimeout: 20000,
  },
});
