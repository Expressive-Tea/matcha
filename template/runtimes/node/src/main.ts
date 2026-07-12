import { createApp } from '@green-tea/core';
import { AppModule } from './app.module';

const app = createApp({ modules: [AppModule] });

app.listen(3000);
console.log('🍵 green-tea running on http://localhost:3000');
