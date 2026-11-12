import fsd from '@feature-sliced/steiger-plugin';
import { defineConfig } from 'steiger';

// Feature-Sliced Design rules for src/: layer import direction, slice isolation, public APIs.
export default defineConfig([
  ...fsd.configs.recommended,
  {
    // The app has a single page so far, so most slices have exactly one consumer. Keeping them
    // separate is deliberate: new screens and panels should be able to reuse them.
    rules: { 'fsd/insignificant-slice': 'off' },
  },
]);
