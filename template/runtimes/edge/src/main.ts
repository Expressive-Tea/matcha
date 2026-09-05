import { edgeHandler } from '@green-tea/core/edge';

import { app } from './app';

export default { fetch: edgeHandler(app) };
