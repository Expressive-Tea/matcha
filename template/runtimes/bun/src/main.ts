import { serveBun } from '@green-tea/core/bun';

import { app } from './app';

// Boots before it binds: a provider that fails stops the process here, instead
// of leaving an open port that answers 500 to everything.
await serveBun(app, { port: 8000 });
console.log('🍵 green-tea running on http://localhost:8000');
