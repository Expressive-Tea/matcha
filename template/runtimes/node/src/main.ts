import { app } from './app';

// Awaited, so a boot or bind failure stops the process with its own error instead of surfacing
// later as an unhandled rejection.
await app.listen(3000);
console.log('🍵 green-tea running on http://localhost:3000');
