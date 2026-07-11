import { createApp } from '@green-tea/core';
import { AppModule } from './app.module';

const app = createApp({ modules: [AppModule] });

Deno.serve({ port: 8000 }, app.fetch);
console.log('🍵 green-tea running on http://localhost:8000');
