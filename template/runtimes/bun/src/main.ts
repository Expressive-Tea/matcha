import { app } from './app';

Bun.serve({ port: 8000, fetch: app.fetch });
console.log('🍵 green-tea running on http://localhost:8000');
