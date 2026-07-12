import { Route, Get, Html, Sse } from '@green-tea/core';

const ZEN = [
  'Simplicity is the ultimate sophistication.',
  'The obstacle is the path.',
  'When you let go, you have two hands to work.',
  'Make it work, make it right, make it fast.',
  'Less, but better.',
];

@Route('/')
export class HomeController {
  @Get('/')
  @Html('public/index.html')
  home() {}

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
