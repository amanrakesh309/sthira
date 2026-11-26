import { defineConfig, mergeConfig } from 'vitest/config';
import viteConfig from './vite.config';

export default mergeConfig(viteConfig, defineConfig({
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: './src/test/setup.ts',
    // 15 suites each spin up their own jsdom environment in parallel; individual tests
    // that render the full `App` routinely exceed the 5s default under that load and
    // were intermittently timing out. This headroom makes the suite deterministic.
    testTimeout: 20000,
  },
}));
