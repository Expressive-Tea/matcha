import { createApp } from '@green-tea/core';
import { AppModule } from './app.module';

const app = createApp({ modules: [AppModule] });

Bun.serve({ port: 8000, fetch: app.fetch });
console.log('🍵 green-tea running on http://localhost:8000');
