import { createApp } from '@green-tea/core';
import { AppModule } from './app.module';

/**
 * The application graph, exported rather than kept local so it can be reached
 * without serving: `matcha graph` and `matcha explain` import this file and
 * call `app.ready()`, which resolves the graph and does not boot providers.
 * `src/main.ts` is the half that binds a port.
 */
export const app = createApp({ modules: [AppModule] });
