import { app } from './app';

Deno.serve({ port: 8000 }, app.fetch);
console.log('🍵 green-tea running on http://localhost:8000');
