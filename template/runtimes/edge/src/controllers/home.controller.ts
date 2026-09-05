import { Route, Sse } from '@green-tea/core';

const ZEN = [
  'Simplicity is the ultimate sophistication.',
  'The obstacle is the path.',
  'When you let go, you have two hands to work.',
  'Make it work, make it right, make it fast.',
  'Less, but better.',
];

@Route('/')
export class HomeController {
  // No @Html('public/index.html') here: workerd has no filesystem to read it
  // from. wrangler.toml's [assets] serves ./public instead, and the worker is
  // reached for everything the assets do not answer.
  @Sse('/zen')
  zen() {
    return (async function* () {
      for (let i = 0; ; i++) {
        yield { zen: ZEN[i % ZEN.length] };
        await new Promise((r) => setTimeout(r, 30_000));
      }
    })();
  }
}
